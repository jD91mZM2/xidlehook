#[cfg(feature = "pulse")] extern crate libpulse_sys;
#[cfg(feature = "tokio")] extern crate futures;
#[cfg(feature = "tokio")] extern crate tokio_core;
#[cfg(feature = "tokio")] extern crate tokio_io;
#[cfg(feature = "tokio")] extern crate tokio_signal;
#[cfg(feature = "tokio")] extern crate tokio_uds;
#[macro_use] extern crate clap;
#[macro_use] extern crate failure;
extern crate x11;

#[cfg(feature = "pulse")] mod pulse;

#[cfg(feature = "pulse")]
use pulse::{Event, PulseAudio};
#[cfg(feature = "tokio")]
use futures::{
    future::{self, Either, Loop},
    sync::mpsc,
    Future,
    Stream
};
#[cfg(feature = "tokio")]
use std::{
    cell::RefCell,
    fs,
    io::ErrorKind,
    rc::Rc
};
#[cfg(feature = "tokio")] use tokio_core::reactor::{Core, Timeout};
#[cfg(feature = "tokio")] use tokio_io::io;
#[cfg(feature = "tokio")] use tokio_signal::unix::{Signal, SIGINT, SIGTERM};
#[cfg(feature = "tokio")] use tokio_uds::UnixListener;
#[cfg(not(feature = "tokio"))] use std::thread;
use clap::{App as ClapApp, Arg};
use failure::Error;
use std::{
    mem,
    os::raw::c_void,
    process::Command,
    ptr,
    time::Duration
};
use x11::{
    xlib::{Display, XA_ATOM, XCloseDisplay, XDefaultRootWindow, XFree, XGetInputFocus, XGetWindowProperty,
           XInternAtom, XOpenDisplay},
    xss::{XScreenSaverAllocInfo, XScreenSaverInfo, XScreenSaverQueryInfo}
};

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
const NET_WM_STATE: &str = "_NET_WM_STATE\0";
const NET_WM_STATE_FULLSCREEN: &str = "_NET_WM_STATE_FULLSCREEN\0";

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
        // Flags
        .arg(
            Arg::with_name("print")
                .help("Print the idle time to standard output. This is similar to xprintidle.")
                .long("print")
        )
        .arg(
            Arg::with_name("not-when-fullscreen")
                .help("Don't invoke the timer when the current application is fullscreen. \
                       Useful for preventing the lockscreen when watching videos")
                .long("not-when-fullscreen")
                .conflicts_with("print")
        )
        .arg(
            Arg::with_name("once")
                .help("Exit after timer command has been invoked once. \
                       This does not include manual invoking using the socket.")
                .long("once")
                .conflicts_with("print")
        )
        // Options
        .arg(
            Arg::with_name("time")
                .help("Set the required amount of idle minutes before invoking timer")
                .long("time")
                .takes_value(true)
                .required_unless("print")
                .conflicts_with("print")
        )
        .arg(
            Arg::with_name("timer")
                .help("Set command to run when the timer goes off")
                .long("timer")
                .takes_value(true)
                .required_unless("print")
                .conflicts_with("print")
        )
        .arg(
            Arg::with_name("notify")
                .help("Run the command passed by --notifier _ seconds before timer goes off")
                .long("notify")
                .takes_value(true)
                .requires("notifier")
                .conflicts_with("print")
        )
        .arg(
            Arg::with_name("notifier")
                .help("Set the command to run when notifier goes off (see --notify)")
                .long("notifier")
                .takes_value(true)
                .requires("notify")
                .conflicts_with("print")
        )
        .arg(
            Arg::with_name("canceller")
                .help("Set the command to run when user cancels the timer after the notifier has already gone off")
                .long("canceller")
                .takes_value(true)
                .requires("notify")
                .conflicts_with("print")
        );
    #[cfg(feature = "tokio")]
    let mut clap_app = clap_app; // make mutable
    #[cfg(feature = "tokio")] {
        clap_app = clap_app
            .arg(
                Arg::with_name("socket")
                    .help("Listen to events over a unix socket")
                    .long("socket")
                    .takes_value(true)
                    .conflicts_with("print")
            );
    }
    #[cfg(feature = "pulse")] {
        clap_app = clap_app
            .arg(
                Arg::with_name("not-when-audio")
                    .help("Don't invoke the timer when any audio is playing (PulseAudio specific)")
                    .long("not-when-audio")
                    .conflicts_with("print")
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
        audio: false,
        delay: Duration::from_secs(SCALE),

        display: display,
        info: info,

        not_when_fullscreen: matches.is_present("not-when-fullscreen"),
        once: matches.is_present("once"),
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

    #[cfg(not(feature = "tokio"))] {
        let mut app = app;
        loop {
            if let Some(result) = app.step() {
                if let Err(ref err) = result {
                    eprintln!("{}", err);
                }
                return result.is_ok();
            }

            thread::sleep(app.delay);
        }
    }
    #[cfg(feature = "tokio")] {
        #[cfg(feature = "pulse")]
        let not_when_audio  = matches.is_present("not-when-audio");

        let socket = matches.value_of("socket");
        let app = Rc::new(RefCell::new(app));

        let mut core = Core::new().unwrap();
        let handle = Rc::new(core.handle());

        let handle_clone = Rc::clone(&handle);
        let app_clone = Rc::clone(&app);
        let future = future::loop_fn((), move |_| {
                    let app = Rc::clone(&app_clone);
                    let delay = app.borrow().delay;
                    Timeout::new(delay, &handle_clone)
                        .unwrap()
                        .from_err()
                        .and_then(move |_| {
                            let step = app.borrow_mut().step();
                            match step {
                                None           => Ok(Loop::Continue(())),
                                Some(Ok(()))   => Ok(Loop::Break(())),
                                Some(Err(err)) => Err(err)
                            }
                        })
                })
                .map_err(Error::from)
                .select2(Signal::new(SIGINT, &handle)
                        .flatten_stream()
                        .into_future()
                        .map_err(|(err, _)| err.into()))
                .map_err(|err| Error::from(Either::split(err).0))
                .select2(Signal::new(SIGTERM, &handle)
                        .flatten_stream()
                        .into_future()
                        .map_err(|(err, _)| err.into()))
                .map_err(|err| Error::from(Either::split(err).0))
                .map(|_| ());
        #[cfg(feature = "pulse")]
        let mut future: Box<Future<Item = (), Error = Error>> = Box::new(future);

        if let Some(socket) = socket {
            let listener = match UnixListener::bind(socket, &handle) {
                Ok(listener) => listener,
                Err(err) => {
                    eprintln!("failed to bind unix socket: {}", err);
                    return false;
                }
            };

            let handle_clone = Rc::clone(&handle);

            let app_clone = Rc::clone(&app);
            handle.spawn(listener.incoming()
                .map_err(|err| eprintln!("unix socket: listener error: {}", err))
                .for_each(move |(conn, _)| {
                    let app = Rc::clone(&app_clone);
                    handle_clone.spawn(future::loop_fn(conn, move |conn| {
                        let app = Rc::clone(&app);
                        io::read_exact(conn, [0; 1])
                            .map_err(|err| {
                                if err.kind() != ErrorKind::UnexpectedEof {
                                    eprintln!("unix socket: io error: {}", err);
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
        #[cfg(feature = "pulse")]
        let mut _pulse = None; // Keep pulse alive
        #[cfg(feature = "pulse")] {
            if not_when_audio {
                let (tx, rx) = mpsc::unbounded::<Event>();

                let pulse = PulseAudio::default();

                // Keep pulse alive
                _pulse = Some(pulse);
                let pulse = _pulse.as_mut().unwrap();

                let mut playing = 0;

                future = Box::new(future
                    .select2(rx
                        .for_each(move |event| {
                            match event {
                                Event::Clear => playing = 0,
                                Event::New => playing += 1,
                                Event::Finish => {
                                    // We've successfully counted all playing inputs
                                    app.borrow_mut().audio = playing != 0;
                                }
                            }
                            Ok(())
                        })
                        .map_err(|_| format_err!("pulseaudio: sink receive failed")))
                    .map_err(|err| Error::from(err.split().0))
                    .map(|_| ()));

                pulse.connect(tx);
            }
        }

        let result = core.run(future);

        if let Err(ref err) = result {
            eprintln!("{}", err);
        }

        if let Some(socket) = socket {
            if let Err(err) = fs::remove_file(socket) {
                eprintln!("failed to clean up unix socket: {}", err);
            }
        }

        result.is_ok()
    }
}
struct App {
    active: bool,
    audio: bool,
    delay: Duration,

    display: *mut Display,
    info: *mut XScreenSaverInfo,

    not_when_fullscreen: bool,
    once: bool,
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
    fn step(&mut self) -> Option<Result<(), Error>> {
        let active = self.active && !self.audio;
        // audio is always false when not-when-audio isn't set, don't worry

        let default_delay = Duration::from_secs(SCALE); // TODO: const fn

        let idle = if active {
            Some(match get_idle(self.display, self.info) {
                Ok(idle) => idle / 1000, // Convert to seconds
                Err(err) => return Some(Err(err))
            })
        } else { None };

        if active &&
            self.notify.map(|notify| (idle.unwrap() + notify) >= self.time).unwrap_or(idle.unwrap() >= self.time) {
            let idle = idle.unwrap();
            if self.not_when_fullscreen && self.fullscreen.is_none() {
                let mut focus = 0u64;
                let mut revert = 0i32;

                let mut actual_type = 0u64;
                let mut actual_format = 0i32;
                let mut nitems = 0u64;
                let mut bytes = 0u64;
                let mut data: *mut u8 = unsafe { mem::uninitialized() };

                self.fullscreen = Some(unsafe {
                    XGetInputFocus(self.display, &mut focus, &mut revert);

                    if XGetWindowProperty(
                        self.display,
                        focus,
                        XInternAtom(self.display, NET_WM_STATE.as_ptr() as *mut i8, 0),
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
                        eprintln!("warning: failed to get window property");
                        false
                    } else {
                        let mut fullscreen = false;

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

                        let data_enum = match actual_format {
                            8  => Some(Ptr::U8(data)),
                            16 => Some(Ptr::U16(data as *mut u16)),
                            32 => Some(Ptr::U32(data as *mut u32)),
                            _  => None
                        };

                        let atom = XInternAtom(
                            self.display,
                            NET_WM_STATE_FULLSCREEN.as_ptr() as *mut i8,
                            0
                        );

                        for i in 0..nitems as isize {
                            if data_enum.unwrap().offset(i).deref_u64() == atom {
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

                    if self.once {
                        return Some(Ok(()));
                    }

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

        None
    }
    fn trigger(&self) {
        invoke(&self.timer);
    }
}
fn get_idle(display: *mut Display, info: *mut XScreenSaverInfo) -> Result<u64, Error> {
    if unsafe { XScreenSaverQueryInfo(display, XDefaultRootWindow(display), info) } == 0 {
        return Err(format_err!("failed to query screen saver info"));
    }

    Ok(unsafe { (*info).idle })
}
fn invoke(cmd: &str) {
    if let Err(err) =
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .status() {
        eprintln!("warning: failed to invoke command: {}", err);
    }
}
