//! Session-only, server-authoritative player chat.
use super::director::ChatView;
use super::net::SERVER_TICK_RATE;
use std::collections::{HashMap, VecDeque};

pub const MAX_MESSAGE_CHARS: usize = 280;
const HISTORY_LIMIT: usize = 32;
const BURST: u64 = 5;
const PREVIEW_TICKS: u64 = SERVER_TICK_RATE * 8;

#[derive(Default)]
pub(crate) struct PlayerChat {
    lines: VecDeque<String>,
    preview_until: u64,
    // Five messages immediately, then one per second. Simulation time keeps
    // validation deterministic and bounds work independently of client clocks.
    budget: HashMap<u16, (u64, u64)>,
    notices: HashMap<u16, (String, u64)>,
}

impl PlayerChat {
    pub fn notice(&mut self, id: u16, message: String, tick: u64) {
        self.notices.insert(id, (message, tick + PREVIEW_TICKS));
    }
    pub fn submit(&mut self, id: u16, text: &str, tick: u64) {
        if text.chars().any(char::is_control) {
            return;
        }
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let notice = if text.chars().count() > MAX_MESSAGE_CHARS {
            Some("Message too long (280 characters maximum).")
        } else if text.starts_with('/') {
            Some("Unknown chat command. Type a message to talk to other players.")
        } else {
            None
        };
        if let Some(notice) = notice {
            self.notices
                .insert(id, (notice.into(), tick + PREVIEW_TICKS));
            return;
        }
        let (tokens, refill) = self.budget.entry(id).or_insert((BURST, tick));
        let elapsed = tick.saturating_sub(*refill) / SERVER_TICK_RATE;
        *tokens = tokens.saturating_add(elapsed).min(BURST);
        *refill += elapsed * SERVER_TICK_RATE;
        if *tokens == 0 {
            self.notices.insert(
                id,
                (
                    "Chat is busy; wait a moment before sending again.".into(),
                    tick + PREVIEW_TICKS,
                ),
            );
            return;
        }
        *tokens -= 1;
        self.notices.remove(&id);
        self.lines.push_back(format!("P{id}: {text}"));
        while self.lines.len() > HISTORY_LIMIT {
            self.lines.pop_front();
        }
        self.preview_until = tick + PREVIEW_TICKS;
    }

    pub fn disconnect(&mut self, id: u16) {
        self.budget.remove(&id);
        self.notices.remove(&id);
    }

    pub fn view(&self, id: u16, tick: u64, experimental: Option<&ChatView>) -> ChatView {
        let mut view = ChatView {
            mode: format!("players:{id}"),
            lines: self.lines.clone(),
            status: if tick < self.preview_until {
                self.lines.back().cloned().unwrap_or_default()
            } else {
                String::new()
            },
            ..ChatView::default()
        };
        if let Some(chat) = experimental {
            view.lines.extend(chat.lines.iter().cloned());
            view.pending = chat.pending;
            if !chat.status.is_empty() {
                view.status.clone_from(&chat.status);
            }
        }
        if let Some((notice, until)) = self.notices.get(&id)
            && tick < *until
        {
            view.status.clone_from(notice);
            view.lines.push_back(notice.clone());
        }
        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_uses_server_identity_and_session_history_outlives_preview() {
        let mut chat = PlayerChat::default();
        chat.submit(7, "  P1: hello  ", 0);
        let sender = chat.view(7, 1, None);
        let guest = chat.view(2, 1, None);
        assert_eq!(sender.lines, guest.lines);
        assert_eq!(guest.lines[0], "P7: P1: hello");
        assert_eq!(guest.mode, "players:2");
        let later = chat.view(2, PREVIEW_TICKS, None);
        assert!(later.status.is_empty());
        assert_eq!(later.lines, guest.lines);
    }

    #[test]
    fn validation_limits_unicode_length_and_never_broadcasts_controls_or_commands() {
        let mut chat = PlayerChat::default();
        for text in ["", "  ", "hello\nP1: forged", "\x1b[2J", "/connect"] {
            chat.submit(1, text, 0);
        }
        assert!(chat.view(2, 1, None).lines.is_empty());
        chat.submit(1, &"界".repeat(MAX_MESSAGE_CHARS + 1), 0);
        assert!(chat.view(1, 1, None).status.contains("too long"));
        chat.submit(1, &"界".repeat(MAX_MESSAGE_CHARS), 0);
        assert_eq!(chat.view(2, 1, None).lines.len(), 1);
    }

    #[test]
    fn flood_is_per_sender_and_history_and_notices_stay_bounded() {
        let mut chat = PlayerChat::default();
        for _ in 0..100 {
            chat.submit(1, "hello", 0);
        }
        assert_eq!(chat.view(2, 1, None).lines.len(), BURST as usize);
        assert!(chat.view(1, 1, None).status.contains("wait"));
        chat.submit(2, "hi", 0);
        chat.submit(1, "refilled", SERVER_TICK_RATE);
        assert_eq!(chat.view(2, 21, None).lines.len(), BURST as usize + 2);
        for tick in 2..100 {
            chat.submit(2, "later", tick * SERVER_TICK_RATE);
        }
        assert_eq!(chat.view(2, 2000, None).lines.len(), HISTORY_LIMIT);
        chat.disconnect(1);
        assert!(!chat.budget.contains_key(&1));
        assert!(!chat.notices.contains_key(&1));
    }
}
