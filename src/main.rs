#[cfg(feature = "pulse")] extern crate libpulse_sys;
#[cfg(feature = "tokio")] extern crate ctrlc;
#[cfg(feature = "tokio")] extern crate futures;
#[cfg(feature = "tokio")] extern crate tokio_core;
#[cfg(feature = "tokio")] extern crate tokio_io;
#[cfg(feature = "tokio")] extern crate tokio_uds;
#[macro_use] extern crate clap;
extern crate x11;

#[cfg(feature = "pulse")] mod pulse;

#[cfg(any(feature = "pulse", not(feature = "tokio")))] use std::thread;
#[cfg(feature = "pulse")] use libpulse_sys::context::pa_context;
#[cfg(feature = "pulse")] use libpulse_sys::context::subscribe::pa_subscription_event_type_t;
#[cfg(feature = "pulse")] use pulse::PulseAudio;
#[cfg(feature = "tokio")] use futures::future::{self, Loop};
#[cfg(feature = "tokio")] use futures::sync::mpsc;
#[cfg(feature = "tokio")] use futures::{Future, Sink, Stream};
#[cfg(feature = "tokio")] use std::cell::RefCell;
#[cfg(feature = "tokio")] use std::fs;
#[cfg(feature = "tokio")] use std::io::ErrorKind;
#[cfg(feature = "tokio")] use std::rc::Rc;
#[cfg(feature = "tokio")] use tokio_core::reactor::{Core, Timeout};
#[cfg(feature = "tokio")] use tokio_io::io;
#[cfg(feature = "tokio")] use tokio_uds::UnixListener;
use clap::{App as ClapApp, Arg};
use std::ffi::CString;
use std::os::raw::c_void;
use std::process::Command;
use std::ptr;
use std::time::Duration;
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

#[cfg(feature = "tokio")] const COMMAND_DEACTIVATE: u8 = 0;
#[cfg(feature = "tokio")] const COMMAND_ACTIVATE:   u8 = 1;
#[cfg(feature = "tokio")] const COMMAND_TRIGGER:    u8 = 2;

fn main() {
    let success = do_main();
    std::process::exit(if success { 0 } else { 1 });
}
fn do_main() -> bool {
    let clap_app = ClapApp::new(crate_name!())
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
        );
    #[cfg(feature = "tokio")]
    let mut clap_app = clap_app; // make mutable
    #[cfg(feature = "tokio")] {
        clap_app = clap_app
            .arg(
                Arg::with_name("socket")
                    .help("Listens to events over a unix socket")
                    .long("socket")
                    .takes_value(true)
            );
    }
    #[cfg(feature = "pulse")] {
        clap_app = clap_app
            .arg(
                Arg::with_name("not-when-audio")
                    .help("Doesn't lock when anything is playing on PulseAudio")
                    .long("not-when-audio")
            );
    }
    let matches = clap_app.get_matches();

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

    let time = value_t_or_exit!(matches, "time", u32) as u64 * SCALE;
    let app = App {
        active: true,
        delay: Duration::from_secs(SCALE),

        display: display,
        info: info,

        not_when_fullscreen: matches.is_present("not-when-fullscreen"),
        time: time,
        timer: matches.value_of("timer").unwrap().to_string(),
        notify: value_t!(matches, "notify", u32).ok().map(|notify| notify as u64),
        notifier: matches.value_of("notifier").map(String::from),
        canceller: matches.value_of("canceller").map(String::from),

        ran_notify: false,
        ran_timer:  false,

        fullscreen: None,

        time_new: time
    };

    #[cfg(not(feature = "tokio"))]
    {
        let mut app = app;
        loop {
            if !app.step() {
                return false;
            }

            thread::sleep(app.delay);
        }
    }
    #[cfg(feature = "tokio")]
    {
        #[cfg(feature = "pulse")]
        let not_when_audio  = matches.is_present("not-when-audio");

        let socket = matches.value_of("socket");
        let app = Rc::new(RefCell::new(app));

        let mut core = Core::new().unwrap();
        let handle = Rc::new(core.handle());

        let (tx_ctrlc, rx_ctrlc) = mpsc::channel(1);
        let tx_ctrlc = RefCell::new(Some(tx_ctrlc));

        if let Err(err) =
            ctrlc::set_handler(move || {
                if let Some(tx_ctrlc) = tx_ctrlc.borrow_mut().take() {
                    tx_ctrlc.send(()).wait().unwrap();
                }
            }) {
            eprintln!("failed to create signal handler: {}", err);
        }

        if let Some(socket) = socket {
            let listener = match UnixListener::bind(socket, &handle) {
                Ok(listener) => listener,
                Err(err) => {
                    eprintln!("failed to bind unix socket: {}", err);
                    return false;
                }
            };

            let app = Rc::clone(&app);
            let handle_clone = Rc::clone(&handle);

            handle.spawn(listener.incoming()
                .map_err(|err| eprintln!("listener error: {}", err))
                .for_each(move |(conn, _)| {
                    let app = Rc::clone(&app);
                    handle_clone.spawn(future::loop_fn(conn, move |conn| {
                        let app = Rc::clone(&app);
                        io::read_exact(conn, [0; 1])
                            .map_err(|err| {
                                if err.kind() != ErrorKind::UnexpectedEof {
                                    eprintln!("io error: {}", err);
                                }
                            })
                            .and_then(move |(conn, buf)| {
                                match buf[0] {
                                    COMMAND_ACTIVATE   => app.borrow_mut().active = true,
                                    COMMAND_DEACTIVATE => app.borrow_mut().active = false,
                                    COMMAND_TRIGGER    => app.borrow().trigger(),
                                    x => eprintln!("unix socket: invalid command: {}", x)
                                }
                                Ok(Loop::Continue(conn))
                            })
                    }));
                    Ok(())
                }))
        }
        #[cfg(feature = "pulse")] {
            if not_when_audio {
                thread::spawn(move || {
                    let pulse = PulseAudio::new();

                    extern "C" fn callback1(_: *mut pa_context, event: pa_subscription_event_type_t, i: u32, _: *mut c_void) {
                        if event & pulse::PA_SUBSCRIPTION_EVENT_CHANGE == pulse::PA_SUBSCRIPTION_EVENT_CHANGE {
                            println!("Audio: Change");
                        } else if event & pulse::PA_SUBSCRIPTION_EVENT_REMOVE == pulse::PA_SUBSCRIPTION_EVENT_REMOVE {
                            println!("Audio: Remove");
                        } else if event & pulse::PA_SUBSCRIPTION_EVENT_NEW == pulse::PA_SUBSCRIPTION_EVENT_NEW {
                            println!("Audio: New");
                        }
                        println!("Event: {}; i: {}", event, i);
                    }
                    extern "C" fn callback2(context: *mut pa_context, _: *mut c_void) {
                        let context = pulse::PulseAudioContext(context);
                        let state = context.get_state();

                        if state == pulse::PA_CONTEXT_READY {
                            println!("Ready!");
                            context.set_subscribe_callback(Some(callback1));
                            context.subscribe(pulse::PA_SUBSCRIPTION_MASK_SINK_INPUT, None);
                        }
                    }
                    pulse.set_state_callback(Some(callback2));
                    pulse.connect();

                    pulse.run();
                });
            }
        }

        let handle_clone = Rc::clone(&handle);

        handle.spawn(future::loop_fn((), move |_| {
            let app = Rc::clone(&app);
            let delay = app.borrow().delay;
            Timeout::new(delay, &handle_clone)
                .unwrap()
                .map_err(|_| ())
                .and_then(move |_| {
                    if app.borrow_mut().step() {
                        Ok(Loop::Continue(()))
                    } else {
                        Err(())
                    }
                })
        }));

        let status = core.run(rx_ctrlc.into_future()).is_ok();

        if let Some(socket) = socket {
            if let Err(err) = fs::remove_file(socket) {
                eprintln!("failed to clean up unix socket: {}", err);
            }
        }

        status
    }
}
struct App {
    active: bool,
    delay: Duration,

