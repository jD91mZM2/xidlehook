use std::convert::Infallible;

use async_std::{
    io::{BufReader, BufWriter},
    os::unix::net::UnixListener,
    prelude::*,
    task,
};
use futures::{
    channel::{mpsc, oneshot},
    sink::SinkExt,
};
use log::{trace, warn};

pub mod handler;
pub mod models;

pub use self::models::*;

pub async fn socket_loop(
    address: &str,
    socket_tx: mpsc::Sender<(Message, oneshot::Sender<Reply>)>,
) -> xidlehook::Result<Infallible> {
    let listener = UnixListener::bind(address).await?;
    trace!("Bound unix listener on address {:?}", address);

    loop {
        let (stream, addr) = listener.accept().await?;
        trace!("Connection from {:?}", addr);

        let mut socket_tx = socket_tx.clone();
        task::spawn(async move {
            let reader = BufReader::new(&stream);
            let mut writer = BufWriter::new(&stream);
            let mut lines = reader.lines();
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

                let (reply_tx, reply_rx) = oneshot::channel();
                socket_tx.send((msg, reply_tx)).await.unwrap();

                let reply = reply_rx.await.unwrap();

                let res = async {
                    let msg = serde_json::to_vec(&reply)?;
                    writer.write_all(&msg).await?;
                    writer.write_all(&[b'\n']).await?;
                    writer.flush().await?;
                    Ok::<(), std::io::Error>(())
                };

                if let Err(err) = res.await {
                    warn!("couldn't send reply: {}", err);
                }
            }
        });
    }
}
