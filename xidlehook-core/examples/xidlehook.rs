//! Minimal xidlehook shell command, aiming to be small and
//! modular. In a way, this is more of an example of how to implement
//! your own xidlehook-based client than an actual implementation.

use std::{
    process::Command,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use nix::{
    libc,
    sys::{signal, wait},
};
use structopt::StructOpt;
use xidlehook_core::{
    modules::{StopAt, Xcb},
    timers::CmdTimer,
    Module, Xidlehook,
};

static EXITED: AtomicBool = AtomicBool::new(false);

extern "C" fn exit_handler(_signo: libc::c_int) {
    EXITED.store(true, Ordering::SeqCst);
}

extern "C" fn sigchld_handler(_signo: libc::c_int) {
    let _ = wait::waitpid(None, Some(wait::WaitPidFlag::WNOHANG));
}

#[derive(StructOpt, Debug)]
pub struct Opt {
    /// Exit after the whole chain of timer commands have been invoked
    /// once
    #[structopt(long)]
    pub once: bool,
    /// Don't invoke the timer when the current application is
    /// fullscreen. Useful for preventing a lockscreen when watching
    /// videos.
    #[structopt(long)]
    pub not_when_fullscreen: bool,
    /// Don't invoke the timer when any audio is playing (only
    /// compatible with PulseAudio)
    #[cfg(feature = "pulse")]
    #[structopt(long)]
    pub not_when_audio: bool,

    /// The duration is the number of seconds of inactivity which
    /// should trigger this timer.
    ///
    /// The command is what is invoked when the idle duration is
    /// reached. It's passed through \"/bin/sh -c\".
    ///
    /// The canceller is what is invoked when the user becomes active
    /// after the timer has gone off, but before the next timer (if
    /// any). Pass an empty string to not have one.
    #[structopt(long, required = true, value_names = &["duration", "command", "canceller"])]
    pub timer: Vec<String>,
}

fn main() -> xidlehook_core::Result<()> {
    env_logger::init();

    let opt = Opt::from_args();

    let xcb = Rc::new(Xcb::new()?);

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
            modules.push(Box::new(xidlehook_core::modules::NotWhenAudio::new()?))
        }
    }

    let xidlehook = Xidlehook::new(timers).register(modules);

    unsafe {
        for &(signal, handler) in &[
            (
                signal::Signal::SIGINT,
                exit_handler as extern "C" fn(libc::c_int),
            ),
            (
                signal::Signal::SIGCHLD,
                sigchld_handler as extern "C" fn(libc::c_int),
            ),
        ] {
            signal::sigaction(
                signal,
                &signal::SigAction::new(
                    signal::SigHandler::Handler(handler),
                    signal::SaFlags::empty(),
                    signal::SigSet::empty(),
                ),
            )?;
        }
    }

    xidlehook.main_sync(&xcb, || EXITED.load(Ordering::SeqCst))?;
    Ok(())
}
fn command(cmd: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg(cmd);
    command
}
