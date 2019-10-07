use std::{fs, process::Command, rc::Rc, time::Duration};

use async_std::{future::select, task};
use futures::{channel::{mpsc, oneshot}, future, prelude::*};
use log::{trace, warn};
use nix::{libc, sys::signal::Signal};
use structopt::StructOpt;
use xidlehook::{
    modules::{StopAt, Xcb},
    timers::CmdTimer,
    Module, Xidlehook,
};

mod signal_handler;
mod socket;

struct Defer<F: FnMut()>(F);
impl<F: FnMut()> Drop for Defer<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

#[derive(StructOpt, Debug)]
pub struct Opt {
    /// Print the idle time to standard output. This is similar to xprintidle.
    #[structopt(long)]
    pub print: bool,
    /// Exit after the whole chain of timer commands have been invoked
    /// once
    #[structopt(long, conflicts_with("print"))]
    pub once: bool,
    /// Don't invoke the timer when the current application is
    /// fullscreen. Useful for preventing a lockscreen when watching
    /// videos.
    #[structopt(long, conflicts_with("print"))]
    pub not_when_fullscreen: bool,

    /// The duration is the number of seconds of inactivity which
    /// should trigger this timer.
    ///
    /// The command is what is invoked when the idle duration is
    /// reached. It's passed through \"/bin/sh -c\".
    ///
    /// The canceller is what is invoked when the user becomes active
    /// after the timer has gone off, but before the next timer (if
    /// any). Pass an empty string to not have one.
    #[structopt(long, conflicts_with("print"), required_unless("print"), value_names = &["duration", "command", "canceller"])]
    pub timer: Vec<String>,

    /// Don't invoke the timer when any audio is playing (PulseAudio specific)
    #[cfg(feature = "pulse")]
    #[structopt(long, conflicts_with("print"))]
    pub not_when_audio: bool,

    /// Listen to a unix socket at this address for events.
    /// Each event is one line of JSON data.
    #[structopt(long, conflicts_with("print"))]
    pub socket: Option<String>,
}

