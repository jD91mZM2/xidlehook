#[cfg(feature = "nix")] extern crate nix;
#[cfg(feature = "pulse")] extern crate libpulse_sys;
#[macro_use] extern crate clap;
#[macro_use] extern crate failure;
extern crate mio;
extern crate x11;

#[cfg(feature = "pulse")] mod pulse;
mod x11api;

#[cfg(feature = "pulse")] use pulse::PulseAudio;
#[cfg(feature = "pulse")] use std::sync::mpsc;
use clap::{App as ClapApp, Arg};
use failure::Error;
use mio::{*, unix::EventedFd};
#[cfg(feature = "nix")]
use nix::sys::{
    signal::{Signal, SigSet},
    signalfd::{SignalFd, SfdFlags}
};
use std::{
    collections::HashMap,
    fs,
    io::{self, prelude::*},
    mem,
    os::{
        raw::c_void,
        unix::{
            io::AsRawFd,
            net::UnixListener
        }
    },
    path::Path,
    process::Command,
    ptr,
    time::Duration
};
use x11::{
    xlib::{Display, XCloseDisplay, XFree, XOpenDisplay},
    xss::{XScreenSaverAllocInfo, XScreenSaverInfo}
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
struct DeferRemove<T: AsRef<Path>>(T);
impl<T: AsRef<Path>> Drop for DeferRemove<T> {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

const SCALE: u64 = 60; // Second:minute scale. Can be changed for debugging purposes.

const COMMAND_DEACTIVATE: u8 = 0;
const COMMAND_ACTIVATE:   u8 = 1;
const COMMAND_TRIGGER:    u8 = 2;

#[cfg(feature = "nix")]
const TOKEN_SIGNAL: Token = Token(0);
const TOKEN_SERVER: Token = Token(1);
const TOKEN_CLIENT: Token = Token(2);

fn maybe<T>(res: io::Result<T>) -> io::Result<Option<T>> {
    match res {
        Ok(res) => Ok(Some(res)),
        Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => Ok(None),
        Err(err) => Err(err)
    }
}

fn main() -> Result<(), Error> {
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
                .long_help("\
                    Don't invoke the timer when the current application is \
                    fullscreen. Useful for preventing a lockscreen when \
                    watching videos. \
                ")
                .long("not-when-fullscreen")
                .conflicts_with("print")
        )
        .arg(
            Arg::with_name("once")
                .long_help("\
                    Exit after timer command has been invoked once. \
                    This does not include manual invoking using the socket. \
                ")
                .long("once")
                .conflicts_with("print")
        )
        // Options
        .arg(
            Arg::with_name("time")
                .long_help("\
                    Set the required amount of idle time before invoking timer. \
                    The time is specified in minutes. The timer may be anything \
                    within an entire minute late, because xidlehook prefers to \
                    sleep for long durations of time. \
                ")
                .long("time")
                .takes_value(true)
                .required_unless("print")
                .conflicts_with("print")
        )
        .arg(
            Arg::with_name("timer")
                .help("Set command to run when the timer goes off (see --time)")
                .long("timer")
                .takes_value(true)
                .required_unless("print")
                .conflicts_with("print")
        )
        .arg(
            Arg::with_name("notify")
                .long_help("\
                    A notifier command is run the specified amount of time \
                    before the timer is invoked. The time is specified in \
                    seconds. Since the timer may be late, it is delayed further \
                    to guarantee that the notifier has been shown for at least \
                    this duration. \
                ")
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
                .long_help("\
                    Set the command to run when the user stops being idle after \
                    a notifier has gone off but before the timer did. Upon a \
                    successful timer invocation, the canceller is not invoked. \
               ")
                .long("canceller")
                .takes_value(true)
                .requires("notify")
                .conflicts_with("print")
        )
        .arg(
            Arg::with_name("time2")
                .long_help("\
                    When the first timer has gone off, wait this amount of time \
                    before executing the second timer. The time is specified in \
                    minutes. An example of this would be to automatically \
                    suspend the computer 10 minutes after the lockscreen was \
                    started. Then the first timer would be the lockscreen and \
                    the second timer would be the suspending. \
                ")
                .long("time2")
                .takes_value(true)
                .requires("timer2")
                .conflicts_with("print")
        )
        .arg(
            Arg::with_name("timer2")
                .help("Set command to run when the second timer goes off (see --time2)")
                .long("timer2")
                .takes_value(true)
                .requires("time2")
                .conflicts_with("print")
        )
        .arg(
            Arg::with_name("socket")
                .long_help("\
                    Listen to events over a specified unix socket.\n\
                    Events are as following:\n\
                    \t0x0 - Disable xidlehook\n\
                    \t0x1 - Re-enable xidlehook\n\
                    \t0x2 - Trigger the timer immediately\n\
                ")
                .long("socket")
                .takes_value(true)
                .conflicts_with("print")
        );
    #[cfg(feature = "pulse")]
    let mut clap_app = clap_app; // make mutable
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
        bail!("failed to open x server");
    }
    let _cleanup = DeferXClose(display);

    let info = unsafe { XScreenSaverAllocInfo() };
    let _cleanup = DeferXFree(info as *mut c_void);

    if matches.is_present("print") {
        println!("{}", x11api::get_idle(display, info)?);
        return Ok(());
    }

    #[cfg(feature = "nix")]
    let mut signal = {
        let mut mask = SigSet::empty();
        mask.add(Signal::SIGINT);
        mask.add(Signal::SIGTERM);

        // signalfd won't receive stuff unless
        // we make the signals be sent synchronously
        mask.thread_block()?;

        SignalFd::with_flags(&mask, SfdFlags::SFD_NONBLOCK)?
    };


    let time = value_t!(matches, "time", u32).unwrap_or_else(|err| err.exit()) as u64 * SCALE;
    let mut app = App {
        // Data
        display,
        info,

        // Flags
        not_when_fullscreen: matches.is_present("not-when-fullscreen"),
        once: matches.is_present("once"),
        time,
        timer: matches.value_of("timer").unwrap().to_string(),
        notify: value_t!(matches, "notify", u32).ok().map(u64::from),
        notifier: matches.value_of("notifier").map(String::from),
        canceller: matches.value_of("canceller").map(String::from),
        time2: value_t!(matches, "time2", u32).ok().map(u64::from),
        timer2: matches.value_of("timer2").map(String::from),

        // State
        active: true,
        audio: false,
        delay: Duration::from_secs(SCALE),
        state: State::Waiting,

        // Temporary state
        last_idle: None,
        time_new: time,
        fullscreen: None
    };

    #[cfg(feature = "pulse")]
    let (tx_pulse, rx_pulse) = mpsc::channel();
    #[cfg(feature = "pulse")]
    let mut _pulse = None;
    #[cfg(feature = "pulse")] {
        if matches.is_present("not-when-audio") {
            // be careful not to move the struct
            _pulse = Some(PulseAudio::default());
            unsafe {
                _pulse.as_mut().unwrap().connect(tx_pulse);
            }
        }
    }

    let poll = Poll::new()?;

    #[cfg(feature = "nix")]
    poll.register(&EventedFd(&signal.as_raw_fd()), TOKEN_SIGNAL, Ready::readable(), PollOpt::edge())?;

    let mut _socket = None;
    let mut listener = match matches.value_of("socket") {
        None => None,
        Some(socket) => {
            let mut listener = UnixListener::bind(&socket)?;
            _socket = Some(DeferRemove(socket)); // remove file when exiting

            listener.set_nonblocking(true)?;

            poll.register(&EventedFd(&listener.as_raw_fd()), TOKEN_SERVER, Ready::readable(), PollOpt::edge())?;
            Some(listener)
        }
    };
    let mut clients = HashMap::new();
    let mut next_client = TOKEN_CLIENT.into();

    let mut events = Events::with_capacity(1024);

    'main: loop {
        poll.poll(&mut events, Some(app.delay))?;

        for event in &events {
            match event.token() {
                #[cfg(feature = "nix")]
                TOKEN_SIGNAL => if signal.read_signal()?.is_some() { break 'main; },
                TOKEN_SERVER => if let Some(listener) = listener.as_mut() {
                    let (mut socket, _) = match maybe(listener.accept())? {
                        Some(socket) => socket,
                        None => continue
                    };
                    socket.set_nonblocking(true)?;

                    let token = Token(next_client);
                    poll.register(&EventedFd(&socket.as_raw_fd()), token, Ready::readable(), PollOpt::edge())?;

                    clients.insert(token, socket);
                    next_client += 1;
                },
                token => {
                    let mut byte = [0];

                    let read = match clients.get_mut(&token) {
                        None => continue,
                        Some(client) => maybe(client.read(&mut byte))?
                    };
                    match read {
                        None => (),
                        Some(0) => {
                            // EOF, drop client
                            let socket = clients.remove(&token).unwrap();
                            poll.deregister(&EventedFd(&socket.as_raw_fd()))?;
                        },
                        Some(_) => match byte[0] {
                            COMMAND_DEACTIVATE => app.active = false,
                            COMMAND_ACTIVATE => app.active = true,
                            COMMAND_TRIGGER => if app.state < State::Invoked {
                                app.trigger();

                                match x11api::get_idle(app.display, app.info) {
                                    Ok(idle) => app.time_new = idle / 1000,
                                    Err(err) => eprintln!("couldn't get idle time: {}", err)
                                }

                                if app.once && app.time2.is_none() {
                                    break 'main;
                                }
                            },
                            byte => eprintln!("socket: unknown command: {}", byte)
                        }
                    }

                }
            }
        }

        #[cfg(feature = "pulse")] {
            while let Ok(count) = rx_pulse.try_recv() {
                // If the number of active audio devices is more than 0
                app.audio = count > 0;
            }
        }

        if !app.step()? {
            // Returning Ok(false) means exiting
            break;
        }
    }
    Ok(())
}
fn invoke(cmd: &str) {
    if let Err(err) =
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .spawn() {
        eprintln!("warning: failed to invoke command: {}", err);
    }
}
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum State {
    Waiting,
    Notified,
    Invoked,
    Invoked2
}
struct App {
    // Data
    display: *mut Display,
    info: *mut XScreenSaverInfo,

