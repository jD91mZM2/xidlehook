//! Various X-related utilities. The `Xcb` object must be used
//! regardless of whether or not you want to use `NotWhenAudio` - it's
//! xidlehook's simple way to obtain the idle time. The
//! `NotWhenFullscreen` module is used to implement
//! `--not-when-fullscreen` in the example client.

use crate::{Module, Progress, Result, TimerInfo};

use std::{fmt, rc::Rc, slice, time::Duration};

use log::debug;

const NET_WM_STATE: &str = "_NET_WM_STATE";
const NET_WM_STATE_FULLSCREEN: &str = "_NET_WM_STATE_FULLSCREEN";

/// See the crate-level documentation
pub struct Xcb {
    conn: xcb::Connection,
    root_window: xcb::Window,
    atom_net_wm_state: xcb::Atom,
    atom_net_wm_state_fullscreen: xcb::Atom,
}
impl Xcb {
    /// Initialize all the things, like setting up an X connection.
    pub fn new() -> Result<Self> {
        let (conn, _) = xcb::Connection::connect(None)?;

        let setup = conn.get_setup();
        let screen = setup.roots().next().ok_or("no xcb root")?;
        let root_window = screen.root();

        let atom_net_wm_state = xcb::xproto::intern_atom(&conn, false, NET_WM_STATE)
            .get_reply()?
            .atom();
        let atom_net_wm_state_fullscreen =
            xcb::xproto::intern_atom(&conn, false, NET_WM_STATE_FULLSCREEN)
                .get_reply()?
                .atom();

        Ok(Self {
            conn,
            root_window,
            atom_net_wm_state,
            atom_net_wm_state_fullscreen,
        })
    }
    /// Get the user's idle time using the `XScreenSaver` plugin
    pub fn get_idle(&self) -> Result<Duration> {
        let info = xcb::screensaver::query_info(&self.conn, self.root_window).get_reply()?;
        Ok(Duration::from_millis(info.ms_since_user_input().into()))
    }
    /// Get whether or not the user's currently active window is
    /// fullscreen
    pub fn get_fullscreen(&self) -> Result<bool> {
        let focused_window = xcb::xproto::get_input_focus(&self.conn)
            .get_reply()?
            .focus();
        let prop = xcb::xproto::get_property(
            &self.conn,             // c
            false,                  // delete
            focused_window,         // window
            self.atom_net_wm_state, // property
            xcb::xproto::ATOM_ATOM, // type_
            0,                      // long_offset
            u32::max_value(),       // long_length
        )
        .get_reply()?;

        // The safe API can't possibly know what value xcb returned,
        // sadly. Here we are manually transmuting &[c_void] to
        // &[Atom], as we specified we want an atom.
        let value = prop.value();

        debug!("xcb::xproto::get_property(...) = {:?}", value);
        debug!(
            "NET_WM_STATE_FULLSCREEN = {:?}",
            self.atom_net_wm_state_fullscreen
        );

        let value = unsafe {
            slice::from_raw_parts(value.as_ptr() as *const xcb::xproto::Atom, value.len())
        };

        for &atom in value {
            if atom == self.atom_net_wm_state_fullscreen {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Return a `NotWhenFullscreen` instance for a reference-counted
    /// self
    pub fn not_when_fullscreen(self: Rc<Self>) -> NotWhenFullscreen {
        NotWhenFullscreen { xcb: self }
    }
}
impl fmt::Debug for Xcb {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Xcb")
    }
}

/// See the module-level documentation
pub struct NotWhenFullscreen {
    xcb: Rc<Xcb>,
}
impl Module for NotWhenFullscreen {
    fn pre_timer(&mut self, _timer: TimerInfo) -> Result<Progress> {
        self.xcb.get_fullscreen().map(|fullscreen| {
            if fullscreen {
                Progress::Abort
            } else {
                Progress::Continue
            }
        })
    }
}
impl fmt::Debug for NotWhenFullscreen {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NotWhenFullscreen")
    }
}
