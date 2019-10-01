use crate::{Module, Progress, Result, TimerInfo};

use libpulse_binding::{
    callbacks::ListResult,
    context::{subscribe::Facility, Context, State},
    mainloop::threaded::Mainloop,
};
use log::debug;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

const PA_NAME: &str = "xidlehook";

struct Counter {
    in_progress: AtomicUsize,
    last_total: AtomicUsize,
}

pub struct NotWhenAudio {
    counter: Arc<Counter>,
    ctx: Rc<RefCell<Context>>,
    main: Mainloop,
}
impl NotWhenAudio {
    /// Create a new pulseaudio main loop, but don't connect it yet
    pub fn new() -> Result<Self> {
        let mut main = Mainloop::new().ok_or("pulseaudio: failed to create main loop")?;

        // Should probably be thread-safe, see https://github.com/jnqnfe/pulse-binding-rust/issues/27
        // ... but it can't be thread-safe, see https://github.com/jnqnfe/pulse-binding-rust/issues/19
        let ctx = Rc::new(RefCell::new(
            Context::new(&main, PA_NAME).ok_or("pulseaudio: failed to create context")?,
        ));

        let counter = Arc::new(Counter {
            in_progress: AtomicUsize::new(0),
            last_total: AtomicUsize::new(0),
        });

        {
            let counter = Arc::clone(&counter);
            let ctx = Rc::clone(&ctx);

            Rc::clone(&ctx)
                .borrow_mut()
                .set_state_callback(Some(Box::new(move || {
                    if ctx.borrow().get_state() != State::Ready {
                        return;
                    }

                    let subscribe_callback = {
                        let ctx = Rc::clone(&ctx);
                        let counter = Arc::clone(&counter);

                        move |_, _, _| {
                            let counter = Arc::clone(&counter);
                            ctx.borrow_mut()
                                .introspect()
                                .get_sink_input_info_list(move |res| match res {
                                    ListResult::Item(item) => {
                                        if !item.corked {
                                            let count =
                                                counter.in_progress.fetch_add(1, Ordering::SeqCst);
                                            debug!("Partial count: {}", count);
                                        }
                                    },
                                    ListResult::End | ListResult::Error => {
                                        let count = counter.in_progress.swap(0, Ordering::SeqCst);
                                        counter.last_total.store(count, Ordering::SeqCst);
                                        debug!("Total sum: {}", count);
                                    },
                                });
                        }
                    };

                    // Subscribe to sink input events
                    {
                        let mut ctx = ctx.borrow_mut();
                        ctx.set_subscribe_callback(Some(Box::new(subscribe_callback.clone())));
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
        main.start().map_err(|err| format!("pulseaudio: {}", err))?;

        Ok(Self { counter, ctx, main })
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
        let players = self.counter.last_total.load(Ordering::SeqCst);
        if players == 0 {
            Ok(Progress::Continue)
        } else {
            Ok(Progress::Abort)
        }
    }
}
