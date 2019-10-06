use crate::Opt;

use std::{
    mem,
    os::unix::io::RawFd,
    rc::Rc,
    thread,
};

use async_std::{
    io::BufReader,
    os::unix::net::UnixListener,
    prelude::*,
    task,
};
use nix::{
    libc,
    sys::{
        signal::{
            self,
            SaFlags,
            SigAction,
            SigHandler,
            SigSet,
            Signal,
        },
        wait,
    },
    unistd,
};
use futures::future;
use log::debug;
use xidlehook::{
    modules::Xcb,
    Module,
    Timer,
    Xidlehook,
};

static mut SIGNAL_PIPE: (RawFd, RawFd) = (0, 0);

extern "C" fn handler(sig: libc::c_int) {
    let _ = unistd::write(unsafe { SIGNAL_PIPE.1 }, &sig.to_ne_bytes());
}

pub(crate) fn main_loop<T, M>(opt: Opt, xidlehook: Xidlehook<T, M>, xcb: Rc<Xcb>) -> xidlehook::Result<()>
where
    T: Timer,
    M: Module,
{
    if let Some(address) = opt.socket {
        task::spawn::<_, std::io::Result<()>>(async move {
            let listener = UnixListener::bind(&address).await?;
            loop {
                let (stream, _addr) = listener.accept().await?;
                let stream = BufReader::new(stream);

                task::spawn::<_, std::io::Result<()>>(async {
                    let mut lines = stream.lines();
                    while let Some(msg) = lines.next().await {
                        let json: serde_json::Value = serde_json::from_str(&msg?)?;
                        debug!("JSON message: {:?}", json);
                    }
                    Ok(())
                });
            }
        });
    }

    // Signal handling in async-std ***sucks*** so far

    unsafe {
        SIGNAL_PIPE = unistd::pipe()?;
    }

    for &sig in &[Signal::SIGINT, Signal::SIGCHLD] {
        unsafe {
            signal::sigaction(sig, &SigAction::new(
                SigHandler::Handler(handler),
                SaFlags::empty(),
                SigSet::empty(),
            ))?;
        }
    }

    let (maybe_abort, handle) = future::abortable(xidlehook.main_async(&xcb));

    let signal_thread = thread::spawn(move || -> nix::Result<()> {
        loop {
            let mut bytes = [0; mem::size_of::<libc::c_int>()];
            unistd::read(unsafe { SIGNAL_PIPE.0 }, &mut bytes)?;

            let signal = Signal::from_c_int(libc::c_int::from_ne_bytes(bytes))?;
            debug!("Signal received: {}", signal);

            match signal {
                Signal::SIGCHLD => {
                    let _ = wait::waitpid(None, Some(wait::WaitPidFlag::WNOHANG));
                },
                Signal::SIGINT => {
                    handle.abort();
                    break;
                },
                _ => (),
            }
        }
        Ok(())
    });

    if let Ok(not_aborted) = task::block_on(maybe_abort) {
        not_aborted?;
    }

    signal_thread.join().unwrap()?;

    Ok(())
}
