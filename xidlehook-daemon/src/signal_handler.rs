use std::{
    mem,
    os::unix::io::RawFd,
    thread::{self, JoinHandle},
};

use async_std::{task, sync};
use nix::{
    libc,
    sys::{
        signal::{self, SaFlags, SigAction, SigHandler, SigSet, Signal},
        wait,
    },
    unistd,
};

static mut SIGNAL_PIPE: (RawFd, RawFd) = (0, 0);

pub extern "C" fn handler(sig: libc::c_int) {
    let _ = unistd::write(unsafe { SIGNAL_PIPE.1 }, &sig.to_ne_bytes());
}

pub fn handle_signals(
    tx: sync::Sender<Signal>,
) -> xidlehook_core::Result<JoinHandle<nix::Result<()>>> {
    // Signal handling with async-std *sucks* currently (at 0.99.8)

    unsafe {
        SIGNAL_PIPE = unistd::pipe()?;
    }

    for &sig in &[Signal::SIGINT, Signal::SIGCHLD] {
        unsafe {
            signal::sigaction(
                sig,
                &SigAction::new(
                    SigHandler::Handler(handler),
                    SaFlags::empty(),
                    SigSet::empty(),
                ),
            )?;
        }
    }

    Ok(thread::spawn(move || -> nix::Result<()> {
        loop {
            let mut bytes = [0; mem::size_of::<libc::c_int>()];
            unistd::read(unsafe { SIGNAL_PIPE.0 }, &mut bytes)?;

            let signal = Signal::from_c_int(libc::c_int::from_ne_bytes(bytes))?;

            match signal {
                Signal::SIGCHLD => {
                    let _ = wait::waitpid(None, Some(wait::WaitPidFlag::WNOHANG));
                },
                Signal::SIGINT => {
                    task::block_on(tx.send(signal));
                    break;
                },
                _ => (),
            }
        }
        Ok(())
    }))
}
