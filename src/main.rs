use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:5000").await.unwrap();

    let peers = Arc::new(Mutex::new(HashMap::new()));

    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(stream, addr, Arc::clone(&peers)));
    }
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    peers: Arc<Mutex<HashMap<SocketAddr, mpsc::UnboundedSender<String>>>>,
) {
    let ws_stream = tokio_tungstenite::accept_async(stream).await.unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    peers.lock().await.insert(addr, tx);
    let (mut write, mut read) = ws_stream.split();

    let clone_peers = Arc::clone(&peers);

    let rx_fut = async move {
        while let Some(Ok(msg)) = read.next().await {
            if let Message::Text(msg) = msg {
                let peers = clone_peers.lock().await;

                let senders = peers
                    .iter()
                    .filter(|(&peer_addr, _)| peer_addr != addr)
                    .map(|(_, sender)| sender);

                for tx in senders {
                    tx.send(msg.clone()).unwrap();
                }
            }
        }
    };

    tokio::pin!(rx_fut);

    loop {
        tokio::select! {
            _ = &mut rx_fut => {
                // 受信側のWebSocketが切れた
                break;
            }

            Some(msg) = rx.recv() => {
                // 他のWebSocketから通信が来た
                write.send(Message::Text(msg)).await.unwrap();
            }
        }
    }

    peers.lock().await.remove(&addr).unwrap();
}
