use crate::MyError;

use std::{
    ops::{Deref, DerefMut},
    os::raw::{c_char, c_void},
    ptr
};
use x11::{
    xlib::{Display, XA_ATOM, XCloseDisplay, XDefaultRootWindow, XFree, XGetInputFocus, XGetWindowProperty,
           XInternAtom, XOpenDisplay},
    xss::{XScreenSaverInfo, XScreenSaverQueryInfo}
};

const NET_WM_STATE: &str = "_NET_WM_STATE\0";
const NET_WM_STATE_FULLSCREEN: &str = "_NET_WM_STATE_FULLSCREEN\0";

pub fn get_idle(display: *mut Display, info: *mut XScreenSaverInfo) -> Result<u64, MyError> {
    unsafe {
        if XScreenSaverQueryInfo(display, XDefaultRootWindow(display), info) == 0 {
            Err(MyError::XScreenSaver)
        } else {
            Ok((*info).idle / 1000)
        }
    }
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
enum XIntPtr {
    U8(*mut u8),
    U16(*mut u16),
    U32(*mut u32)
}
impl XIntPtr {
    unsafe fn offset(&self, i: isize) -> XIntPtr {
        match *self {
            XIntPtr::U8(ptr)  => XIntPtr::U8(ptr.offset(i)),
            XIntPtr::U16(ptr) => XIntPtr::U16(ptr.offset(i)),
            XIntPtr::U32(ptr) => XIntPtr::U32(ptr.offset(i)),
        }
    }
    unsafe fn deref_u64(&self) -> u64 {
        match *self {
            XIntPtr::U8(ptr)  => *ptr as u64,
            XIntPtr::U16(ptr) => *ptr as u64,
            XIntPtr::U32(ptr) => *ptr as u64,
        }
    }
}
impl Drop for XIntPtr {
    fn drop(&mut self) {
        unsafe {
            XFree(match *self {
                XIntPtr::U8(ptr) => ptr as *mut c_void,
                XIntPtr::U16(ptr) => ptr as *mut c_void,
                XIntPtr::U32(ptr) => ptr as *mut c_void
            });
        }
    }
}
pub unsafe fn get_fullscreen(display: *mut Display) -> Result<bool, MyError> {
    let mut focus = 0u64;
    let mut revert = 0i32;

    let mut actual_type = 0u64;
    let mut actual_format = 0i32;
    let mut nitems = 0u64;
    let mut bytes = 0u64;
    let mut data: *mut u8 = ptr::null_mut();

    // Get the focused window
    if XGetInputFocus(display, &mut focus, &mut revert) != 0 {
        return Err(MyError::XGetInputFocus);
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
        return Err(MyError::XGetWindowProperty);
    }

    let mut fullscreen = false;

    let data = match actual_format {
        8  => Some(XIntPtr::U8(data)),
        16 => Some(XIntPtr::U16(data as *mut u16)),
        32 => Some(XIntPtr::U32(data as *mut u32)),
        _  => None
    };

    let atom = XInternAtom(
        display,
        NET_WM_STATE_FULLSCREEN.as_ptr() as *const c_char,
        0
    );

    // Check the list of returned items for _NET_WM_STATE_FULLSCREEN
    for i in 0..nitems as isize {
        if data.as_ref().unwrap().offset(i).deref_u64() == atom {
            fullscreen = true;
            break;
        }
    }

    Ok(fullscreen)
}
