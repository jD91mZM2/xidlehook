use crate::MyError;

use std::{
    ops::{Deref, DerefMut},
    os::raw::*,
    ptr
};
use x11::{
    xlib::{Atom, Display, XA_ATOM, XCloseDisplay, XDefaultRootWindow, XFree,
        XGetInputFocus, XGetWindowProperty, XInternAtom, XOpenDisplay},
    xss::{XScreenSaverInfo, XScreenSaverQueryInfo}
};

const NET_WM_STATE: &str = "_NET_WM_STATE\0";
const NET_WM_STATE_FULLSCREEN: &str = "_NET_WM_STATE_FULLSCREEN\0";

pub fn get_idle(display: *mut Display, info: *mut XScreenSaverInfo) -> Result<u64, MyError> {
    unsafe {
        if XScreenSaverQueryInfo(display, XDefaultRootWindow(display), info) == 0 {
            Err(MyError::XScreenSaver)
        } else {
            Ok((*info).idle)
        }
    }
}
pub fn get_idle_seconds(display: *mut Display, info: *mut XScreenSaverInfo) -> Result<u64, MyError> {
    get_idle(display, info).map(|i| i / 1000)
}

pub struct XDisplay(*mut Display);
impl XDisplay {
    pub fn new() -> Result<Self, MyError> {
        unsafe {
            let ptr = XOpenDisplay(ptr::null());
            if ptr.is_null() {
                Err(MyError::XDisplay)
            } else {
                Ok(XDisplay(ptr))
            }
        }
    }
}
impl Deref for XDisplay {
    type Target = *mut Display;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for XDisplay {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for XDisplay {
    fn drop(&mut self) {
        unsafe {
            XCloseDisplay(self.0);
        }
    }
}

pub struct XPtr<T>(*mut T);
impl<T> XPtr<T> {
    pub unsafe fn new(ptr: *mut T) -> Self {
        XPtr(ptr)
    }
}
impl<T> Deref for XPtr<T> {
    type Target = *mut T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> DerefMut for XPtr<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<T> Drop for XPtr<T> {
    fn drop(&mut self) {
        unsafe {
            XFree(self.0 as *mut c_void);
        }
    }
}

#[derive(Clone, Debug)]
struct XIntPtr(*mut Atom);
impl Drop for XIntPtr {
    fn drop(&mut self) {
        unsafe {
            XFree(self.0 as *mut c_void);
        }
    }
}
pub unsafe fn get_fullscreen(display: *mut Display) -> Result<bool, MyError> {
    let ignored_int = &mut 0;
    let ignored_ulong = &mut 0;

    // Get the focused window
    let mut focus = 0;
    XGetInputFocus(display, &mut focus, ignored_int);

    // Get the window properties of said window
    let mut data: *mut u8 = ptr::null_mut();
    let mut nitems = 0;
    XGetWindowProperty(
        display,
        focus,
        XInternAtom(display, NET_WM_STATE.as_ptr() as *const c_char, 0),
        0,
        c_long::max_value(),
        0,
        XA_ATOM,
        ignored_ulong,
        ignored_int,
        &mut nitems,
        ignored_ulong,
        &mut data
    );

    let data = XIntPtr(data as *mut _);
    let atom = XInternAtom(
        display,
        NET_WM_STATE_FULLSCREEN.as_ptr() as *const c_char,
        0
    );

    // Check the list of returned items for _NET_WM_STATE_FULLSCREEN
    for i in 0..nitems as isize {
        if *data.0.offset(i) == atom {
            return Ok(true);
        }
    }
    Ok(false)
}
