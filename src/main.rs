#[macro_use] extern crate clap;
extern crate x11;

use clap::{App, Arg};
use std::ffi::CString;
use std::os::raw::c_void;
use std::process::Command;
use std::time::Duration;
use std::{ptr, thread};
use x11::xlib::{Display, XA_ATOM, XCloseDisplay, XDefaultRootWindow, XFree, XGetInputFocus, XGetWindowProperty,
                XInternAtom, XOpenDisplay};
use x11::xss::{XScreenSaverAllocInfo, XScreenSaverInfo, XScreenSaverQueryInfo};

struct DeferXClose(*mut Display);
impl Drop for DeferXClose {
    fn drop(&mut self) {
        unsafe { XCloseDisplay(self.0); }
    }
}
struct DeferXFree(*mut c_void);
impl Drop for DeferXFree {
    fn drop(&mut self) {
        unsafe { XFree(self.0); }
    }
}

const SCALE: u64 = 60; // Second:minute scale. Can be changed for debugging purposes.

fn main() {
    let success = do_main();
    std::process::exit(if success { 0 } else { 1 });
}
fn do_main() -> bool {
    let matches = App::new(crate_name!())
        .author(crate_authors!())
        .version(crate_version!())
        .arg(
            Arg::with_name("print")
                .help("Prints the idle time to standard output. This is similar to xprintidle.")
                .long("print")
        )
        .arg(
            Arg::with_name("not-when-fullscreen")
                .help("Don't call the timer when the current application is fullscreen. \
                       Useful for preventing the lockscreen when watching videos")
                .long("not-when-fullscreen")
        )
        .arg(
            Arg::with_name("time")
                .help("Sets the required amount of idle minutes before executing command")
                .long("time")
                .required_unless("print")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("timer")
                .help("Sets command to run when timer goes off")
                .long("timer")
                .required_unless("print")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("notify")
                .help("Runs the command passed by --notifier _ seconds before timer goes off")
                .long("notify")
                .requires("notifier")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("notifier")
                .help("Sets the command to run when notifier goes off (see --notify)")
                .long("notifier")
                .requires("notify")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("canceller")
                .help("Sets the command to run when user prevents the timer after the notifier has already gone off")
                .long("canceller")
                .requires("notify")
                .takes_value(true)
        )
        .get_matches();

    let display = unsafe { XOpenDisplay(ptr::null()) };
    if display.is_null() {
        eprintln!("failed to open x server");
        return false;
    }
    let _cleanup = DeferXClose(display);

    let info = unsafe { XScreenSaverAllocInfo() };
    let _cleanup = DeferXFree(info as *mut c_void);

    if matches.is_present("print") {
        if let Ok(idle) = get_idle(display, info) {
            println!("{}", idle);
        }
        return true;
    }

    let not_when_fullscreen = matches.is_present("not-when-fullscreen");
    let time     = value_t_or_exit!(matches, "time", u32) as u64 * SCALE;
    let timer    = matches.value_of("timer").unwrap();
    let notify   = value_t!(matches, "notify", u32).ok().map(|notify| notify as u64);
    let notifier = matches.value_of("notifier");
    let canceller = matches.value_of("canceller");

    let mut ran_notify = false;
    let mut ran_timer  = false;

    let mut fullscreen = None;

    let default_delay = Duration::from_secs(SCALE as u64);
    let mut delay = default_delay;

    let mut time_new = time;

    loop {
        let idle = match get_idle(display, info) {
            Ok(idle) => idle,
            Err(_) => return false
        };

        let idle = idle / 1000; // Convert to seconds

        if notify.map(|notify| (idle + notify) >= time).unwrap_or(idle >= time) {
            if not_when_fullscreen && fullscreen.is_none() {
                let mut focus = 0u64;
                let mut revert = 0i32;

                let mut actual_type = 0u64;
                let mut actual_format = 0i32;
                let mut nitems = 0u64;
                let mut bytes = 0u64;
                let mut data: *mut u8 = unsafe { std::mem::uninitialized() };

                fullscreen = Some(unsafe {
                    XGetInputFocus(display, &mut focus as *mut _, &mut revert as *mut _);

                    let cstring = CString::from_vec_unchecked("_NET_WM_STATE".into());

                    if XGetWindowProperty(
                        display,
                        focus,
                        XInternAtom(display, cstring.as_ptr(), 0),
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
                        eprintln!("failed to get window property");
                        false
                    } else {
                        // Welcome to hell.
                        // I spent waay to long trying to get `data` to work.
                        // Currently it returns 75, because it overflows 331 to fit into a byte.
                        // Changing `data` to a *mut u64 gives me 210453397504.
                        // I have no idea why, and at this point I don't want to know.
                        // So here I'll just compare it to 75 and assume fullscreen.

                        let mut fullscreen = false;

                        for i in 0..nitems as isize {
                            let cstring = CString::from_vec_unchecked("_NET_WM_STATE_FULLSCREEN".into());
                            if *data.offset(i) == (XInternAtom(display, cstring.as_ptr(), 0) & 0xFF) as u8 {
                                fullscreen = true;
                                break;
                            }
                        }

                        XFree(data as *mut c_void);

                        fullscreen
                    }
                });
            }
            if !not_when_fullscreen || !fullscreen.unwrap() {
                if notify.is_some() && !ran_notify {
                    invoke(&notifier.unwrap());

                    ran_notify = true;
                    delay = Duration::from_secs(1);

                    // Since the delay is usually a minute, I could've exceeded both the notify and the timer.
                    // The simple solution is to change the timer to a point where it's guaranteed
                    // it's been _ seconds since the notifier.
                    time_new = idle + notify.unwrap();
                } else if idle >= time_new && !ran_timer {
                    invoke(&timer);

                    ran_timer = true;
                    delay = default_delay;
                }
            }
        } else {
            if ran_notify && !ran_timer {
                // In case the user goes back from being idle between the notify and timer
                if let Some(canceller) = canceller {
                    invoke(&canceller);
                }
            }
            delay = default_delay;
            ran_notify = false;
            ran_timer  = false;
            fullscreen = None;
        }

        thread::sleep(delay);
    }
}
fn get_idle(display: *mut Display, info: *mut XScreenSaverInfo) -> Result<u64, ()> {
    if unsafe { XScreenSaverQueryInfo(display, XDefaultRootWindow(display), info) } == 0 {
        eprintln!("failed to query screen saver info");
        return Err(());
    }

    Ok(unsafe { (*info).idle })
}
fn invoke(cmd: &str) {
    if let Err(err) =
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .status() {
        eprintln!("failed to invoke command: {}", err);
    }
}
