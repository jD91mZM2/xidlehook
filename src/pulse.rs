use futures::sync::mpsc;
use libpulse_sys::context::*;
use libpulse_sys::context::pa_context;
use libpulse_sys::mainloop::threaded::*;
use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;

const PA_NAME: &str = "xidlehook";

#[derive(Clone, Copy)]
pub enum Event {
    Clear,
    New,
    Finish
}
pub type Sender = mpsc::UnboundedSender<Event>;

pub struct PulseAudio {
    main: *mut pa_threaded_mainloop,
    ctx: *mut pa_context,

    // needs to be kept alive
    tx: Option<Sender>
}
impl Default for PulseAudio {
    fn default() -> Self {
        unsafe {
            let main = pa_threaded_mainloop_new();
            let name = CString::from_vec_unchecked(PA_NAME.as_bytes().to_vec());
            Self {
                main: main,
                ctx: pa_context_new(pa_threaded_mainloop_get_api(main), name.as_ptr()),

                tx: None
            }
        }
    }
}
impl PulseAudio {
    pub fn connect(&mut self, tx: Sender) {
        extern "C" fn sink_info_callback(
            _: *mut pa_context,
            info: *const pa_sink_input_info,
            _: i32,
            userdata: *mut c_void
        ) {
            unsafe {
                let tx = userdata as *mut _ as *mut Sender;
                if info.is_null() {
                    (&*tx).unbounded_send(Event::Finish).unwrap();
                } else if (*info).corked == 0 {
                    (&*tx).unbounded_send(Event::New).unwrap();
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
                let tx = userdata as *mut _ as *mut Sender;
                (&*tx).unbounded_send(Event::Clear).unwrap();

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

        self.tx = Some(tx);
        let userdata = self.tx.as_mut().unwrap() as *mut _ as *mut c_void;
        unsafe {
            pa_context_set_state_callback(self.ctx, Some(state_callback), userdata);
            pa_context_connect(self.ctx, ptr::null(), 0, ptr::null());

            pa_threaded_mainloop_start(self.main);
        }
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
