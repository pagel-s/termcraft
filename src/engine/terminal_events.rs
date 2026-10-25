//! Keep terminal motion bursts from delaying control keys behind rendering.
use crossterm::event::{self, Event, MouseEventKind};
use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

const CAPACITY: usize = 256;
const POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Default)]
struct Queue {
    events: VecDeque<Event>,
    error: Option<io::Error>,
    stopping: bool,
}

impl Queue {
    fn replace_motion(&mut self, incoming: &Event) -> bool {
        if matches!(incoming, Event::Mouse(mouse) if mouse.kind == MouseEventKind::Moved)
            && matches!(self.events.back(), Some(Event::Mouse(mouse)) if mouse.kind == MouseEventKind::Moved)
        {
            *self.events.back_mut().unwrap() = incoming.clone();
            true
        } else {
            false
        }
    }
}

type Shared = Arc<(Mutex<Queue>, Condvar)>;

pub(super) struct TerminalEvents {
    shared: Shared,
    worker: Option<JoinHandle<()>>,
}

impl TerminalEvents {
    pub(super) fn spawn() -> io::Result<Self> {
        let shared = Shared::default();
        let input = shared.clone();
        let worker = thread::Builder::new()
            .name("terminal-input".into())
            .spawn(move || {
                let (queue, changed) = &*input;
                loop {
                    if queue.lock().unwrap().stopping {
                        break;
                    }
                    let incoming = match event::poll(POLL_INTERVAL) {
                        Ok(false) => continue,
                        Ok(true) => event::read(),
                        Err(error) => Err(error),
                    };
                    let mut pending = queue.lock().unwrap();
                    let incoming = match incoming {
                        Ok(incoming) => incoming,
                        Err(error) => {
                            pending.error = Some(error);
                            changed.notify_all();
                            break;
                        }
                    };
                    // Coalesce only consecutive passive motion. Clicks, drags,
                    // wheel steps, keys and releases retain their exact order.
                    if pending.replace_motion(&incoming) {
                        continue;
                    }
                    while pending.events.len() >= CAPACITY && !pending.stopping {
                        pending = changed.wait(pending).unwrap();
                    }
                    if pending.stopping {
                        break;
                    }
                    pending.events.push_back(incoming);
                    changed.notify_all();
                }
            })?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    pub(super) fn next(&self) -> io::Result<Option<Event>> {
        let (queue, changed) = &*self.shared;
        let mut pending = queue.lock().unwrap();
        if let Some(error) = pending.error.take() {
            return Err(error);
        }
        let event = pending.events.pop_front();
        if event.is_some() {
            changed.notify_all();
        }
        Ok(event)
    }

    pub(super) fn wait(&self, timeout: Duration) {
        let (queue, changed) = &*self.shared;
        let pending = queue.lock().unwrap();
        if pending.events.is_empty() && pending.error.is_none() && !pending.stopping {
            drop(changed.wait_timeout(pending, timeout).unwrap());
        }
    }
}

impl Drop for TerminalEvents {
    fn drop(&mut self) {
        let (queue, changed) = &*self.shared;
        queue.lock().unwrap().stopping = true;
        changed.notify_all();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent};

    fn mouse(column: u16, kind: MouseEventKind) -> Event {
        Event::Mouse(MouseEvent {
            kind,
            column,
            row: 20,
            modifiers: KeyModifiers::NONE,
        })
    }

    fn push(queue: &mut Queue, event: Event) {
        if !queue.replace_motion(&event) {
            queue.events.push_back(event);
        }
    }

    #[test]
    fn motion_burst_retains_latest_position_and_following_quit_key() {
        let mut queue = Queue::default();
        for column in 0..10_000 {
            push(&mut queue, mouse(column, MouseEventKind::Moved));
        }
        let quit = Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        push(&mut queue, quit.clone());
        assert_eq!(queue.events.len(), 2);
        assert_eq!(queue.events[0], mouse(9_999, MouseEventKind::Moved));
        assert_eq!(queue.events[1], quit);
    }

    #[test]
    fn motion_does_not_cross_click_key_release_or_focus_boundaries() {
        let boundaries = [
            mouse(4, MouseEventKind::Down(MouseButton::Left)),
            mouse(5, MouseEventKind::Up(MouseButton::Left)),
            Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char('d'),
                KeyModifiers::NONE,
                crossterm::event::KeyEventKind::Release,
            )),
            Event::FocusLost,
        ];
        for boundary in boundaries {
            let mut queue = Queue::default();
            push(&mut queue, mouse(1, MouseEventKind::Moved));
            push(&mut queue, boundary.clone());
            push(&mut queue, mouse(9, MouseEventKind::Moved));
            assert_eq!(queue.events.len(), 3);
            assert_eq!(queue.events[1], boundary);
        }
    }

    #[test]
    fn inventory_drag_and_wheel_steps_are_never_coalesced() {
        for kind in [
            MouseEventKind::Drag(MouseButton::Left),
            MouseEventKind::ScrollDown,
        ] {
            let mut queue = Queue::default();
            for column in 1..4 {
                push(&mut queue, mouse(column, kind));
            }
            assert_eq!(queue.events.len(), 3);
        }
    }
}
