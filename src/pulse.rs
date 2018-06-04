use libpulse_sys::{
    context::*,
    context::pa_context,
    mainloop::threaded::*
};
use std::{
    ffi::CString,
    os::raw::c_void,
    process::abort,
    ptr,
    sync::mpsc
};

const PA_NAME: &str = "xidlehook";

pub type Sender = mpsc::Sender<usize>;

pub struct AudioCounter {
    count: usize,
    tx: Sender
}

pub struct PulseAudio {
    ctx: *mut pa_context,
    main: *mut pa_threaded_mainloop,

    // needs to be kept alive
    counter: Option<AudioCounter>,
}
impl Default for PulseAudio {
    fn default() -> Self {
        unsafe {
            let main = pa_threaded_mainloop_new();
            let name = CString::from_vec_unchecked(PA_NAME.as_bytes().to_vec());
            Self {
                main: main,
                ctx: pa_context_new(pa_threaded_mainloop_get_api(main), name.as_ptr()),

                counter: None
            }
        }
    }
}
impl PulseAudio {
    /// Start a new thread that will send events to tx.
    /// You will need to make sure that this struct is never moved around in memory after this.
    pub unsafe fn connect(&mut self, tx: Sender) {
        extern "C" fn sink_info_callback(
            _: *mut pa_context,
            info: *const pa_sink_input_info,
            _: i32,
            userdata: *mut c_void
        ) {
            unsafe {
                let counter = &mut *(userdata as *mut _ as *mut AudioCounter);
                if info.is_null() {
                    counter.tx.send(counter.count).unwrap_or_else(|_| abort());
                } else if (*info).corked == 0 {
                    counter.count += 1;
                }
            }
        }
        extern "C" fn subscribe_callback(
            ctx: *mut pa_context,
            _: pa_subscription_event_type_t,
            _: u32,
            userdata: *mut c_void
        ) {
            unsafe {
                let counter = &mut *(userdata as *mut _ as *mut AudioCounter);
                counter.count = 0;

                // You *could* keep track of events here (like making change events toggle the on/off status),
                // but it's not reliable
                pa_context_get_sink_input_info_list(ctx, Some(sink_info_callback), userdata);
            }
        }
        extern "C" fn state_callback(ctx: *mut pa_context, userdata: *mut c_void) {
            unsafe {
                let state = pa_context_get_state(ctx);

                if state == PA_CONTEXT_READY {
                    pa_context_set_subscribe_callback(ctx, Some(subscribe_callback), userdata);
                    pa_context_subscribe(ctx, PA_SUBSCRIPTION_MASK_SINK_INPUT, None, ptr::null_mut());

                    // In case audio already plays
                    pa_context_get_sink_input_info_list(ctx, Some(sink_info_callback), userdata);
                }
            }
        }

        self.counter = Some(AudioCounter {
            count: 0,
            tx: tx
        });
        let userdata = self.counter.as_mut().unwrap() as *mut _ as *mut c_void;
        pa_context_set_state_callback(self.ctx, Some(state_callback), userdata);
        pa_context_connect(self.ctx, ptr::null(), 0, ptr::null());

        pa_threaded_mainloop_start(self.main);
    }
}
impl Drop for PulseAudio {
    fn drop(&mut self) {
        unsafe {
            pa_context_disconnect(self.ctx);
            pa_threaded_mainloop_stop(self.main);
            pa_threaded_mainloop_free(self.main);
        }
    }
}
