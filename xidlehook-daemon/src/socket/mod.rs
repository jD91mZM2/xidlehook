use std::convert::Infallible;

use tokio::{
    io::{BufReader, BufWriter},
    net::UnixListener,
    prelude::*,
    sync::{mpsc, oneshot},
};
use log::{trace, warn};

pub mod handler;
pub mod models;

pub use self::models::*;

pub async fn main_loop(
    address: &str,
    socket_tx: mpsc::Sender<(Message, oneshot::Sender<Reply>)>,
) -> xidlehook_core::Result<Infallible> {
    let mut listener = UnixListener::bind(address)?;
    trace!("Bound unix listener on address {:?}", address);

    loop {
        let (mut stream, addr) = listener.accept().await?;
        trace!("Connection from {:?}", addr);

        let mut socket_tx = socket_tx.clone();
        tokio::spawn(async move {
            let (reader, writer) = stream.split();
            let reader = BufReader::new(reader);
            let mut writer = BufWriter::new(writer);
            let mut lines = reader.lines();
            while let Some(msg) = lines.next_line().await.ok().and_then(|inner| inner) {
                let res = serde_json::from_str(&msg).map_err(|err| err.to_string());

                let msg: Message = match res {
                    Ok(json) => json,
                    Err(err) => {
                        warn!("couldn't interpret message: {}", err);
                        continue;
                    },
                };

                let (reply_tx, reply_rx) = oneshot::channel();
                socket_tx.send((msg, reply_tx)).await.expect("receiver closed too early");

                let reply = match reply_rx.await {
                    Ok(reply) => reply,
                    Err(_) => break,
                };

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
