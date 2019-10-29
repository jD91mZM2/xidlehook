//! Uses `PulseAudio`'s APIs to detect whenever audio is playing, and
//! if so it refuses to let xidlehook run the next timer command. This
//! is used to implement `--not-when-audio` in the xidlehook example
//! application.

use crate::{Module, Progress, Result, TimerInfo};

use libpulse_binding::{
    callbacks::ListResult,
    context::{self, subscribe::Facility, Context, State},
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
    mainloop: Rc<RefCell<Mainloop>>,
}
impl NotWhenAudio {
    /// Connect to PulseAudio and subscribe to notification of changes
    pub fn new() -> Result<Self> {
        let mut mainloop = Rc::new(RefCell::new(
            Mainloop::new().expect("pulseaudio: failed to create main loop")
        ));

        let ctx = Rc::new(RefCell::new(
            Context::new(&main, PA_NAME).expect("pulseaudio: failed to create context")
        ));

        // Setup context state change callback
        {
            let ml_ref = Rc::clone(&mainloop);
            let ctx_ref = Rc::clone(&ctx);

            ctx.borrow_mut().set_state_callback(Some(Box::new(move || {
                let state = unsafe { &*ctx_ref.as_ptr() } // Borrow checker workaround
                    .get_state();
                match state {
                    context::State::Ready |
                    context::State::Failed |
                    context::State::Terminated => {
                        unsafe { &mut *ml_ref.as_ptr() } // Borrow checker workaround
                            .signal(false);
                    },
                    _ => {},
                }
            })));
        }

        ctx.borrow_mut().connect(None, context::flags::NOFLAGS, None)
            .expect("pulseaudio: failed to connect context");

        mainloop.borrow_mut().lock();
        mainloop.borrow_mut().start().expect("pulseaudio: failed to start mainloop");

        // Wait for context to be ready
        loop {
            match ctx.borrow().get_state() {
                pulse::context::State::Ready => { break; },
                pulse::context::State::Failed |
                pulse::context::State::Terminated => {
                    mainloop.borrow_mut().unlock();
                    mainloop.borrow_mut().stop();
                    panic!("pulseaudio: context state failed/terminated unexpectedly");
                    return;
                },
                _ => { mainloop.borrow_mut().wait(); },
            }
        }
        context.borrow_mut().set_state_callback(None);

        let counter = Rc::new(Counter {
            in_progress: Cell::new(0),
            last_total: Cell::new(0),
        });

        // Closure for setting up async count of input sinks
        let get_sinks = |ctx: &mut Context, counter: Rc<Counter>| {
            ctx
                .introspect()
                .get_sink_input_info_list(move |res|
                    match res {
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
                    }
                );
        };

        // Setup notification callback
        //
        // Upon notification of a change, we will make use of introspection
        // to obtain a fresh count of active input sinks.
        {
            let ctx_ref = Rc::clone(&ctx);
            let counter = Rc::clone(&counter);
            let get_sinks_cl = Rc::Clone(&get_sinks);

            ctx.borrow_mut().set_subscribe_callback(Some(Box::new(move |_, _, _| {
                let ctx_ref = unsafe { &mut *ctx_ref.as_ptr() }; // Borrow checker workaround
                get_sinks_cl(ctx_ref, counter);
            })));
        }

        // Subscribe to sink input events
        ctx.borrow_mut().subscribe(Facility::SinkInput.to_interest_mask(), |_| ());

        // Check if audio is already playing
        get_sinks(ctx.borrow_mut(), Rc::clone(&counter));

        mainloop.borrow_mut().unlock();

        Ok(Self { counter, ctx, mainloop })
    }
}
impl fmt::Debug for NotWhenAudio {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NotWhenAudio")
    }
}
impl Drop for NotWhenAudio {
    fn drop(&mut self) {
        self.mainloop.borrow_mut().lock();
        self.mainloop.borrow_mut().stop();
        self.ctx.borrow_mut().disconnect();
        self.mainloop.borrow_mut().unlock();
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
