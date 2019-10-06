use std::convert::Infallible;

use async_std::{io::BufReader, os::unix::net::UnixListener, prelude::*, task};
use futures::{channel::mpsc, sink::SinkExt};
use log::{trace, warn};

use super::socket_models::*;

pub async fn socket_loop(
    address: &str,
    socket_tx: mpsc::Sender<Message>,
) -> xidlehook::Result<Infallible> {
    let listener = UnixListener::bind(address).await?;
    trace!("Bound unix listener on address {:?}", address);

    loop {
        let (stream, addr) = listener.accept().await?;
        trace!("Connection from {:?}", addr);
        let stream = BufReader::new(stream);

        let mut socket_tx = socket_tx.clone();
        task::spawn(async move {
            let mut lines = stream.lines();
            while let Some(msg) = lines.next().await {
                let res = msg
                    .map_err(|err| err.to_string())
                    .and_then(|msg| serde_json::from_str(&msg).map_err(|err| err.to_string()));
                let msg: Message = match res {
                    Ok(json) => json,
                    Err(err) => {
                        warn!("couldn't interpret message: {}", err);
                        continue;
                    },
                };
                socket_tx.send(msg).await.unwrap();
            }
        });
    }
}
