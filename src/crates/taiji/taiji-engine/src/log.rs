use std::sync::atomic::{AtomicU8, Ordering};

pub struct Logger {
    mode: AtomicU8,
}

impl Logger {
    pub fn new(mode: LogMode) -> Self {
        Self {
            mode: AtomicU8::new(mode as u8),
        }
    }

    pub fn set_mode(&self, mode: LogMode) {
        self.mode.store(mode as u8, Ordering::Relaxed);
    }

    pub fn info(&self, msg: &str) {
        self.log(2, msg);
    }
    pub fn warn(&self, msg: &str) {
        self.log(3, msg);
    }
    pub fn error(&self, msg: &str) {
        self.log(4, msg);
    }
    pub fn debug(&self, msg: &str) {
        self.log(1, msg);
    }

    fn log(&self, level: u8, msg: &str) {
        let mode = self.mode.load(Ordering::Relaxed);
        match mode {
            0 => {} // Off
            1 => {
                eprintln!("[taiji] {}", msg);
            } // Simple
            _ => match level {
                1 => tracing::debug!("{}", msg),
                2 => tracing::info!("{}", msg),
                3 => tracing::warn!("{}", msg),
                4 => tracing::error!("{}", msg),
                _ => {}
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LogMode {
    Off = 0,
    Simple = 1,
    Tracing = 2,
}

impl Default for Logger {
    fn default() -> Self {
        let mode = std::env::var("TAIJI_LOG_MODE")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(|m: u8| match m {
                0 => LogMode::Off,
                1 => LogMode::Simple,
                _ => LogMode::Tracing,
            })
            .unwrap_or(LogMode::Tracing);
        Self::new(mode)
    }
}
