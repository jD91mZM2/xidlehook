use crate::MyError;

use std::slice;

const NET_WM_STATE: &str = "_NET_WM_STATE";
const NET_WM_STATE_FULLSCREEN: &str = "_NET_WM_STATE_FULLSCREEN";

pub struct Xcb {
    conn: xcb::Connection,
    root_window: xcb::Window,
    atom_net_wm_state: xcb::Atom,
    atom_net_wm_state_fullscreen: xcb::Atom,
}
impl Xcb {
    pub fn new() -> Result<Self, MyError> {
        let (conn, _) = xcb::Connection::connect(None).map_err(MyError::XcbConnError)?;

        let setup = conn.get_setup();
        let screen = setup.roots().next().ok_or(MyError::XcbNoRoot)?;
        let root_window = screen.root();

        let atom_net_wm_state = xcb::xproto::intern_atom(&conn, false, NET_WM_STATE).get_reply()?.atom();
        let atom_net_wm_state_fullscreen = xcb::xproto::intern_atom(&conn, false, NET_WM_STATE_FULLSCREEN).get_reply()?.atom();

        Ok(Self {
            conn,
            root_window,
            atom_net_wm_state,
            atom_net_wm_state_fullscreen,
        })
    }
    pub fn get_idle(&self) -> Result<u32, MyError> {
        let info = xcb::screensaver::query_info(&self.conn, self.root_window).get_reply()?;
        Ok(info.ms_since_user_input())
    }
    pub fn get_idle_seconds(&self) -> Result<u32, MyError> {
        self.get_idle().map(|i| i / 1000)
    }
    pub fn get_fullscreen(&self) -> Result<bool, MyError> {
        let focused_window = xcb::xproto::get_input_focus(&self.conn).get_reply()?.focus();
        let prop = xcb::xproto::get_property(
            &self.conn, // c
            false, // delete
            focused_window, // window
            self.atom_net_wm_state, // property
            xcb::xproto::ATOM_ATOM, // type_
            0, // long_offset
            u32::max_value() // long_length
        ).get_reply()?;

        // The safe API can't possibly know what value xcb returned,
        // sadly. Here we are manually transmuting &[c_void] to
        // &[Atom], as we specified we want an atom.
        let value = dbg!(prop.value());
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
}
