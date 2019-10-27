//! Uses `PulseAudio`'s APIs to detect whenever audio is playing, and
//! if so it refuses to let xidlehook run the next timer command. This
//! is used to implement `--not-when-audio` in the xidlehook example
//! application.

use crate::{Module, Progress, Result, TimerInfo};

use libpulse_binding::{
    callbacks::ListResult,
    context::{subscribe::Facility, Context, State},
    mainloop::threaded::Mainloop,
};
use log::debug;
use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
};

const PA_NAME: &str = "xidlehook";

struct Counter {
    in_progress: Cell<usize>,
    last_total: Cell<usize>,
}

/// See module-level docs
pub struct NotWhenAudio {
    counter: Rc<Counter>,
    ctx: Rc<RefCell<Context>>,
    main: Mainloop,
}
impl NotWhenAudio {
    /// Create a new pulseaudio main loop, but don't connect it yet
    pub fn new() -> Result<Self> {
        let mut main = Mainloop::new().ok_or("pulseaudio: failed to create main loop")?;

        // These variables don't need to use thread-safe reference
        // counters due to the fact that we're using `main.lock()`
        // which will prevent processing of events.
        let ctx = Rc::new(RefCell::new(
            Context::new(&main, PA_NAME).ok_or("pulseaudio: failed to create context")?,
        ));

        let counter = Rc::new(Counter {
            in_progress: Cell::new(0),
            last_total: Cell::new(0),
        });

        {
            let counter = Rc::clone(&counter);
            let ctx = Rc::clone(&ctx);

            let subscribe_callback = {
                let ctx = Rc::clone(&ctx);
                let counter = Rc::clone(&counter);

                move |_, _, _| {
                    let counter = Rc::clone(&counter);
                    ctx.borrow_mut()
                        .introspect()
                        .get_sink_input_info_list(move |res| match res {
                            ListResult::Item(item) => {
                                if !item.corked {
                                    let count = counter.in_progress.get() + 1;
                                    counter.in_progress.set(count);
                                    debug!("Partial count: {}", count);
                                }
                            },
                            ListResult::End | ListResult::Error => {
                                let count = counter.in_progress.replace(0);
                                counter.last_total.set(count);
                                debug!("Total sum: {}", count);
                            },
                        });
                }
            };

            ctx.borrow_mut().set_subscribe_callback(Some(Box::new(subscribe_callback.clone())));

            Rc::clone(&ctx)
                .borrow_mut()
                .set_state_callback(Some(Box::new(move || {
                    if ctx.borrow().get_state() != State::Ready {
                        return;
                    }

                    // Subscribe to sink input events
                    {
                        let mut ctx = ctx.borrow_mut();
                        ctx.subscribe(Facility::SinkInput.to_interest_mask(), |_| ());
                    }

                    // In case audio already plays, trigger
                    subscribe_callback(None, None, 0);
                })));
        }

        // We sadly can't use borrow_mut here because that keeps a
        // mutable reference alive while it's runnig all the
        // callbacks, leading to mutability errors there. See
        // https://github.com/jnqnfe/pulse-binding-rust/issues/19.
        unsafe { &mut *ctx.as_ptr() }
            .connect(None, 0, None)
            .map_err(|err| format!("pulseaudio: {}", err))?;

        main.lock();
        main.start().map_err(|err| format!("pulseaudio: {}", err))?;
        main.unlock();

        Ok(Self { counter, ctx, main })
    }
}
impl fmt::Debug for NotWhenAudio {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NotWhenAudio")
    }
}
impl Drop for NotWhenAudio {
    fn drop(&mut self) {
        // See note above
        unsafe { &mut *self.ctx.as_ptr() }.disconnect();
        self.main.stop();
    }
}
impl Module for NotWhenAudio {
    fn pre_timer(&mut self, _timer: TimerInfo) -> Result<Progress> {
        let players = self.counter.last_total.get();
        if players == 0 {
            Ok(Progress::Continue)
        } else {
            Ok(Progress::Abort)
        }
    }
}
