use libpulse_binding::{
    callbacks::ListResult,
    context::{subscribe::Facility, Context, State},
    error::PAErr,
    mainloop::threaded::Mainloop
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc
};

const PA_NAME: &str = "xidlehook";

pub type Sender = mpsc::Sender<usize>;

pub struct AudioCounter {
    count: Cell<usize>,
    tx: Sender
}

pub struct PulseAudio {
    ctx: Rc<RefCell<Context>>,
    main: Mainloop,
}
impl PulseAudio {
    /// Create a new pulseaudio main loop, but don't connect it yet
    pub fn new() -> Option<Self> {
        let main = Mainloop::new()?;
        Some(Self {
            ctx: Rc::new(RefCell::new(Context::new(&main, PA_NAME)?)),
            main
        })
    }

    /// Start a new thread that will send a count of audio devices to
    /// tx after each change event
    pub fn connect(&mut self, tx: Sender) -> Result<(), PAErr> {
        let counter = Rc::new(AudioCounter {
            count: Cell::new(0),
            tx
        });

        let ctx = Rc::clone(&self.ctx);

        self.ctx.borrow_mut().set_state_callback(Some(Box::new(move || {
            if ctx.borrow().get_state() != State::Ready {
                return;
            }

            let subscribe_callback = {
                let ctx = Rc::clone(&ctx);
                let counter = Rc::clone(&counter);

                move |_, _, _| {
                    let counter = Rc::clone(&counter);
                    ctx.borrow().introspect().get_sink_input_info_list(move |res| match res {
                        ListResult::Item(item) => if !item.corked {
                            counter.count.set(counter.count.get() + 1);
                        },
                        ListResult::End | ListResult::Error => {
                            counter.tx.send(counter.count.replace(0)).unwrap();
                        }
                    });
                }
            };

            // Subscribe to sink input events
            ctx.borrow_mut().set_subscribe_callback(Some(Box::new(subscribe_callback.clone())));
            ctx.borrow_mut().subscribe(Facility::SinkInput.to_interest_mask(), |_| ());

            // In case audio already plays, trigger
            subscribe_callback(None, None, 0);
        })));

        // We sadly can't use borrow_mut here because that keeps a
        // mutable reference alive while it's runnig all the
        // callbacks, leading to mutability errors there. See
        // https://github.com/jnqnfe/pulse-binding-rust/issues/19.
        unsafe { &mut *self.ctx.as_ptr() }.connect(None, 0, None)?;
        self.main.start()
    }
}
impl Drop for PulseAudio {
    fn drop(&mut self) {
        // See note above
        unsafe { &mut *self.ctx.as_ptr() }.disconnect();
        self.main.stop();
    }
}
