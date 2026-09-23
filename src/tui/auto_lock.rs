use std::time::Instant;

pub struct AutoLock {
    pub enabled: bool,

    pub after_secs: u64,

    pub last_activity: Instant,
}

impl AutoLock {
    pub fn new(enabled: bool, after_secs: u64) -> Self {
        Self {
            enabled,
            after_secs,
            last_activity: Instant::now(),
        }
    }

    pub fn reset(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn is_expired(&self) -> bool {
        self.enabled && self.last_activity.elapsed().as_secs() >= self.after_secs
    }
}
