use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Router};
use futures::lock::Mutex;
use futures::stream::{SplitSink, StreamExt};
use futures::SinkExt;
use uuid::Uuid;

use std::collections::HashMap;
use std::sync::Arc;

pub struct State {
    pub connections: Mutex<HashMap<Uuid, SplitSink<WebSocket, Message>>>,
}

#[tokio::main]
async fn main() {
    let gosplan_url = std::env::var("GOSPLAN_URL").unwrap_or_else(|_| "127.0.0.1:5000".to_string());

    let state = Arc::new(State {
        connections: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/", get(ws_handler))
        .layer(Extension(state));

    axum_server::bind(gosplan_url.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<State>) {
    let (mut sender, mut reader) = socket.split();

    let uuid = Uuid::new_v4();

    let _ = sender.send(Message::Text(uuid.to_string())).await;

    {
        let mut connections = state.connections.lock().await;
        connections.insert(uuid, sender);
    }

    while let Some(Ok(message)) = reader.next().await {
        match message {
            Message::Close(_) => {
                let mut connections = state.connections.lock().await;
                connections.remove(&uuid);
            }
            Message::Text(_) => {
                let mut connections = state.connections.lock().await;
                let message = message.clone();
                for connection in connections.iter_mut() {
                    let _ = connection.1.send(message.clone()).await;
                }
            }
            _ => {}
        }
    }

    let mut connections = state.connections.lock().await;
    connections.remove(&uuid);
}
