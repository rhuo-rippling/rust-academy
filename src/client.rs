use academy::{verify_client, Stdout};
use academy::{ClientConfig, ClientEvent, Message, ServerEvent, User};
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex};

pub struct Client {
    config: ClientConfig,
}

impl Client {
    fn new(config: ClientConfig) -> Self {
        Self { config }
    }
    async fn start(config: ClientConfig) -> Result<()> {
        let mut client = Self::new(config);
        client.run().await.context("Error in Client::run")?;
        Ok(())
    }

    async fn run(&mut self) -> Result<()> {
        let stream = TcpStream::connect(&self.config.addr).await?;
        let (stream_rx, mut stream_tx) = stream.into_split();
        let stream_tx = Arc::new(Mutex::new(BufWriter::new(stream_tx)));
        let stream_rx = Arc::new(Mutex::new(BufReader::new(stream_rx)));
        let (write_tx, write_rx): (
            Sender<Arc<Mutex<BufWriter<OwnedWriteHalf>>>>,
            Receiver<Arc<Mutex<BufWriter<OwnedWriteHalf>>>>,
        ) = mpsc::channel(64);
        let (read_tx, read_rx): (
            Sender<Arc<Mutex<BufReader<OwnedReadHalf>>>>,
            Receiver<Arc<Mutex<BufReader<OwnedReadHalf>>>>,
        ) = mpsc::channel(64);
        write_tx.send(Arc::clone(&stream_tx)).await;
        let stdout = self.config.stdout.clone();
        let write_task = tokio::spawn(async move {
            write_handler(write_rx, read_tx, Arc::clone(&stream_rx)).await;
        });
        let read_task = tokio::spawn(async move {
            read_handler(read_rx, write_tx, Arc::clone(&stream_tx), stdout).await;
        });

        tokio::select! {
            _ = write_task => {
            println!("write_handler completed");
            }
            _ = read_task => {
                println!("read_handler completed");
            }
        };
        Ok(())
    }
}
async fn write_handler(
    mut write_rx: Receiver<Arc<Mutex<BufWriter<OwnedWriteHalf>>>>,
    read_tx: Sender<Arc<Mutex<BufReader<OwnedReadHalf>>>>,
    stream_rx: Arc<Mutex<BufReader<OwnedReadHalf>>>,
) -> Result<()> {
    while let Some(stream_tx) = write_rx.recv().await {
        println!("write_handler Received message");
        let user = User {
            name: String::from("test-name"),
        };
        let ident_event = ClientEvent::Ident(user.clone());
        let event_json = serde_json::to_string(&ident_event).unwrap();
        let mut stream_tx = stream_tx.lock().await;
        stream_tx.write_all(event_json.as_bytes()).await;
        stream_tx.flush().await;
    }
    read_tx.send(stream_rx).await;
    println!("write_handler channel closed");
    Ok(())
}

async fn read_handler(
    mut read_rx: Receiver<Arc<Mutex<BufReader<OwnedReadHalf>>>>,
    write_tx: Sender<Arc<Mutex<BufWriter<OwnedWriteHalf>>>>,
    stream_wx: Arc<Mutex<BufWriter<OwnedWriteHalf>>>,
    stdout: Stdout,
) -> Result<()> {
    while let Some(stream_rx) = read_rx.recv().await {
        println!("write_handler Received message");
        let mut reader = stream_rx.lock().await;
        let mut message = String::new();
        reader.read_line(&mut message).await.unwrap();
        let message = message.trim();
        let event = serde_json::from_str::<ServerEvent>(&message).unwrap();
        match event {
            ServerEvent::Message(Message { from, text, .. }) => {
                let mut stdout = stdout.lock().unwrap();
                writeln!(
                    stdout.as_mut().unwrap(),
                    "{}: {}",
                    from.name.as_str(),
                    text.as_str()
                );
                stdout.as_mut().unwrap().flush();
            }
            _ => panic!("bad event"),
        }
        write_tx.send(stream_wx.clone()).await;
    }
    println!("read_handler channel closed");
    Ok(())
}
#[cfg(test)]
pub mod tests {
    use super::*;

    #[tokio::test]
    async fn test_harness() {
        academy::verify_client(Client::start).await;
    }
}