fn main() -> xidlehook::Result<()> {
    env_logger::init();

    let opt = Opt::from_args();

    let xcb = Rc::new(Xcb::new()?);

    if opt.print {
        let idle = xcb.get_idle()?;
        println!("{}", idle.as_millis());
        return Ok(());
    }

    let mut timers = Vec::new();
    let mut iter = opt.timer.iter().peekable();
    while iter.peek().is_some() {
        // clap-rs will ensure there are always a multiple of 3
        let duration: u64 = match iter.next().unwrap().parse() {
            Ok(duration) => duration,
            Err(err) => {
                eprintln!("error: failed to parse duration as number: {}", err);
                return Ok(());
            },
        };
        timers.push(CmdTimer {
            time: Duration::from_secs(duration),
            activation: Some(command(iter.next().unwrap())),
            abortion: iter.next().filter(|s| !s.is_empty()).map(|s| command(&s)),
            ..CmdTimer::default()
        });
    }

    let mut modules: Vec<Box<dyn Module>> = Vec::new();

    if opt.once {
        modules.push(Box::new(StopAt::completion()));
    }
    if opt.not_when_fullscreen {
        modules.push(Box::new(Rc::clone(&xcb).not_when_fullscreen()));
    }
    #[cfg(feature = "pulse")]
    {
        if opt.not_when_audio {
            modules.push(Box::new(xidlehook::modules::NotWhenAudio::new()?))
        }
    }

    let mut xidlehook = Xidlehook::new(timers).register(modules);

    let (socket_tx, socket_rx) = mpsc::channel(4);
    let _scope = if let Some(address) = opt.socket {
        {
            let address = address.clone();
            task::spawn(async move {
                if let Err(err) = socket::socket_loop(&address, socket_tx).await {
                    warn!("Socket handling errored: {}", err);
                }
            });
        }
        Some(Defer(move || {
            trace!("Removing unix socket {}", address);
            let _ = fs::remove_file(&address);
        }))
    } else {
        None
    };

    let (signal_tx, signal_rx) = mpsc::channel(1);
    let signal_thread = signal_handler::handle_signals(signal_tx)?;

    let mut socket_rx = Some(socket_rx);
    let mut signal_rx = Some(signal_rx);

    loop {
        enum Selected {
            Socket(Option<(socket::Message, oneshot::Sender<socket::Reply>)>),
            Signal(Option<Signal>),
            Exit(xidlehook::Result<()>),
        }

        let a = socket_rx
            .as_mut()
            .map(|rx| -> Box<dyn Future<Output = _> + Unpin> { Box::new(rx.next().map(Selected::Socket)) })
            .unwrap_or_else(|| Box::new(future::pending()));
        let b = signal_rx
            .as_mut()
            .map(|rx| -> Box<dyn Future<Output = _> + Unpin> { Box::new(rx.next().map(Selected::Signal)) })
            .unwrap_or_else(|| Box::new(future::pending()));

        let c = xidlehook.main_async(&xcb).map(Selected::Exit);
        let res = task::block_on(select!(a, b, c));

        match res {
            Selected::Socket(data) => {
                if let Some((msg, reply)) = data {
                    trace!("Got command over socket: {:#?}", msg);
                    let response = match msg {
                        socket::Message::Add(add) => {
                            let timers = xidlehook.timers_mut()?;

                            let index = add.index.map(usize::from).unwrap_or(timers.len());
                            if index > timers.len() {
                                socket::Reply::Error(String::from("index > length"))
                            } else {
                                timers.insert(index, CmdTimer {
                                    time: add.time,
                                    activation: if add.activation.is_empty() {
                                        None
                                    } else {
                                        let mut activation = Command::new(&add.activation[0]);
                                        activation.args(&add.activation[1..]);
                                        Some(activation)
                                    },
                                    abortion: if add.abortion.is_empty() {
                                        None
                                    } else {
                                        let mut abortion = Command::new(&add.abortion[0]);
                                        abortion.args(&add.abortion[1..]);
                                        Some(abortion)
                                    },
                                    deactivation: if add.deactivation.is_empty() {
                                        None
                                    } else {
                                        let mut deactivation = Command::new(&add.deactivation[0]);
                                        deactivation.args(&add.deactivation[1..]);
                                        Some(deactivation)
                                    },
                                    disabled: false,
                                });
                                socket::Reply::Empty
                            }
                        },
                        socket::Message::Control(control) => {
                            let timers = xidlehook.timers_mut()?;

                            let mut removed = 0;
                            for id in control.timer.iter(timers.len() as socket::TimerId) {
                                let id = usize::from(id - removed);
                                if id >= timers.len() {
                                    continue;
                                }

                                match control.action {
                                    socket::Action::Disable => timers[id].disabled = true,
                                    socket::Action::Enable => timers[id].disabled = false,
                                    socket::Action::Trigger => unimplemented!(),
                                    socket::Action::Delete => {
                                        // Probably want to use `retain` to optimize this...
                                        timers.remove(id);
                                        removed += 1;
                                    }
                                }
                            }

                            socket::Reply::Empty
                        },
                        socket::Message::Query(query) => {
                            let timers = xidlehook.timers();
                            let mut output = Vec::new();

                            for id in query.timer.iter(timers.len() as socket::TimerId) {
                                let timer = match timers.get(usize::from(id)) {
                                    Some(timer) => timer,
                                    None => continue,
                                };
                                output.push(socket::QueryResult {
                                    timer: id,
                                    activation: Vec::new(), // TODO
                                    abortion: Vec::new(), // TODO
                                    deactivation: Vec::new(), // TODO
                                    // activation: timer.activation.clone(),
                                    // abortion: timer.abortion.clone(),
                                    // deactivation: timer.deactivation.clone(),
                                    disabled: timer.disabled,
                                });
                            }

                            socket::Reply::QueryResult(output)
                        },
                    };
                    reply.send(response).unwrap();
                } else {
                    socket_rx = None;
                }
            },
            Selected::Signal(sig) => {
                if let Some(sig) = sig {
                    trace!("Signal received: {}", sig);
                    break;
                } else {
                    signal_rx = None;
                }
            },
            Selected::Exit(res) => {
                res?;
            },
        }
    }

    // Call signal handler to pretend there's a signal - which will
    // cause thread to exit
    signal_handler::handler(Signal::SIGINT as i32 as libc::c_int);

    signal_thread.join().unwrap()?;

    Ok(())
}
fn command(cmd: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg(cmd);
    command
}
