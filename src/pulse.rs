use libpulse_sys::context::*;
use libpulse_sys::context::pa_context;
use libpulse_sys::mainloop::threaded::*;
use std::ffi::CString;

const PA_NAME: &str = "xidlehook";

pub enum PulseEvent {
    Clear,
    New,
    Finish
}
pub struct PulseAudio {
    pub main: *mut pa_threaded_mainloop,
    pub ctx: *mut pa_context
}

impl Default for PulseAudio {
    fn default() -> Self {
        unsafe {
            let main = pa_threaded_mainloop_new();
            let name = CString::from_vec_unchecked(PA_NAME.as_bytes().to_vec());
            Self {
                main: main,
                ctx: pa_context_new(pa_threaded_mainloop_get_api(main), name.as_ptr())
            }
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
