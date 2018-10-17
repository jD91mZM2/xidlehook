use failure::Error;
use std::{ptr, os::raw::{c_char, c_void}};
use x11::{
    xlib::{Display, XA_ATOM, XDefaultRootWindow, XFree, XGetInputFocus, XGetWindowProperty,
           XInternAtom},
    xss::{XScreenSaverInfo, XScreenSaverQueryInfo}
};

const NET_WM_STATE: &str = "_NET_WM_STATE\0";
const NET_WM_STATE_FULLSCREEN: &str = "_NET_WM_STATE_FULLSCREEN\0";

pub fn get_idle(display: *mut Display, info: *mut XScreenSaverInfo) -> Result<u64, Error> {
    if unsafe { XScreenSaverQueryInfo(display, XDefaultRootWindow(display), info) } == 0 {
        bail!("failed to query screen saver info");
    }

    Ok(unsafe { (*info).idle })
}

#[derive(Clone, Copy, Debug)]
enum Ptr {
    U8(*mut u8),
    U16(*mut u16),
    U32(*mut u32)
}
impl Ptr {
    unsafe fn offset(self, i: isize) -> Ptr {
        match self {
            Ptr::U8(ptr)  => Ptr::U8(ptr.offset(i)),
            Ptr::U16(ptr) => Ptr::U16(ptr.offset(i)),
            Ptr::U32(ptr) => Ptr::U32(ptr.offset(i)),
        }
    }
    unsafe fn deref_u64(self) -> u64 {
        match self {
            Ptr::U8(ptr)  => *ptr as u64,
            Ptr::U16(ptr) => *ptr as u64,
            Ptr::U32(ptr) => *ptr as u64,
        }
    }
}
pub unsafe fn get_fullscreen(display: *mut Display) -> Result<bool, Error> {
    let mut focus = 0u64;
    let mut revert = 0i32;

    let mut actual_type = 0u64;
    let mut actual_format = 0i32;
    let mut nitems = 0u64;
    let mut bytes = 0u64;
    let mut data: *mut u8 = ptr::null_mut();

    // Get the focused window
    if XGetInputFocus(display, &mut focus, &mut revert) != 0 {
        bail!("failed to get input focus");
    }

    // Get the window properties of said window
    if XGetWindowProperty(
        display,
        focus,
        XInternAtom(display, NET_WM_STATE.as_ptr() as *const c_char, 0),
        0,
        !0,
        0,
        XA_ATOM,
        &mut actual_type,
        &mut actual_format,
        &mut nitems,
        &mut bytes,
        &mut data
    ) != 0 {
        bail!("failed to get window property");
    }

    let mut fullscreen = false;

    let data_enum = match actual_format {
        8  => Some(Ptr::U8(data)),
        16 => Some(Ptr::U16(data as *mut u16)),
        32 => Some(Ptr::U32(data as *mut u32)),
        _  => None
    };

    let atom = XInternAtom(
        display,
        NET_WM_STATE_FULLSCREEN.as_ptr() as *const c_char,
        0
    );

    // Check the list of returned items for _NET_WM_STATE_FULLSCREEN
    for i in 0..nitems as isize {
        if data_enum.unwrap().offset(i).deref_u64() == atom {
            fullscreen = true;
            break;
        }
    }

    XFree(data as *mut c_void);

    Ok(fullscreen)
}
