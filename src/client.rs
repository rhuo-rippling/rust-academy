use academy::verify_client;
use academy::{ClientConfig, ClientEvent, Message, ServerEvent, User};
use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
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
        tokio::spawn(async move {
            let user = User {
                name: String::from("test-name"),
            };
            let ident_event = ClientEvent::Ident(user.clone());
            let event_json = serde_json::to_string(&ident_event).unwrap();
            stream_tx.write_all(event_json.as_bytes()).await;
            stream_tx.flush().await;
        })
        .await?;
        let mut reader = BufReader::new(stream_rx);
        let stdout = &self.config.stdout.clone();
        loop {
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
                    )?;
                    stdout.as_mut().unwrap().flush()?;
                    break;
                }
                _ => panic!("bad event: {event:?}"),
            }
        }

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[tokio::test]
    async fn test_harness() {
        academy::verify_client(Client::start).await;
    }
}
