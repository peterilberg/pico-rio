use embassy_time::{Duration, Instant, Ticker};
use messages::Diagnostics;
use {defmt_rtt as _, panic_probe as _};

pub struct Timer {
    pub duration: Duration,
    pub ticker: Ticker,
    pub start_time: Instant,
    pub expected_time: Option<Instant>,
    pub diagnostics: Diagnostics,
}

impl Timer {
    pub fn new(duration: Duration) -> Self {
        Timer {
            duration,
            ticker: Ticker::every(duration),
            start_time: Instant::now(),
            expected_time: None,
            diagnostics: Diagnostics {
                timestamp_us: 0,
                execution_us: 0,
                jitter_in_us: 0,
                period_in_us: duration.as_micros(),
            },
        }
    }

    pub fn wait(&mut self) -> impl Future<Output = ()> {
        self.ticker.next()
    }

    pub fn start(&mut self) {
        self.start_time = Instant::now();

        let jitter = match self.expected_time {
            None => Duration::from_secs(0),
            Some(expected_time) => self
                .start_time
                .checked_duration_since(expected_time)
                .unwrap_or(Duration::from_secs(0)),
        };

        self.diagnostics.timestamp_us = self.start_time.as_micros();
        self.diagnostics.jitter_in_us = jitter.as_micros();
        self.expected_time = match self.expected_time {
            None => self.start_time.checked_add(self.duration),
            Some(expected_time) => expected_time.checked_add(self.duration),
        };
    }

    pub fn stop(&mut self) -> Diagnostics {
        self.diagnostics.execution_us = Instant::now()
            .checked_duration_since(self.start_time)
            .unwrap_or(Duration::from_secs(0))
            .as_micros();
        Diagnostics { ..self.diagnostics }
    }
}
