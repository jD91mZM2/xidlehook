#![warn(
    // Harden built-in lints
    missing_copy_implementations,
    missing_debug_implementations,

    // Harden clippy lints
    clippy::cargo_common_metadata,
    clippy::clone_on_ref_ptr,
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    clippy::float_cmp_const,
    clippy::get_unwrap,
    clippy::integer_division,
)]

use std::{fs, rc::Rc, time::Duration};

use tokio::{stream::{self, StreamExt}, sync::mpsc, signal::unix::{signal, SignalKind}};
use log::{trace, warn};
use nix::sys::wait;
use structopt::StructOpt;
use xidlehook_core::{
    modules::{StopAt, Xcb},
    Module, Xidlehook,
};

mod socket;
mod timers;

use self::timers::CmdTimer;

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

#[tokio::main]
async fn main() -> xidlehook_core::Result<()> {
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
        timers.push(CmdTimer::from_shell(
            Duration::from_secs(duration),
            iter.next().unwrap().into(),
            iter.next().unwrap().into(),
            String::new(),
        ));
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
            modules.push(Box::new(xidlehook_core::modules::NotWhenAudio::new()?))
        }
    }

    let xidlehook = Xidlehook::new(timers).register(modules);
    App {
        opt,
        xcb,
        xidlehook,
    }.main_loop().await
}

struct App {
    opt: Opt,
    xcb: Rc<Xcb>,
    xidlehook: Xidlehook<CmdTimer, ((), Vec<Box<dyn Module>>)>,
}
impl App {
    async fn main_loop(&mut self) -> xidlehook_core::Result<()> {
        let (socket_tx, socket_rx) = mpsc::channel(4);
        let _scope = if let Some(address) = self.opt.socket.clone() {
            {
                let address = address.clone();
                tokio::spawn(async move {
                    if let Err(err) = socket::main_loop(&address, socket_tx).await {
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

        let mut socket_rx = Some(socket_rx);

        let mut sigint = signal(SignalKind::interrupt())?;
        let mut sigchld = signal(SignalKind::child())?;

        loop {
            let socket_msg = async {
                if let Some(ref mut rx) = socket_rx {
                    rx.recv().await
                } else {
                    // TODO: use future::pending() when released
                    stream::pending::<()>().next().await;
                    unreachable!();
                }
            };

            tokio::select! {
                data = socket_msg => {
                    if let Some((msg, reply)) = data {
                        trace!("Got command over socket: {:#?}", msg);
                        let response = match self.handle_socket(msg)? {
                            Some(response) => response,
                            None => break,
                        };
                        let _ = reply.send(response);
                    } else {
                        socket_rx = None;
                    }
                },
                res = self.xidlehook.main_async(&self.xcb) => {
                    res?;
                    break;
                },
                _ = sigint.recv() => {
                    trace!("SIGINT received");
                    break;
                },
                _ = sigchld.recv() => {
                    trace!("Waiting for child process");
                    let _ = wait::waitpid(None, Some(wait::WaitPidFlag::WNOHANG));
                },
            }
        }

        Ok(())
    }
}