    // Flags
    not_when_fullscreen: bool,
    once: bool,
    time: u64,
    timer: String,
    notify: Option<u64>,
    notifier: Option<String>,
    canceller: Option<String>,
    time2: Option<u64>,
    timer2: Option<String>,

    // State
    active: bool,
    audio: bool,
    delay: Duration,
    state: State,

    // Temporary state
    last_idle: Option<u64>,
    time_new: u64,
    fullscreen: Option<bool>
}
impl App {
    fn trigger(&mut self) {
        invoke(&self.timer);

        self.state = State::Invoked;
        self.delay = Duration::from_secs(SCALE); // TODO: const fn;
    }
    fn reset(&mut self) {
        let default_delay = Duration::from_secs(SCALE); // TODO: const fn

        if self.state == State::Notified {
            // In case the user goes back from being idle between the notify and timer
            if let Some(canceller) = self.canceller.as_ref() {
                invoke(canceller);
            }
        }

        self.delay = default_delay;
        self.fullscreen = None;
        self.state = State::Waiting;
        self.time_new = self.time;
    }
    fn step(&mut self) -> Result<bool, Error> {
        let active = self.active && !self.audio;

        if !active {
            self.reset();
            return Ok(true);
        }

        let idle = x11api::get_idle(self.display, self.info)?;
        let last_idle = mem::replace(&mut self.last_idle, Some(idle));
        if last_idle.map(|last| idle < last).unwrap_or(false) {
            // Mouse must have moved, idle time is less than previous
            self.reset();
            return Ok(true)
        }
        let idle = idle / 1000; // millis to seconds

        if self.state < State::Notified &&
                self.notify.map(|notify| idle + notify < self.time).unwrap_or(idle < self.time) {
            // We're in before any notifier or timer, do nothing
            return Ok(true);
        }

        if self.not_when_fullscreen && self.fullscreen.is_none() {
            // We haven't cached a fullscreen status, let's fetch one
            self.fullscreen = Some(match unsafe { x11api::get_fullscreen(self.display) } {
                Ok(value) => value,
                Err(err) => {
                    eprintln!("warning: {}", err);
                    false
                }
            });
        }
        if self.not_when_fullscreen && self.fullscreen.unwrap() {
            // Something is (or was) fullscreen, do nothing
            return Ok(true);
        }
        if self.state < State::Notified && self.notify.is_some() {
            invoke(self.notifier.as_ref().unwrap());

            self.state = State::Notified;
            self.delay = Duration::from_secs(1);

            // Since the delay is usually a minute, I could've exceeded both the notify and the timer.
            // The simple solution is to change the timer to a point where it's guaranteed
            // it's been _ seconds since the notifier.
            self.time_new = idle + self.notify.unwrap();
        } else if self.state < State::Invoked && idle >= self.time_new {
            self.trigger();

            if self.once && self.time2.is_none() {
                return Ok(false); // exit
            }
        } else if self.state == State::Invoked &&
                self.time2.map(|time2| idle >= self.time_new + time2).unwrap_or(false) {
            invoke(self.timer2.as_ref().unwrap());

            self.state = State::Invoked2;

            if self.once {
                return Ok(false); // exit
            }
        }
        Ok(true)
    }
}
