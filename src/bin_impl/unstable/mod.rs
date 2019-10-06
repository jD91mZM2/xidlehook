use crate::Opt;

use std::{fs, rc::Rc};

use async_std::{future::select, task};
use futures::{channel::mpsc, prelude::*};
use log::{trace, warn};
use nix::{libc, sys::signal::Signal};
use xidlehook::{modules::Xcb, timers::CmdTimer, Module, Xidlehook};

mod signal_handler;
mod socket_api;
mod socket_models;

struct Defer<F: FnMut()>(F);
impl<F: FnMut()> Drop for Defer<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}

pub fn main_loop(
    opt: Opt,
    mut xidlehook: Xidlehook<CmdTimer, ((), Vec<Box<dyn Module>>)>,
    xcb: Rc<Xcb>,
) -> xidlehook::Result<()> {
    let (socket_tx, mut socket_rx) = mpsc::channel(4);
    let _scope = if let Some(address) = opt.socket {
        {
            let address = address.clone();
            task::spawn(async move {
                if let Err(err) = socket_api::socket_loop(&address, socket_tx).await {
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

    let (signal_tx, mut signal_rx) = mpsc::channel(1);
    let signal_thread = signal_handler::handle_signals(signal_tx)?;

    loop {
        enum Selected {
            Socket(Option<socket_models::Message>),
            Signal(Option<Signal>),
            Exit(xidlehook::Result<()>),
        }
        let a = socket_rx.next().map(Selected::Socket);
        let b = signal_rx.next().map(Selected::Signal);
        let c = xidlehook.main_async(&xcb).map(Selected::Exit);
        let res = task::block_on(select!(a, b, c));
        match res {
            Selected::Socket(msg) => if let Some(msg) = msg {
                trace!("Got command over socket: {:#?}", msg);
            },
            Selected::Signal(sig) => if let Some(sig) = sig {
                trace!("Signal received: {}", sig);
                break;
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