    display: *mut Display,
    info: *mut XScreenSaverInfo,

    not_when_fullscreen: bool,
    time: u64,
    timer: String,
    notify: Option<u64>,
    notifier: Option<String>,
    canceller: Option<String>,

    ran_notify: bool,
    ran_timer: bool,

    fullscreen: Option<bool>,

    time_new: u64
}
impl App {
    fn step(&mut self) -> bool {
        let default_delay = Duration::from_secs(SCALE); // TODO: const fn

        let idle = if self.active {
            Some(match get_idle(self.display, self.info) {
                Ok(idle) => idle / 1000, // Convert to seconds
                Err(_) => return false
            })
        } else { None };

        if self.active &&
            self.notify.map(|notify| (idle.unwrap() + notify) >= self.time).unwrap_or(idle.unwrap() >= self.time) {
            let idle = idle.unwrap();
            if self.not_when_fullscreen && self.fullscreen.is_none() {
                let mut focus = 0u64;
                let mut revert = 0i32;

                let mut actual_type = 0u64;
                let mut actual_format = 0i32;
                let mut nitems = 0u64;
                let mut bytes = 0u64;
                let mut data: *mut u8 = unsafe { std::mem::uninitialized() };

                self.fullscreen = Some(unsafe {
                    XGetInputFocus(self.display, &mut focus as *mut _, &mut revert as *mut _);

                    let cstring = CString::from_vec_unchecked("_NET_WM_STATE".into());

                    if XGetWindowProperty(
                        self.display,
                        focus,
                        XInternAtom(self.display, cstring.as_ptr(), 0),
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
                            if *data.offset(i) == (XInternAtom(self.display, cstring.as_ptr(), 0) & 0xFF) as u8 {
                                fullscreen = true;
                                break;
                            }
                        }

                        XFree(data as *mut c_void);

                        fullscreen
                    }
                });
            }
            if !self.not_when_fullscreen || !self.fullscreen.unwrap() {
                if self.notify.is_some() && !self.ran_notify {
                    invoke(self.notifier.as_ref().unwrap());

                    self.ran_notify = true;
                    self.delay = Duration::from_secs(1);

                    // Since the delay is usually a minute, I could've exceeded both the notify and the timer.
                    // The simple solution is to change the timer to a point where it's guaranteed
                    // it's been _ seconds since the notifier.
                    self.time_new = idle + self.notify.unwrap();
                } else if idle >= self.time_new && !self.ran_timer {
                    self.trigger();

                    self.ran_timer = true;
                    self.delay = default_delay;
                }
            }
        } else {
            if self.ran_notify && !self.ran_timer {
                // In case the user goes back from being idle between the notify and timer
                if let Some(canceller) = self.canceller.as_ref() {
                    invoke(canceller);
                }
            }
            self.delay = default_delay;
            self.ran_notify = false;
            self.ran_timer  = false;
            self.fullscreen = None;
        }

        true
    }
    fn trigger(&self) {
        invoke(&self.timer);
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
