//! The timer trait and some useful implementations

use crate::Result;
use std::{
    process::{Child, Command},
    time::Duration,
};

/// The timer trait is used to tell xidlehook after how much idle time
/// your timer should activate (relatively), and what activation
/// actually means. It also provides you with the ability to implement
/// what happens when the next timer is activated, and also to disable
/// the timer.
pub trait Timer {
    /// Return the time left based on the relative idle time
    fn time_left(&mut self, idle_time: Duration) -> Result<Option<Duration>>;
    /// How urgent this timer wants to be notified on abort (when the user is no longer idle).
    /// Return as slow of a duration as you think is acceptable to be nice to the CPU - preferrably
    /// return `None` which basically means infinity.
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
    /// Return true if the timer is disabled and should be skipped. Changes to this value are
    /// reflected - you may enable a timer that was previously disabled, and xidlehook will call it
    /// as soon as the timer is passed - or immediately if the timer has already passed.
    fn disabled(&mut self) -> bool {
        false
    }
}

/// A simple timer that runs a binary executable after a certain
/// amount of time
#[derive(Debug, Default)]
pub struct CmdTimer {
    /// The idle time required for this timer to activate
    pub time: Duration,
    /// The command, if any, to run upon activation
    pub activation: Option<Command>,
    /// The command, if any, to run upon abortion
    pub abortion: Option<Command>,
    /// The command, if any, to run upon deactivation
    pub deactivation: Option<Command>,
    /// Whether or not to disable this timer
    pub disabled: bool,

    /// The child process that is currently running
    pub activation_child: Option<Child>,
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
            let child = activation.spawn()?;
            let pid = child.id().to_string();

            if let Some(ref mut abortion) = self.abortion {
                abortion.env("XIDLEHOOK_PID", &pid);
            }
            if let Some(ref mut deactivation) = self.deactivation {
                deactivation.env("XIDLEHOOK_PID", &pid);
            }

            self.activation_child = Some(child);
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
    fn disabled(&mut self) -> bool {
        if let Some(Ok(None)) = self.activation_child.as_mut().map(|child| child.try_wait()) {
            // We temporarily disable this timer while the child is still running
            true
        } else {
            // Whether or not this command is disabled
            self.disabled
        }
    }
}

/// A timer that lets you easily execute a rust callback on
/// activation
#[derive(Debug)]
pub struct CallbackTimer<F>
where
    F: FnMut(),
{
    time: Duration,
    f: F,

    /// Whether or not to disable this timer
    pub disabled: bool,
}
impl<'a> CallbackTimer<Box<dyn FnMut() + 'a>> {
    /// Create a new instance, which boxes the closure to a dynamic
    /// type. Use `new_unboxed` to use static dispatch, although keep
    /// in mind this will potentially make you unable to register more
    /// than one type of callback timer due to its static type.
    pub fn new<F>(time: Duration, f: F) -> Self
    where
        F: FnMut() + 'a,
    {
        Self::new_unboxed(time, Box::new(f))
    }
}
impl<F> CallbackTimer<F>
where
    F: FnMut(),
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
        Self {
            time,
            f,
            disabled: false,
        }
    }
}
impl<F> Timer for CallbackTimer<F>
where
    F: FnMut(),
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
    fn disabled(&mut self) -> bool {
        self.disabled
    }
}
