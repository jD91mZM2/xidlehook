use std::convert::Infallible;

use async_std::{
    io::{BufReader, BufWriter},
    os::unix::net::UnixListener,
    prelude::*,
    task,
    sync,
};
use log::{trace, warn};

pub mod handler;
pub mod models;

pub use self::models::*;

pub async fn main_loop(
    address: &str,
    socket_tx: sync::Sender<(Message, sync::Sender<Reply>)>,
) -> xidlehook_core::Result<Infallible> {
    let listener = UnixListener::bind(address).await?;
    trace!("Bound unix listener on address {:?}", address);

    loop {
        let (stream, addr) = listener.accept().await?;
        trace!("Connection from {:?}", addr);

        let socket_tx = socket_tx.clone();
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

                let (reply_tx, reply_rx) = sync::channel(1);
                socket_tx.send((msg, reply_tx)).await;

                let reply = reply_rx.recv().await;

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
