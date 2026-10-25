//! Bounded rolling timing samples; diagnostics stay out of the normal HUD.
use std::time::Duration;

const SAMPLE_COUNT: usize = 300;

pub struct TimingWindow {
    samples_us: [u64; SAMPLE_COUNT],
    count: usize,
    cursor: usize,
}

impl Default for TimingWindow {
    fn default() -> Self {
        Self {
            samples_us: [0; SAMPLE_COUNT],
            count: 0,
            cursor: 0,
        }
    }
}

impl TimingWindow {
    pub fn record(&mut self, duration: Duration) {
        self.samples_us[self.cursor] = duration.as_micros().min(u64::MAX as u128) as u64;
        self.cursor = (self.cursor + 1) % SAMPLE_COUNT;
        self.count = (self.count + 1).min(SAMPLE_COUNT);
    }

    pub fn p95_ms(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        let mut samples = self.samples_us;
        samples[..self.count].sort_unstable();
        samples[(self.count - 1) * 95 / 100] as f64 / 1000.0
    }
}

#[derive(Default)]
pub struct PerformanceMetrics {
    pub tick: TimingWindow,
    pub render: TimingWindow,
    pub save_and_cache: TimingWindow,
    pub dropped_catchups: u64,
    pub network_inbox: TimingWindow,
    pub network_received_at: Option<std::time::Instant>,
    pub network_tick: u64,
    pub network_superseded: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_forgets_old_spikes_and_handles_no_samples() {
        let mut window = TimingWindow::default();
        assert_eq!(window.p95_ms(), 0.0);
        for _ in 0..SAMPLE_COUNT {
            window.record(Duration::from_millis(200));
        }
        assert_eq!(window.p95_ms(), 200.0);
        for _ in 0..SAMPLE_COUNT {
            window.record(Duration::from_millis(2));
        }
        assert_eq!(window.p95_ms(), 2.0);
        assert_eq!(window.count, SAMPLE_COUNT);
    }
}
