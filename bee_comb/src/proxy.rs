use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{
  Message,
  WebSocket,
  WebSocketUpgrade,
};
use axum::extract::{
  ConnectInfo,
  State,
};
use axum::response::IntoResponse;
use axum::routing::get;
use futures::{
  SinkExt,
  StreamExt,
};
use tokio::io::{
  AsyncBufReadExt,
  AsyncWriteExt,
  BufReader,
};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tower_http::cors::{
  Any,
  CorsLayer,
};
use tower_http::trace::TraceLayer;

/// WebSocket proxy server that bridges WebSocket clients to the TCP backend manager
#[derive(Clone)]
pub struct WebSocketProxy {
  backend_address: String,
}

/// Shared state for the proxy
#[derive(Clone)]
struct ProxyState {
  backend_address: String,
}

impl WebSocketProxy {
  /// Create a new WebSocket proxy
  pub fn new(listen_addr: SocketAddr, backend_address: String) -> Self {
    log::info!("Initializing WebSocket Proxy: {} -> {}", listen_addr, backend_address);

    Self { backend_address }
  }

  /// Start the proxy server
  pub async fn run(self, listen_addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let state = ProxyState {
      backend_address: self.backend_address.clone(),
    };

    // Configure CORS - allow WebSocket upgrades
    let cors = CorsLayer::new()
      .allow_origin(Any)
      .allow_methods(Any)
      .allow_headers(Any);

    // Build the router - WebSocket endpoint at root
    let app = Router::new()
      .route("/", get(websocket_handler))
      .route("/ws", get(websocket_handler))
      .route("/health", get(health_check))
      .layer(cors)
      .layer(TraceLayer::new_for_http())
      .with_state(state);

    log::info!("WebSocket Proxy listening on ws://{}", listen_addr);
    log::info!("WebSocket endpoints: ws://{}/ws or ws://{}/", listen_addr, listen_addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
  }
}

/// Health check endpoint
async fn health_check() -> &'static str { "WebSocket Proxy OK" }

/// WebSocket upgrade handler
async fn websocket_handler(
  ws: WebSocketUpgrade, State(state): State<ProxyState>, ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
  log::info!("WebSocket upgrade request from: {}", addr);

  ws.on_upgrade(move |socket| handle_websocket(socket, state, addr))
}

/// Handle WebSocket connection
async fn handle_websocket(ws: WebSocket, state: ProxyState, client_addr: SocketAddr) {
  log::info!("WebSocket connected: {}", client_addr);

  // Connect to the backend TCP server
  let backend_stream = match TcpStream::connect(&state.backend_address).await {
    Ok(stream) => {
      log::info!("Connected to backend: {} for client {}", state.backend_address, client_addr);
      stream
    }
    Err(e) => {
      log::error!("Failed to connect to backend {}: {}", state.backend_address, e);
      return;
    }
  };

  let (backend_read, backend_write) = backend_stream.into_split();
  let backend_write = Arc::new(Mutex::new(backend_write));
  let mut backend_reader = BufReader::new(backend_read);

  let (mut ws_sender, mut ws_receiver) = ws.split();

  // Spawn task to read from backend and send to WebSocket
  let backend_to_ws = {
    tokio::spawn(async move {
      let mut buffer = String::new();
      loop {
        buffer.clear();
        match backend_reader.read_line(&mut buffer).await {
          Ok(0) => {
            log::info!("Backend connection closed for client {}", client_addr);
            let _ = ws_sender.send(Message::Close(None)).await;
            break;
          }
          Ok(_) => {
            let trimmed = buffer.trim();
            if trimmed.is_empty() {
              log::debug!("Received empty message from backend for client {}, skipping", client_addr);
              continue;
            }

            log::debug!("Backend -> WebSocket ({}): {}", client_addr, trimmed);

            // Forward to WebSocket
            if let Err(e) = ws_sender
              .send(Message::Text(trimmed.to_string()))
              .await
            {
              log::error!("Failed to send to WebSocket {}: {}", client_addr, e);
              break;
            }
          }
          Err(e) => {
            log::error!("Error reading from backend for client {}: {}", client_addr, e);
            break;
          }
        }
      }
    })
  };

  // Handle WebSocket messages and forward to backend
  let ws_to_backend = {
    let backend_write = Arc::clone(&backend_write);
    tokio::spawn(async move {
      while let Some(msg) = ws_receiver.next().await {
        match msg {
          Ok(Message::Text(text)) => {
            log::debug!("WebSocket ({}) -> Backend: {}", client_addr, text);

            // Validate JSON
            if let Err(e) = serde_json::from_str::<serde_json::Value>(&text) {
              log::warn!("Invalid JSON from WebSocket {}: {}", client_addr, e);
              continue;
            }

            // Forward to backend with newline
            let mut writer = backend_write.lock().await;
            let message = format!("{}\n", text);
            if let Err(e) = writer.write_all(message.as_bytes()).await {
              log::error!("Failed to write to backend for client {}: {}", client_addr, e);
              break;
            }
            if let Err(e) = writer.flush().await {
              log::error!("Failed to flush backend writer for client {}: {}", client_addr, e);
              break;
            }
          }
          Ok(Message::Binary(data)) => {
            log::warn!("Received binary message from {}, ignoring: {} bytes", client_addr, data.len());
          }
          Ok(Message::Ping(_)) => {
            log::trace!("Received ping from {}", client_addr);
            // Axum handles pong automatically
          }
          Ok(Message::Pong(_)) => {
            log::trace!("Received pong from {}", client_addr);
          }
          Ok(Message::Close(frame)) => {
            log::info!("WebSocket close frame from {}: {:?}", client_addr, frame);
            break;
          }
          Err(e) => {
            log::error!("WebSocket error from {}: {}", client_addr, e);
            break;
          }
        }
      }
      log::debug!("WebSocket receiver task finished for {}", client_addr);
    })
  };

  // Wait for either task to complete
  tokio::select! {
      _ = backend_to_ws => {
          log::debug!("Backend to WebSocket task completed for {}", client_addr);
      }
      _ = ws_to_backend => {
          log::debug!("WebSocket to backend task completed for {}", client_addr);
      }
  }

  log::info!("WebSocket connection from {} closed", client_addr);
}
