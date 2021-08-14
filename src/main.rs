use futures::{SinkExt, StreamExt, TryStreamExt};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:5000").await.unwrap();

    let peers = Arc::new(Mutex::new(HashMap::new()));

    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(stream, addr, peers.clone()));
    }
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    peers: Arc<Mutex<HashMap<SocketAddr, mpsc::UnboundedSender<String>>>>,
) {
    let ws_stream = tokio_tungstenite::accept_async(stream).await.unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (mut write, read) = ws_stream.split();

    peers.lock().unwrap().insert(addr, tx);

    let clone_peers = peers.clone();
    tokio::spawn(read.try_for_each(move |msg| {
        if let Message::Text(msg) = msg {
            let peers = clone_peers.lock().unwrap();

            let receivers = peers.iter().filter_map(|(peer_addr, receiver)| {
                if peer_addr != &addr {
                    Some(receiver)
                } else {
                    None
                }
            });

            for rx in receivers {
                rx.send(msg.clone()).unwrap();
            }
        }

        async { Ok(()) }
    }));

    while let Some(msg) = rx.recv().await {
        write.send(Message::Text(msg)).await.unwrap();
    }

    peers.lock().unwrap().remove(&addr).unwrap();
}
