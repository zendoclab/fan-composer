use std::time::Duration;
pub const QUIET_DURATION: Duration = Duration::from_millis(3500);
const INTERVAL: Duration = Duration::from_secs(30);
pub struct CalibrationSchedule {
    started: Duration,
    until: Duration,
    next: Duration,
    was_periodic: bool,
}
impl Default for CalibrationSchedule {
    fn default() -> Self {
        Self {
            started: Duration::ZERO,
            until: QUIET_DURATION,
            next: INTERVAL,
            was_periodic: false,
        }
    }
}
impl CalibrationSchedule {
    pub fn update(&mut self, now: Duration, manual: bool, periodic: bool) -> bool {
        if periodic && !self.was_periodic {
            self.next = now + INTERVAL;
        }
        self.was_periodic = periodic;
        if manual || periodic && now >= self.next {
            self.started = now;
            self.until = now + QUIET_DURATION;
            self.next = now + INTERVAL;
        }
        now < self.until
    }
    pub fn settled(&self, now: Duration) -> bool {
        now >= self.started + Duration::from_millis(450)
    }
}
