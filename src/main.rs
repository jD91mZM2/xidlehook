#[cfg(feature = "nix")] extern crate nix;
#[cfg(feature = "pulse")] extern crate libpulse_sys;
#[macro_use] extern crate clap;
#[macro_use] extern crate failure;
extern crate mio;
extern crate x11;

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
        unix::{
            io::AsRawFd,
            net::UnixListener
        }
    },
    path::Path,
    process::Command,
    time::Duration
};
use x11::xss::{XScreenSaverAllocInfo, XScreenSaverInfo};

#[cfg(feature = "pulse")] mod pulse;
mod x11api;

#[cfg(feature = "pulse")] use pulse::PulseAudio;
use x11api::{XDisplay, XPtr};

#[derive(Debug, Fail)]
pub enum MyError {
    #[fail(display = "failed to open x display")]
    XDisplay,
    #[fail(display = "failed to query x for input focus")]
    XGetInputFocus,
    #[fail(display = "failed to query x for window properties")]
    XGetWindowProperty,
    #[fail(display = "failed to query for screen saver info")]
    XScreenSaver
}

struct DeferRemove<T: AsRef<Path>>(T);
impl<T: AsRef<Path>> Drop for DeferRemove<T> {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

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
            Arg::with_name("timer")
                .long_help("\
                    Pass the following arguments:\n\
                    <\"normal\"/\"primary\"> <duration> <command> <canceller>
                    \n\n\
                    If the timer is specified as primary it's the timer chosen \
                    to be triggered by the socket. Only one timer may be \
                    specified as primary. \
                    \n\n\
                    The duration is the number of seconds of inactivity which \
                    should trigger this timer. \
                    \n\n\
                    The command is what is invoked when the idle duration is \
                    reached. It's passed through \"sh -c\". \
                    \n\n\
                    The canceller is what is invoked when the user becomes \
                    active after the timer has gone off, but before the next \
                    timer (if any). Pass an empty string to not have one. \
                ")
                .long("timer")
                .takes_value(true)
                .number_of_values(4)
                .multiple(true)
                .required_unless("print")
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

    let display = XDisplay::new()?;

    let info = unsafe { XPtr::new(XScreenSaverAllocInfo()) };

    if matches.is_present("print") {
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

    let mut timers = Vec::new();
    let mut primary = None;
    if let Some(iter) = matches.values_of("timer") {
        let mut iter = iter.peekable();
        while iter.peek().is_some() {
            // clap will ensure there are always a multiple of 4
            match iter.next().unwrap() {
                "normal" => (),
                "primary" => if primary.is_none() {
                    primary = Some(timers.len())
                } else {
                    eprintln!("error: more than one primary timer specified");
                    return Ok(());
                },
                mode => {
                    eprintln!("error: invalid mode specified. {:?} is neither \"normal\" nor \"primary\"", mode);
                    return Ok(());
                }
            }
            let duration = match iter.next().unwrap().parse() {
                Ok(duration) => duration,
                Err(err) => {
                    eprintln!("error: failed to parse duration as number: {}", err);
                    return Ok(());
                }
            };
            timers.push(Timer {
                duration,
                command: iter.next().unwrap().to_string(),
                canceller: Some(iter.next().unwrap()).filter(|s| !s.is_empty()).map(String::from)
            });
        }
    }

    let mut app = App {
        // Data
        display,
        info,

        // Flags
        not_when_fullscreen: matches.is_present("not-when-fullscreen"),
        once: matches.is_present("once"),
        timers,

        // State
        active: true,
        audio: false,
        index: 0,

        // Temporary state
        last_idle: None,
        base: 0,
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
        // Wait for as much time as we can guarantee
        let delay = if app.current().map(|t| t.canceller.is_some()).unwrap_or(false) {
            // There's a canceller, so we need to check idle time very often
            1
        } else if let Some(duration) = app.next().map(|t| t.duration) {
            // Sleep for how much of the duration is left
            let idle = app.last_idle.map(Ok).unwrap_or_else(|| x11api::get_idle(*app.display, *app.info))?;
            duration.saturating_sub(idle.saturating_sub(app.base))
                .min(app.timers.first().unwrap().duration)
        } else {
            // Sleep for as long as the first duration, as it's going to reset
            // when they wake up
            app.timers.first().unwrap().duration
        };
        poll.poll(&mut events, Some(Duration::from_secs(delay)))?;

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
                            COMMAND_TRIGGER => if let Some(primary) = primary {
                                if app.index <= primary {
                                    invoke(&app.timers[primary].command);
                                    app.index = primary + 1;

                                    if app.once && app.index >= app.timers.len() {
                                        break 'main;
                                    }
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
struct Timer {
    duration: u64,
    command: String,
    canceller: Option<String>
}
struct App {
    // Data
    display: XDisplay,
    info: XPtr<XScreenSaverInfo>,

    // Flags
    not_when_fullscreen: bool,
    once: bool,
    timers: Vec<Timer>,

    // State
    active: bool,
    audio: bool,
    index: usize,

    // Temporary state
    last_idle: Option<u64>,
    base: u64,
    fullscreen: Option<bool>
}
impl App {
    fn current(&self) -> Option<&Timer> {
        self.index.checked_sub(1).map(|i| &self.timers[i])
    }
    fn next(&self) -> Option<&Timer> {
        self.timers.get(self.index)
    }
    fn reset(&mut self) {
        if let Some(canceller) = self.current().and_then(|t| t.canceller.as_ref()) {
            // In case the user goes back from being idle between two timers
            invoke(canceller);
        }

        self.fullscreen = None;
        self.index = 0;
        self.base = 0;
    }
    fn step(&mut self) -> Result<bool, Error> {
        let active = self.active && !self.audio;

        if !active {
            self.reset();
            return Ok(true);
        }

        let idle = x11api::get_idle(*self.display, *self.info)?;
        let last_idle = mem::replace(&mut self.last_idle, Some(idle));
        if last_idle.map(|last| idle < last).unwrap_or(false) {
            // Mouse must have moved, idle time is less than previous
            self.reset();
            return Ok(true)
        }

        if self.index >= self.timers.len() {
            // We've ran all timers, sit tight
            return Ok(true);
        }

        if idle < self.base + self.timers[self.index].duration {
            // We're in before any timer
            return Ok(true);
        }

        if self.not_when_fullscreen && self.fullscreen.is_none() {
            // We haven't cached a fullscreen status, let's fetch one
            self.fullscreen = Some(match unsafe { x11api::get_fullscreen(*self.display) } {
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

        let timer = &self.timers[self.index];
        invoke(&timer.command);
        self.base += timer.duration;
        self.index += 1;

        if self.once && self.index >= self.timers.len() {
            // false = exit
            return Ok(false);
        }

        Ok(true)
    }
}
