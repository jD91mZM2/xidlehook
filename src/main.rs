#[macro_use] extern crate clap;
extern crate x11;

use clap::{App, Arg};
use std::os::raw::c_void;
use std::process::Command;
use std::time::Duration;
use std::{ptr, thread};
use x11::xlib::{Display, XCloseDisplay, XDefaultRootWindow, XFree, XOpenDisplay};
use x11::xss::{XScreenSaverAllocInfo, XScreenSaverQueryInfo};

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
    let matches = App::new(crate_name!())
        .author(crate_authors!())
        .version(crate_version!())
        .arg(
            Arg::with_name("print")
                .help("Prints the idle time to standard output. This is similar to xprintidle.")
                .long("print")
        )
        .arg(
            Arg::with_name("time")
                .help("Sets the required amount of idle minutes before executing command")
                .long("time")
                .required_unless("print")
                .requires("timer")
                .takes_value(true)
        )
        .arg(
            Arg::with_name("timer")
                .help("Sets command to run when timer goes off")
                .long("timer")
                .requires("time")
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
        return;
    }
    let _cleanup = DeferXClose(display);

    if matches.is_present("print") {
        if let Ok(idle) = get_idle(display) {
            println!("{}", idle);
        }
        return;
    }

    let time     = value_t_or_exit!(matches, "time", u32) as u64 * SCALE;
    let timer    = matches.value_of("timer").unwrap();
    let notify   = value_t!(matches, "notify", u32).ok().map(|notify| notify as u64);
    let notifier = matches.value_of("notifier");
    let canceller = matches.value_of("canceller");

    let mut ran_timer  = false;
    let mut ran_notify = false;

    let default_delay = Duration::from_secs(SCALE as u64);
    let mut delay = default_delay;

    let mut time_new = time;

    loop {
        let idle = match get_idle(display) {
            Ok(idle) => idle,
            Err(_) => return
        };

        let idle = idle / 1000; // Convert to seconds

        if notify.map(|notify| (idle + notify) >= time).unwrap_or(true) {
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
        } else {
            if ran_notify && !ran_timer {
                // In case the user goes back from being idle between the notify and timer
                if let Some(canceller) = canceller {
                    invoke(&canceller);
                }
            }
            delay = Duration::from_secs(SCALE);
            ran_notify = false;
            ran_timer  = false;
            time_new = time;
        }

        thread::sleep(delay);
    }
}
fn get_idle(display: *mut Display) -> Result<u64, ()> {
    let info = unsafe { XScreenSaverAllocInfo() };
    let _cleanup = DeferXFree(info as *mut c_void);

    if unsafe { XScreenSaverQueryInfo(display, XDefaultRootWindow(display), info) } != 1 {
        eprintln!("failed to query screen saver info");
        return Err(());
    }

    Ok(unsafe { (*info).idle })
}
fn invoke(cmd: &str) {
    if let Err(err) =
        Command::new("bash")
            .arg("-c")
            .arg(cmd)
            .status() {
        eprintln!("failed to invoke command: {}", err);
    }
}
