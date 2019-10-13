//! Stops xidlehook completely at a specific index of the chain or at
//! the end. This is used to implement `--once` in the xidlehook
//! example application.

use crate::{Module, Progress, Result, TimerInfo};

use std::fmt;

use log::trace;

/// See the module-level documentation
#[derive(Clone, Copy)]
pub struct StopAt {
    stop_after: Option<usize>,
}
impl StopAt {
    /// Returns a module which will stop execution after a chain of
    /// timers have reached a certain timer index.
    pub fn index(i: usize) -> Self {
        Self {
            stop_after: Some(i),
        }
    }
    /// Returns a module which will stop execution after a chain of
    /// timers have executed entirely once.
    pub fn completion() -> Self {
        Self { stop_after: None }
    }
}
impl Module for StopAt {
    fn post_timer(&mut self, timer: TimerInfo) -> Result<Progress> {
        #[allow(clippy::integer_arithmetic)] // timer list is never empty
        let stop_after = self.stop_after.unwrap_or(timer.length - 1);

        trace!("{}/{}", timer.index, stop_after);
        if timer.index >= stop_after {
            Ok(Progress::Stop)
        } else {
            Ok(Progress::Continue)
        }
    }
}
impl fmt::Debug for StopAt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StopAt")
    }
}
