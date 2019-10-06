use crate::Opt;

use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};

use nix::{
    libc,
    sys::{signal, wait},
};
use xidlehook::{modules::Xcb, Module, Timer, Xidlehook};

static EXITED: AtomicBool = AtomicBool::new(false);

extern "C" fn exit_handler(_signo: libc::c_int) {
    EXITED.store(true, Ordering::SeqCst);
}

extern "C" fn sigchld_handler(_signo: libc::c_int) {
    let _ = wait::waitpid(None, Some(wait::WaitPidFlag::WNOHANG));
}

pub fn main_loop<T, M>(_opt: Opt, xidlehook: Xidlehook<T, M>, xcb: Rc<Xcb>) -> xidlehook::Result<()>
where
    T: Timer,
    M: Module,
{
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
