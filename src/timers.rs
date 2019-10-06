use crate::Result;
use std::{process::Command, time::Duration};

pub trait Timer {
    /// Return the time left based on the relative idle time
    fn time_left(&mut self, idle_time: Duration) -> Result<Option<Duration>>;
    /// How urgent this timer wants to be notified on abort (when the
    /// user is no longer idle). Return as slow of a duration as you
    /// think is acceptable to be nice to the CPU - preferrably return
    /// `None` which basically means infinity.
    fn abort_urgency(&self) -> Option<Duration> {
        None
    }

    /// Called when the timer was activated
    fn activate(&mut self) -> Result<()> {
        Ok(())
    }
    /// Called when the timer was aborted early - such as when the
    /// user moves their mouse or otherwise stops being idle.
    fn abort(&mut self) -> Result<()> {
        Ok(())
    }
    /// Called when another timer was activated after this one
    fn deactivate(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct CmdTimer {
    pub time: Duration,
    pub activation: Option<Command>,
    pub abortion: Option<Command>,
    pub deactivation: Option<Command>,
}
impl Timer for CmdTimer {
    fn time_left(&mut self, idle_time: Duration) -> Result<Option<Duration>> {
        Ok(self
            .time
            .checked_sub(idle_time)
            .filter(|&dur| dur != Duration::default()))
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

/// A timer that lets you easily execute a rust callback on
/// activation
pub struct CallbackTimer<F>
where
    F: FnMut()
{
    time: Duration,
    f: F,
}
impl<'a> CallbackTimer<Box<dyn FnMut() + 'a>> {
    /// Create a new instance, which boxes the closure to a dynamic
    /// type. Use `new_unboxed` to use static dispatch, although keep
    /// in mind this will potentially make you unable to register more
    /// than one type of callback timer due to its static type.
    pub fn new<F>(time: Duration, f: F) -> Self
    where
        F: FnMut() + 'a
    {
        Self::new_unboxed(time, Box::new(f))
    }
}
impl<F> CallbackTimer<F>
where
    F: FnMut()
{
    /// Create a new unboxed instance. Due to it's static type, only
    /// one type can be used. This means that registering 2 timers
    /// with 2 different callbacks will conflict. An easy way to
    /// bypass this is using the `new` function, which behind the
    /// scenes just wraps the callback in a Box.
    ///
    /// TL;DR: Don't use this unless you're planning on using another
    /// means of dynamic dispatch (an enum?) or if you're a masochist.
    pub fn new_unboxed(time: Duration, f: F) -> Self {
        Self { time, f }
    }
}
impl<F> Timer for CallbackTimer<F>
where
    F: FnMut()
{
    fn time_left(&mut self, idle_time: Duration) -> Result<Option<Duration>> {
        Ok(self
            .time
            .checked_sub(idle_time)
            .filter(|&d| d != Duration::default()))
    }
    fn activate(&mut self) -> Result<()> {
        (self.f)();
        Ok(())
    }
}

