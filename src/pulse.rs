use libpulse_sys::mainloop::standard::*;
use libpulse_sys::operation::pa_operation;
use libpulse_sys::subscribe::pa_subscription_mask_t;
use std::ffi::CString;
use std::ops::Deref;
use std::ptr;

pub use libpulse_sys::context::*;
pub use libpulse_sys::context::subscription_masks::*;
pub use libpulse_sys::context::subscribe::event_facilities::*;
pub use libpulse_sys::context::subscribe::event_operations::*;

const PA_NAME: &str = "xidlehook";

pub struct PulseAudioContext(pub *mut pa_context);
pub struct PulseAudio {
    main: *mut pa_mainloop,
    context: PulseAudioContext
}

impl Default for PulseAudio {
    fn default() -> Self {
        unsafe {
            let main = pa_mainloop_new();
            let name = CString::from_vec_unchecked(PA_NAME.as_bytes().to_vec());
            Self {
                main: main,
                context: PulseAudioContext(pa_context_new(pa_mainloop_get_api(main), name.as_ptr()))
            }
        }
    }
}
impl Deref for PulseAudio {
    type Target = PulseAudioContext;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}
impl PulseAudio {
    /// Construct a new PulseAudio object. Same as `PulseAudio::default`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start the main loop
    pub fn run(&self) -> bool {
        let mut retval = 0;
        let val = unsafe { pa_mainloop_run(self.main, &mut retval) };
        println!("retval: {}", retval);
        val < 0
    }
}
impl PulseAudioContext {
    /// Set state callback
    pub fn set_state_callback(&self, callback: pa_context_notify_cb_t) {
        unsafe { pa_context_set_state_callback(self.0, callback, ptr::null_mut()) }
    }
    /// Connect to the server. Use set_state_callback to wait until the connection is completed.
    pub fn connect(&self) {
        unsafe { pa_context_connect(self.0, ptr::null(), 0, ptr::null()); }
    }
    /// Get the state from a context
    pub fn get_state(&self) -> pa_context_state_t {
        unsafe { pa_context_get_state(self.0) }
    }

    /// Set callback to receive events in
    pub fn set_subscribe_callback(&self, callback: pa_context_subscribe_cb_t) {
        unsafe { pa_context_set_subscribe_callback(self.0, callback, ptr::null_mut()) }
    }
    /// Subscribe to a PulseAudio event, calling value in `set_subscribe_callback`
    pub fn subscribe(&self, mask: pa_subscription_mask_t, callback: pa_context_success_cb_t) -> *mut pa_operation {
        unsafe { pa_context_subscribe(self.0, mask, callback, ptr::null_mut()) }
    }
}
impl Drop for PulseAudio {
    fn drop(&mut self) {
        unsafe {
            pa_mainloop_free(self.main);
            pa_context_disconnect(self.context.0);
        }
    }
}
