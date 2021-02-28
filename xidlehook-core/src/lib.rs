#![warn(
    // Harden built-in lints
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,

    // Harden clippy lints
    clippy::cargo_common_metadata,
    clippy::clone_on_ref_ptr,
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    clippy::float_cmp_const,
    clippy::get_unwrap,
    clippy::integer_arithmetic,
    clippy::integer_division,
    clippy::print_stdout,
)]
#![allow(
    // I don't agree with this lint
    clippy::must_use_candidate,

    // The integer arithmetic here is mostly regarding indexes into Vecs, indexes where memory
    // allocation will fail far, far earlier than the arithmetic will fail.
    clippy::integer_arithmetic,
)]

//! Instead of implementing your extension as something that
//! communicates with xidlehook, what about implementing your
//! extension as something that *is* xidlehook?
//!
//! This library lets you create your own xidlehook front-end using a
//! powerful timer and module system.

use std::{
    cmp,
    convert::TryInto,
    fmt, ptr,
    time::{Duration, Instant},
};

use log::{info, trace, warn};
use nix::libc;

/// The default error type for xidlehook. Unfortunately, it's a
/// dynamic type for now.
pub type Error = Box<dyn std::error::Error>;
/// An alias to Result which overrides the default Error type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

pub mod modules;
pub mod timers;

pub use self::{
    modules::{Module, Progress},
    timers::Timer,
};

/// An identifier for a timer, based on the index in the timer list
/// and its length.
#[derive(Clone, Copy, Debug)]
pub struct TimerInfo {
    /// The index of this timer in the timer list
    pub index: usize,
    /// The length of the timer list
    pub length: usize,
}

/// Return value of `poll`, which specifies what one should do next: sleep,
/// wait forever (until client modifies the xidlehook instance),
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Sleep for (at most) a specified duration
    Sleep(Duration),
    /// Xidlehook has nothing to do, so you should effectively wait forever until the client modifies the xidlehook instance
    Forever,
    /// A module wants xidlehook to quit
    Quit,
}

/// The main xidlehook instance that allows you to schedule things
pub struct Xidlehook<T: Timer, M: Module>
where
    T: Timer,
    M: Module,
{
    module: M,

    /// Whether to reset on sleep
    detect_sleep: bool,

    timers: Vec<T>,
    next_index: usize,
    /// The base idle time: the absolute idle time when the last timer
    /// was called, used to retrieve the relative idle time since it.
    base_idle_time: Duration,
    /// The previous idle time, used for comparing whether or not the
    /// user has moved.
    previous_idle_time: Duration,
    /// If a chain is aborted during the process, store this here as
    /// to not make any more attempts to continue it.
    aborted: bool,
}
impl<T: Timer> Xidlehook<T, ()> {
    /// An empty instance without any modules
    pub fn new(timers: Vec<T>) -> Self {
        Self {
            module: (),

            detect_sleep: false,

            timers,
            next_index: 0,
            base_idle_time: Duration::default(),
            previous_idle_time: Duration::default(),
            aborted: false,
        }
    }
}

macro_rules! with_module {
    ($self:expr, $module:expr) => {
        Xidlehook {
            module: $module,

            detect_sleep: $self.detect_sleep,

            timers: $self.timers,
            next_index: $self.next_index,
            base_idle_time: $self.base_idle_time,
            previous_idle_time: $self.previous_idle_time,
            aborted: $self.aborted,
        }
    };
}

