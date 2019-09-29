use crate::Result;
use std::{
    process::Command,
    time::Duration,
};

pub trait Timer {
    /// Return the time left based on the relative idle time
    fn time_left(&mut self, idle_time: Duration) -> Result<Option<Duration>>;
    /// How urgent this timer wants to be notified on abort (when the
    /// user is no longer idle). Return as slow of a duration as you
    /// think is acceptable to be nice to the CPU - preferrably return
    /// `None` which basically means infinity.
    fn abort_urgency(&self) -> Option<Duration> { None }

    /// Called when the timer was activated
    fn activate(&mut self) -> Result<()> { Ok(()) }
    /// Called when the timer was aborted early - such as when the
    /// user moves their mouse or otherwise stops being idle.
    fn abort(&mut self) -> Result<()> { Ok(()) }
    /// Called when another timer was activated after this one
    fn deactivate(&mut self) -> Result<()> { Ok(()) }
}

#[derive(Default)]
pub struct CmdTimer {
    pub time: Duration,
    pub activation: Option<Command>,
    pub abortion: Option<Command>,
    pub deactivation: Option<Command>,
}
impl Timer for CmdTimer {
    fn time_left(&mut self, idle_time: Duration) -> Result<Option<Duration>> {
        Ok(self.time.checked_sub(idle_time).filter(|&dur| dur != Duration::new(0, 0)))
    }
    fn abort_urgency(&self) -> Option<Duration> {
        self.abortion.as_ref().map(|_| Duration::from_secs(1))
    }

    fn activate(&mut self) -> Result<()> {
        if let Some(ref mut activation) = self.activation {
            activation.spawn()?;
        }
        Ok(())
    }
    fn abort(&mut self) -> Result<()> {
        if let Some(ref mut abortion) = self.abortion {
            abortion.spawn()?;
        }
        Ok(())
    }
    fn deactivate(&mut self) -> Result<()> {
        if let Some(ref mut deactivation) = self.deactivation {
            deactivation.spawn()?;
        }
        Ok(())
    }
}
