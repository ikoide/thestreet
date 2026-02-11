use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::client_async;
use tokio_tungstenite::WebSocketStream;
use url::Url;

use street_common::crypto::Keypair;
use street_common::ids::new_message_id;
use street_protocol::signing::unsigned_envelope;
use street_protocol::{ClientAuth, Envelope, ServerHello, ServerWelcome};

pub enum OutgoingMessage {
    Envelope(Envelope),
    Close,
}

pub struct Connection {
    pub outgoing: mpsc::UnboundedSender<OutgoingMessage>,
    pub incoming: mpsc::UnboundedReceiver<Envelope>,
    pub welcome: ServerWelcome,
}

trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncStream for T {}

pub async fn connect(
    config: &street_common::config::ClientConfig,
    signing_key: &ed25519_dalek::SigningKey,
) -> anyhow::Result<Connection> {
    let url = Url::parse(&config.relay_url)?;
    let host = url.host_str().ok_or_else(|| anyhow::anyhow!("invalid relay_url"))?;
    let port = url.port_or_known_default().ok_or_else(|| anyhow::anyhow!("invalid relay_url"))?;
    let addr = format!("{host}:{port}");

    let stream: Box<dyn AsyncStream> = if config.tor_enabled {
        let proxy = config
            .socks5_proxy
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("socks5_proxy required when tor_enabled"))?;
        let socks = tokio_socks::tcp::Socks5Stream::connect(proxy.as_str(), addr).await?;
        Box::new(socks)
    } else {
        let tcp = TcpStream::connect(addr).await?;
        Box::new(tcp)
    };

    let (ws_stream, _) = client_async(config.relay_url.clone(), stream).await?;
    let ws_stream: WebSocketStream<Box<dyn AsyncStream>> = ws_stream;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    let hello_env = ws_read
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("no server hello"))??;
    let hello_env: Envelope = serde_json::from_str(hello_env.to_text()?)?;
    if hello_env.message_type != "server.hello" {
        anyhow::bail!("expected server.hello")
    }
    let hello: ServerHello = serde_json::from_value(hello_env.payload)?;

    let keypair = Keypair::from_signing_key_bytes(signing_key.to_bytes());
    let challenge_sig = street_common::crypto::sign_bytes(signing_key, hello.challenge.as_bytes());
    let auth = ClientAuth {
        pubkey: keypair.verifying_key_base64(),
        challenge_sig,
        client_version: "0.1".to_string(),
    };
    let auth_env = unsigned_envelope("client.auth", &new_message_id(), now_ms(), &auth)?;
    ws_write
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&auth_env)?,
        ))
        .await?;

    let welcome_env = ws_read
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("no server welcome"))??;
    let welcome_env: Envelope = serde_json::from_str(welcome_env.to_text()?)?;
    if welcome_env.message_type != "server.welcome" {
        anyhow::bail!("expected server.welcome")
    }
    let welcome: ServerWelcome = serde_json::from_value(welcome_env.payload)?;

    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<OutgoingMessage>();
    let (in_tx, in_rx) = mpsc::unbounded_channel::<Envelope>();

    let mut ws_write_task = ws_write;
    tokio::spawn(async move {
        use tokio_tungstenite::tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};
        use std::borrow::Cow;
        while let Some(msg) = out_rx.recv().await {
            match msg {
                OutgoingMessage::Envelope(env) => {
                    if let Ok(text) = serde_json::to_string(&env) {
                        let _ = ws_write_task
                            .send(tokio_tungstenite::tungstenite::Message::Text(text))
                            .await;
                    }
                }
                OutgoingMessage::Close => {
                    let frame = CloseFrame {
                        code: CloseCode::Normal,
                        reason: Cow::Borrowed("client exit"),
                    };
                    let _ = ws_write_task
                        .send(tokio_tungstenite::tungstenite::Message::Close(Some(frame)))
                        .await;
                    break;
                }
            }
        }
    });

    let mut ws_read_task = ws_read;
    tokio::spawn(async move {
        while let Some(msg) = ws_read_task.next().await {
            if let Ok(msg) = msg {
                if msg.is_text() {
                    if let Ok(env) = serde_json::from_str::<Envelope>(msg.to_text().unwrap_or("")) {
                        let _ = in_tx.send(env);
                    }
                }
            }
        }
    });

    Ok(Connection {
        outgoing: out_tx,
        incoming: in_rx,
        welcome,
    })
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_millis() as i64
}
