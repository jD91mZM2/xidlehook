use crate::{Module, Progress, Result, TimerInfo};
use log::debug;

pub struct StopAt {
    stop_after: Option<usize>,
}
impl StopAt {
    /// Returns a module which will stop execution after a chain of
    /// timers have reached a certain timer index.
    pub fn index(i: usize) -> Self {
        StopAt { stop_after: Some(i) }
    }
    /// Returns a module which will stop execution after a chain of
    /// timers have executed entirely once.
    pub fn completion() -> Self {
        StopAt { stop_after: None }
    }
}
impl Module for StopAt {
    fn post_timer(&mut self, timer: TimerInfo) -> Result<Progress> {
        let stop_after = self.stop_after.unwrap_or(timer.length - 1); // timer list is never empty
        if timer.index >= stop_after {
            Ok(Progress::Stop)
        } else {
            Ok(Progress::Continue)
        }
    }
}