// There are some false positive with Self and generics.
#[allow(clippy::use_self)]
impl<T, M> Xidlehook<T, M>
where
    T: Timer,
    M: Module,
{
    /// Return this xidlehook instance but with this module replaced.
    pub fn with_module<N: Module>(self, other: N) -> Xidlehook<T, N> {
        with_module!(self, other)
    }

    /// Return this xidlehook instance but with an additional module activated. This works using the
    /// timer impl for `(A, B)` to get a fixed-size list of modules at compile time.
    pub fn register<N: Module>(self, other: N) -> Xidlehook<T, (M, N)> {
        // Sadly cannot use `self.with_module` safely due to use of `self.module` - Rust isn't
        // intelligent enough to realize the function isn't using that field. This is one of the few
        // shortcomings of Rust IMO.
        with_module!(self, (self.module, other))
    }

    /// Set whether or not we reset the idle timer once a suspend was detected. This only affects
    /// main/main_async.
    pub fn set_detect_sleep(&mut self, value: bool) {
        self.detect_sleep = value;
    }
    /// Get whether or not we reset the idle timer once a suspend was detected
    pub fn detect_sleep(&self) -> bool {
        self.detect_sleep
    }
    /// Set whether or not we reset the idle timer once a suspend was detected. This only affects
    /// main/main_async. This is the chainable version of `set_detect_sleep`.
    pub fn with_detect_sleep(mut self, value: bool) -> Self {
        self.detect_sleep = value;
        self
    }

    /// Returns an immutable list of all timers
    pub fn timers(&self) -> &Vec<T> {
        &self.timers
    }

    /// Returns a mutable list of all timers. Use this to add or remove timers as you wish. This
    /// will abort the idle chain as that may otherwise panic.
    pub fn timers_mut(&mut self) -> Result<&mut Vec<T>> {
        self.abort()?;
        Ok(&mut self.timers)
    }

    /// Returns the previous timer that was activated (but not deactivated)
    fn previous(&mut self) -> Option<&mut T> {
        self.next_index
            .checked_sub(1)
            .map(move |i| &mut self.timers[i])
    }

    /// Calls the abortion function on the current timer and stops pursuing the chain
    pub fn abort(&mut self) -> Result<()> {
        if self.aborted {
            return Ok(());
        }

        self.aborted = true;
        if let Some(prev) = self.previous() {
            prev.abort()?;
        }
        Ok(())
    }

    /// Calls the abortion functions on the current timer and restarts from index zero. Just like
    /// with the `poll` function, continued usage after an error discouraged.
    pub fn reset(&mut self, absolute_time: Duration) -> Result<()> {
        self.abort()?;

        trace!("Resetting");

        if self.next_index > 0 {
            if let Err(err) = self.module.reset() {
                self.module.warning(&err)?;
            }
            self.next_index = 0;
        }

        self.base_idle_time = absolute_time;
        self.previous_idle_time = absolute_time;
        self.aborted = false;

        Ok(())
    }

    /// Skip ahead to the selected timer. Timers leading up to this point will not be ran. If you
    /// pass `force`, modules will not even be able to prevent this from happening (all requests
    /// pre-timer would be ignored). Post-timer requests are fully complied with.
    ///
    /// Whatever the return value is, it's already been handled. If the return value is `Err(...)`,
    /// that means this function invoked the module's `warning` function and that still wanted to
    /// propagate the error. If the return value is `Ok(Progress::Abort)`, never mind it. The
    /// `self.abort()` function has already been invoked - it's all cool.
    ///
    /// # Panics
    ///
    /// - If the index is out of bounds
    pub fn trigger(
        &mut self,
        index: usize,
        absolute_time: Duration,
        force: bool,
    ) -> Result<Progress> {
        macro_rules! handle {
            ($progress:expr) => {
                match $progress {
                    Progress::Continue => (),
                    Progress::Abort => {
                        trace!("Module requested abort of chain.");
                        self.abort()?;
                        return Ok(Progress::Abort);
                    },
                    Progress::Reset => {
                        trace!("Module requested reset of chain.");
                        self.reset(absolute_time)?;
                        return Ok(Progress::Reset);
                    },
                    Progress::Stop => return Ok(Progress::Stop),
                }
            };
        }
        trace!("Activating timer {}", index);

        let timer_info = TimerInfo {
            index,
            length: self.timers.len(),
        };

        let next = &mut self.timers[index];

        // Trigger module pre-timer
        match self.module.pre_timer(timer_info) {
            Ok(_) if force => (),
            Ok(progress) => handle!(progress),
            Err(err) => {
                self.module.warning(&err)?;
            },
        }

        // Send activation signal to current timer
        next.activate()?;

        // Send deactivation signal to previous timer
        if let Some(previous) = self.previous() {
            previous.deactivate()?;
        }

        // Reset the idle time to zero
        self.base_idle_time = absolute_time;

        // Send module post-timer
        match self.module.post_timer(timer_info) {
            Ok(progress) => handle!(progress),
            Err(err) => {
                self.module.warning(&err)?;
            },
        }

        // Next time, continue from next index
        self.next_index = index + 1;

        Ok(Progress::Continue)
    }

    /// Polls the scheduler for any activated timers. On success, returns the max amount of time a
    /// program can sleep for. Only fatal errors cause this function to return, and at that point,
    /// the state of xidlehook is undefined so it should not be used.
    pub fn poll(&mut self, absolute_time: Duration) -> Result<Action> {
        if absolute_time < self.previous_idle_time {
            // If the idle time has decreased, the only reasonable explanation is that the user
            // briefly wasn't idle. We reset the base idle time to zero so the entire idle duration
            // is counted.
            self.reset(Duration::from_millis(0))?;
        }

        self.previous_idle_time = absolute_time;

        // We can only ever sleep as long as it takes for the first timer to activate, since the
        // user may become active (and then idle again) at any point.
        let mut max_sleep = Duration::from_nanos(u64::MAX);

        let mut first_timer = 0;

        while let Some(timer) = self.timers.get_mut(first_timer) {
            if !timer.disabled() {
                break;
            }

            // This timer may re-activate in the future and take presedence over the timer we
            // thought was the next enabled timer.
            if let Some(remaining) = timer.time_left(Duration::from_nanos(0))? {
                trace!(
                    "Taking disabled first timer into account. Remaining: {:?}",
                    remaining
                );
                max_sleep = cmp::min(max_sleep, remaining);
            }

            first_timer += 1;
        }

        if let Some(timer) = self.timers.get_mut(first_timer) {
            if let Some(remaining) = timer.time_left(Duration::from_nanos(0))? {
                trace!(
                    "Taking first timer into account. Remaining: {:?}",
                    remaining
                );
                max_sleep = cmp::min(max_sleep, remaining)
            }
        } else {
            // No timer was enabled!
            return Ok(Action::Forever);
        }

        if self.aborted {
            trace!("This chain was aborted, I won't pursue it");
            return Ok(Action::Sleep(max_sleep));
        }

        let relative_time = absolute_time - self.base_idle_time;
        trace!("Relative time: {:?}", relative_time);

        let mut next_index = self.next_index;

        while let Some(timer) = self.timers.get_mut(next_index) {
            if !timer.disabled() {
                break;
            }

            // This timer may re-activate in the future and take presedence over the timer we
            // thought was the next enabled timer.
            if let Some(remaining) = timer.time_left(relative_time)? {
                trace!(
                    "Taking disabled timer into account. Remaining: {:?}",
                    remaining
                );
                max_sleep = cmp::min(max_sleep, remaining);
            }

            next_index += 1;
        }

        // When there's a next timer available, get the time until that activates
        if let Some(next) = self.timers.get_mut(next_index) {
            if let Some(remaining) = next.time_left(relative_time)? {
                trace!(
                    "Taking next enabled timer into account. Remaining: {:?}",
                    remaining
                );
                max_sleep = cmp::min(max_sleep, remaining);
            } else {
                trace!("Triggering timer #{}", next_index);
                // Oh! It has already been passed - let's trigger it.
                match self.trigger(next_index, absolute_time, false)? {
                    Progress::Stop => return Ok(Action::Quit),
                    _ => (),
                }

                // Recurse to find return value
                return self.poll(absolute_time);
            }
        }

        // When there's a previous timer, respect that timer's abort urgency (see
        // `Timer::abort_urgency()`)
        if let Some(abort) = self.previous() {
            if let Some(urgency) = abort.abort_urgency() {
                trace!(
                    "Taking abort urgency into account. Remaining: {:?}",
                    urgency
                );
                max_sleep = cmp::min(max_sleep, urgency);
            }
        }

        Ok(Action::Sleep(max_sleep))
    }

    /// Runs a standard poll-sleep-repeat loop.
    /// ```rust
    /// # if std::env::var("DISPLAY").is_err() {
    /// #     // Don't fail on CI.
    /// #     return Ok::<(), xidlehook_core::Error>(());
    /// # }
    /// # use std::{
    /// #     sync::atomic::{AtomicBool, Ordering},
    /// #     time::Duration,
    /// # };
    /// #
    /// # use nix::{
    /// #     libc,
    /// #     sys::{signal, wait},
    /// # };
    /// # use xidlehook_core::{
    /// #     modules::{StopAt, Xcb},
    /// #     timers::CallbackTimer,
    /// #     Xidlehook,
    /// # };
    /// #
    /// # let timers = vec![
    /// #     CallbackTimer::new(Duration::from_millis(50), || println!("50ms passed!")),
    /// # ];
    /// # let mut xidlehook = Xidlehook::new(timers)
    /// #     .register(StopAt::completion());
    /// # let xcb = Xcb::new()?;
    /// static EXITED: AtomicBool = AtomicBool::new(false);
    ///
    /// extern "C" fn exit_handler(_signo: libc::c_int) {
    ///     EXITED.store(true, Ordering::SeqCst);
    /// }
    ///
    /// unsafe {
    ///     signal::sigaction(
    ///         signal::Signal::SIGINT,
    ///         &signal::SigAction::new(
    ///             signal::SigHandler::Handler(exit_handler),
    ///             signal::SaFlags::empty(),
    ///             signal::SigSet::empty(),
    ///         ),
    ///     )?;
    /// }
    /// xidlehook.main_sync(&xcb, || EXITED.load(Ordering::SeqCst));
    /// # Ok::<(), xidlehook_core::Error>(())
    /// ```
    pub fn main_sync<F>(mut self, xcb: &self::modules::Xcb, mut callback: F) -> Result<()>
    where
        F: FnMut() -> bool,
    {
        loop {
            let idle = xcb.get_idle()?;
            match self.poll(idle)? {
                Action::Sleep(delay) => {
                    trace!("Sleeping for {:?}", delay);

                    let sleep_start = Instant::now();

                    // This sleep, unlike `thread::sleep`, will stop for signals.
                    unsafe {
                        libc::nanosleep(
                            &libc::timespec {
                                tv_sec: delay
                                    .as_secs()
                                    .try_into()
                                    .expect("woah that's one large number"),
                                tv_nsec: delay
                                    .subsec_nanos()
                                    .try_into()
                                    .expect("woah that's one large number"),
                            },
                            ptr::null_mut(),
                        );
                    }

                    if let Some(time_difference) = sleep_start.elapsed().checked_sub(delay) {
                        if time_difference >= Duration::from_secs(3) && self.detect_sleep {
                            info!(
                                "We slept {:?} longer than expected - has the computer been suspended?",
                                time_difference,
                            );
                            self.reset(xcb.get_idle()?)?;
                        }
                    }
                },
                Action::Forever => {
                    warn!("xidlehook has not, and will never get, anything to do");
                    break;
                },
                Action::Quit => break,
            }

            if callback() {
                // Oh look, the callback wants us to exit
                break;
            }
        }
        Ok(())
    }

    /// Runs a standard poll-sleep-repeat loop... asynchronously.
    #[cfg(any(feature = "async-std", feature = "tokio"))]
    pub async fn main_async(&mut self, xcb: &self::modules::Xcb) -> Result<()> {
        loop {
            let idle = xcb.get_idle()?;
            match self.poll(idle)? {
                Action::Sleep(delay) => {
                    trace!("Sleeping for {:?}", delay);

                    let sleep_start = Instant::now();

                    #[cfg(feature = "async-std")]
                    async_std::task::sleep(delay).await;
                    #[cfg(feature = "tokio")]
                    if cfg!(not(feature = "async-std")) {
                        tokio::time::delay_for(delay).await;
                    }

                    if let Some(time_difference) = sleep_start.elapsed().checked_sub(delay) {
                        if time_difference >= Duration::from_secs(3) && self.detect_sleep {
                            info!(
                                "We slept {:?} longer than expected - has the computer been suspended?",
                                time_difference,
                            );
                            self.reset(xcb.get_idle()?)?;
                        }
                    }
                },
                Action::Forever => {
                    trace!("Nothing to do");

                    #[cfg(feature = "async-std")]
                    async_std::future::pending::<()>().await;
                    #[cfg(feature = "tokio")]
                    if cfg!(not(feature = "async-std")) {
                        use tokio::stream::StreamExt;
                        tokio::stream::pending::<()>().next().await;
                    }
                },
                Action::Quit => break,
            }
        }
        Ok(())
    }
}

impl<T, M> fmt::Debug for Xidlehook<T, M>
where
    T: Timer,
    M: Module + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Modules: {:?}", self.module)
    }
}
