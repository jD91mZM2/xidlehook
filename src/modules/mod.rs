use crate::{Error, Result};

use log::warn;

/// A decision each module has to take before a timer is executed:
/// Should it be?
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Progress {
    Continue,
    Abort,
}

/// A generic module that controls whether timers should execute or
/// not (outside of the normal timer)
pub trait Module {
    /// Decides if the timer should be allowed to execute
    fn pre_timer(&mut self) -> Result<Progress> {
        Ok(Progress::Continue)
    }

    /// Is called when there's a recoverable error
    fn warning(&mut self, _error: &Error) {}

    /// If this is called, the counting was reset - clear any cache
    /// here
    fn reset(&mut self) -> Result<()> {
        Ok(())
    }
}

/// The default module is also the unit type because why not
impl Module for () {
    fn warning(&mut self, error: &Error) {
        warn!("{} (Debug: {:?})", error, error);
    }
}

impl Module for Box<dyn Module> {
    fn pre_timer(&mut self) -> Result<Progress> {
        (&mut **self).pre_timer()
    }
    fn warning(&mut self, error: &Error) {
        (&mut **self).warning(error)
    }
    fn reset(&mut self) -> Result<()> {
        (&mut **self).reset()
    }
}

/// Combine two timers using the type-system. Can be recursed for a
/// fixed-size amount of timers. Similar to iterator.chain.
impl<A, B> Module for (A, B)
where
    A: Module,
    B: Module,
{
    fn pre_timer(&mut self) -> Result<Progress> {
        if self.0.pre_timer()? == Progress::Abort {
            return Ok(Progress::Abort);
        }
        self.1.pre_timer()
    }
    fn warning(&mut self, error: &Error) {
        self.0.warning(error);
        self.1.warning(error);
    }
    fn reset(&mut self) -> Result<()> {
        self.0.reset()?;
        self.1.reset()
    }
}

/// Combine multiple modules with a dynamic size
impl<M: Module> Module for Vec<M> {
    fn pre_timer(&mut self) -> Result<Progress> {
        for module in self {
            if module.pre_timer()? == Progress::Abort {
                return Ok(Progress::Abort);
            }
        }
        Ok(Progress::Continue)
    }
    fn warning(&mut self, error: &Error) {
        for module in self {
            module.warning(error);
        }
    }
    fn reset(&mut self) -> Result<()> {
        for module in self {
            module.reset()?;
        }
        Ok(())
    }
}

#[cfg(feature = "pulse")]
pub mod pulse;
pub mod xcb;

#[cfg(feature = "pulse")]
pub use self::pulse::NotWhenAudio;
pub use self::xcb::Xcb;
