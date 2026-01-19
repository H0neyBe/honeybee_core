use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{broadcast, mpsc};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::backend_manager::manager::BackendManager;
use crate::node_manager::NodeManager;
use bee_config::Config;
use bee_message::{
  BackendCommand,
  BackendRegistrationAck,
  BackendResponse,
  BackendToManagerMessage,
  ManagerToBackendMessage,
  MessageEnvelope,
  PROTOCOL_VERSION,
};

#[derive(Clone)]
pub struct WsGateway {
  listen_addr: SocketAddr,
  node_manager: Arc<NodeManager>,
  config: Arc<Config>,
}

#[derive(Clone)]
struct WsState {
  node_manager: Arc<NodeManager>,
  config: Arc<Config>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsIncoming {
  Request { id: String, action: String, params: Option<Value> },
  Subscribe { id: String, topic: String, params: Option<Value> },
  Unsubscribe { id: String, topic: String },
  Ping { id: String },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WsOutgoing {
  Response { id: String, ok: bool, data: Option<Value>, error: Option<String> },
  Event { topic: String, data: Value },
  Subscribed { id: String, topic: String },
  Unsubscribed { id: String, topic: String },
  Pong { id: String },
}

enum OutgoingMessage {
  Json(WsOutgoing),
  Raw(String),
}

#[derive(Debug, Deserialize)]
struct WsNodeCommand {
  node_id: u64,
  command: Value,
}

impl WsGateway {
  pub fn new(listen_addr: SocketAddr, node_manager: Arc<NodeManager>, config: Arc<Config>) -> Self {
    Self { listen_addr, node_manager, config }
  }

  pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
    let state = WsState {
      node_manager: Arc::clone(&self.node_manager),
      config: Arc::clone(&self.config),
    };

    let cors = CorsLayer::new()
      .allow_origin(Any)
      .allow_methods(Any)
      .allow_headers(Any);

    let app = Router::new()
      .route("/", get(websocket_handler))
      .route("/ws", get(websocket_handler))
      .route("/health", get(health_check))
      .layer(cors)
      .layer(TraceLayer::new_for_http())
      .with_state(state);

    log::info!("WebSocket Gateway listening on ws://{}", self.listen_addr);
    log::info!("WebSocket endpoints: ws://{}/ws or ws://{}/", self.listen_addr, self.listen_addr);

    let listener = tokio::net::TcpListener::bind(self.listen_addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
  }
}

async fn health_check() -> &'static str { "WebSocket Gateway OK" }

async fn websocket_handler(
  ws: WebSocketUpgrade, State(state): State<WsState>, ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
  log::info!("WebSocket upgrade request from: {}", addr);
  ws.on_upgrade(move |socket| handle_websocket(socket, state, addr))
}

async fn handle_websocket(ws: WebSocket, state: WsState, client_addr: SocketAddr) {
  log::info!("WebSocket connected: {}", client_addr);

  let (mut ws_sender, mut ws_receiver) = ws.split();
  let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel::<OutgoingMessage>();

  let mut subscriptions: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();
  let mut backend_id: Option<u64> = None;

  let outgoing_task = tokio::spawn(async move {
    while let Some(msg) = outgoing_rx.recv().await {
      let text = match msg {
        OutgoingMessage::Json(payload) => serde_json::to_string(&payload).ok(),
        OutgoingMessage::Raw(payload) => Some(payload),
      };
      if let Some(text) = text {
        if ws_sender.send(Message::Text(text)).await.is_err() {
          break;
        }
      }
    }
  });

  while let Some(msg) = ws_receiver.next().await {
    match msg {
      Ok(Message::Text(text)) => {
        if let Ok(envelope) = serde_json::from_str::<MessageEnvelope<BackendToManagerMessage>>(&text) {
          if let Some(response) = handle_legacy_backend_message(envelope, &state.node_manager, &mut backend_id).await {
            let _ = outgoing_tx.send(response);
          }
          continue;
        }

        let parsed = serde_json::from_str::<WsIncoming>(&text);
        let incoming = match parsed {
          Ok(msg) => msg,
          Err(e) => {
            let _ = outgoing_tx.send(OutgoingMessage::Json(WsOutgoing::Response {
              id: "unknown".to_string(),
              ok: false,
              data: None,
              error: Some(format!("Invalid message: {}", e)),
            }));
            continue;
          }
        };

        match incoming {
          WsIncoming::Ping { id } => {
            let _ = outgoing_tx.send(OutgoingMessage::Json(WsOutgoing::Pong { id }));
          }
          WsIncoming::Request { id, action, params } => {
            let response = handle_request(id, action, params, &state).await;
            let _ = outgoing_tx.send(OutgoingMessage::Json(response));
          }
          WsIncoming::Subscribe { id, topic, params } => {
            if subscriptions.contains_key(&topic) {
              let _ = outgoing_tx.send(OutgoingMessage::Json(WsOutgoing::Subscribed { id, topic }));
              continue;
            }

            match subscribe_topic(topic.clone(), params, &state, outgoing_tx.clone()).await {
              Ok(handle) => {
                subscriptions.insert(topic.clone(), handle);
                let _ = outgoing_tx.send(OutgoingMessage::Json(WsOutgoing::Subscribed { id, topic }));
              }
              Err(e) => {
                let _ = outgoing_tx.send(OutgoingMessage::Json(WsOutgoing::Response {
                  id,
                  ok: false,
                  data: None,
                  error: Some(e),
                }));
              }
            }
          }
          WsIncoming::Unsubscribe { id, topic } => {
            if let Some(handle) = subscriptions.remove(&topic) {
              handle.abort();
            }
            let _ = outgoing_tx.send(OutgoingMessage::Json(WsOutgoing::Unsubscribed { id, topic }));
          }
        }
      }
      Ok(Message::Close(frame)) => {
        log::info!("WebSocket close frame from {}: {:?}", client_addr, frame);
        break;
      }
      Ok(Message::Ping(_)) => {}
      Ok(Message::Pong(_)) => {}
      Ok(Message::Binary(_)) => {}
      Err(e) => {
        log::error!("WebSocket error from {}: {}", client_addr, e);
        break;
      }
    }
  }

  for (_, handle) in subscriptions.drain() {
    handle.abort();
  }
  outgoing_task.abort();

  log::info!("WebSocket connection from {} closed", client_addr);
}

async fn handle_request(
  id: String, action: String, params: Option<Value>, state: &WsState,
) -> WsOutgoing {
  match action.as_str() {
    "nodes.list" => {
      let nodes = state.node_manager.get_nodes().await;
      WsOutgoing::Response { id, ok: true, data: Some(serde_json::json!({"nodes": nodes})), error: None }
    }
    "nodes.get" => {
      let node_id = params.and_then(|p| p.get("node_id").cloned()).and_then(|v| v.as_u64());
      match node_id {
        Some(node_id) => {
          let node = state.node_manager.get_node(node_id).await;
          let data = node.map(|n| serde_json::json!({"node": n}));
          WsOutgoing::Response { id, ok: data.is_some(), data, error: if data.is_none() { Some("Node not found".to_string()) } else { None } }
        }
        None => WsOutgoing::Response { id, ok: false, data: None, error: Some("node_id is required".to_string()) },
      }
    }
    "nodes.count" => {
      let count = state.node_manager.get_nodes().await.len();
      WsOutgoing::Response { id, ok: true, data: Some(serde_json::json!({"count": count})), error: None }
    }
    "nodes.status" => {
      let status = params.and_then(|p| p.get("status").cloned());
      if let Some(status_val) = status {
        let status = serde_json::from_value(status_val);
        match status {
          Ok(status) => {
            let nodes = state.node_manager.get_nodes_by_status(status).await;
            WsOutgoing::Response { id, ok: true, data: Some(serde_json::json!({"nodes": nodes})), error: None }
          }
          Err(_) => WsOutgoing::Response { id, ok: false, data: None, error: Some("Invalid status".to_string()) },
        }
      } else {
        WsOutgoing::Response { id, ok: false, data: None, error: Some("status is required".to_string()) }
      }
    }
    "nodes.active_connections" => {
      let count = state.node_manager.get_nodes().await.len();
      WsOutgoing::Response { id, ok: true, data: Some(serde_json::json!({"active_connections": count})), error: None }
    }
    "node.command" => {
      let cmd = params.and_then(|p| serde_json::from_value::<WsNodeCommand>(p).ok());
      match cmd {
        Some(cmd) => {
          let node_command: Result<bee_message::NodeCommandType, _> = serde_json::from_value(cmd.command);
          match node_command {
            Ok(command) => {
              let backend_cmd = BackendCommand::NodeCommand { node_id: cmd.node_id, command };
              let response = BackendManager::process_backend_command_static(0, backend_cmd, &state.node_manager).await;
              WsOutgoing::Response { id, ok: true, data: Some(serde_json::json!({"response": response})), error: None }
            }
            Err(e) => WsOutgoing::Response { id, ok: false, data: None, error: Some(format!("Invalid command: {}", e)) },
          }
        }
        None => WsOutgoing::Response { id, ok: false, data: None, error: Some("params must include node_id and command".to_string()) },
      }
    }
    "command" => {
      let cmd = params.and_then(|p| p.get("command").cloned());
      match cmd {
        Some(command_val) => {
          let command: Result<BackendCommand, _> = serde_json::from_value(command_val);
          match command {
            Ok(command) => {
              let response = BackendManager::process_backend_command_static(0, command, &state.node_manager).await;
              WsOutgoing::Response { id, ok: true, data: Some(serde_json::json!({"response": response})), error: None }
            }
            Err(e) => WsOutgoing::Response { id, ok: false, data: None, error: Some(format!("Invalid command: {}", e)) },
          }
        }
        None => WsOutgoing::Response { id, ok: false, data: None, error: Some("command is required".to_string()) },
      }
    }
    "config.get" => {
      let config = sanitize_config(&state.config);
      WsOutgoing::Response { id, ok: true, data: Some(config), error: None }
    }
    "potstore.list" => {
      match read_potstore() {
        Ok(data) => WsOutgoing::Response { id, ok: true, data: Some(data), error: None },
        Err(e) => WsOutgoing::Response { id, ok: false, data: None, error: Some(e) },
      }
    }
    _ => WsOutgoing::Response { id, ok: false, data: None, error: Some("Unknown action".to_string()) },
  }
}

async fn subscribe_topic(
  topic: String, _params: Option<Value>, state: &WsState, outgoing_tx: mpsc::UnboundedSender<OutgoingMessage>,
) -> Result<tokio::task::JoinHandle<()>, String> {
  match topic.as_str() {
    "pot_logs" => {
      let mut rx = state.node_manager.subscribe_pot_logs();
      Ok(tokio::spawn(async move {
        loop {
          match rx.recv().await {
            Ok(log) => {
              let _ = outgoing_tx.send(OutgoingMessage::Json(WsOutgoing::Event {
                topic: "pot_logs".to_string(),
                data: serde_json::to_value(log).unwrap_or_else(|_| serde_json::json!({})),
              }));
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => break,
          }
        }
      }))
    }
    "nodes" => {
      let mut rx = state.node_manager.subscribe_node_events();
      Ok(tokio::spawn(async move {
        loop {
          match rx.recv().await {
            Ok(event) => {
              let _ = outgoing_tx.send(OutgoingMessage::Json(WsOutgoing::Event {
                topic: "nodes".to_string(),
                data: serde_json::to_value(event).unwrap_or_else(|_| serde_json::json!({})),
              }));
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => break,
          }
        }
      }))
    }
    _ => Err("Unknown topic".to_string()),
  }
}

async fn handle_legacy_backend_message(
  envelope: MessageEnvelope<BackendToManagerMessage>, node_manager: &NodeManager, backend_id: &mut Option<u64>,
) -> Option<OutgoingMessage> {
  match envelope.message {
    BackendToManagerMessage::BackendRegistration(reg) => {
      let id = rand::random::<u64>();
      *backend_id = Some(id);
      let ack = MessageEnvelope::new(
        PROTOCOL_VERSION,
        ManagerToBackendMessage::ManagerRegistrationAck(BackendRegistrationAck {
          backend_id: id,
          accepted: true,
          message: Some(format!("Registration successful: {}", reg.backend_name)),
        }),
      );
      Some(OutgoingMessage::Raw(serde_json::to_string(&ack).ok()?))
    }
    BackendToManagerMessage::BackendCommand(cmd) => {
      let response = BackendManager::process_backend_command_static(backend_id.unwrap_or(0), cmd, node_manager).await;
      let msg = MessageEnvelope::new(PROTOCOL_VERSION, ManagerToBackendMessage::BackendResponse(response));
      Some(OutgoingMessage::Raw(serde_json::to_string(&msg).ok()?))
    }
    BackendToManagerMessage::BackendDrop => None,
  }
}

fn sanitize_config(config: &Config) -> Value {
  serde_json::json!({
    "server": {
      "host": config.server.host,
      "node_port": config.server.node_port,
      "backend_port": config.server.backend_port,
      "debug": config.server.debug
    },
    "logging": {
      "level": config.logging.level,
      "folder": config.logging.folder,
      "force_color": config.logging.force_color,
      "mongodb": config.logging.mongodb
    },
    "proxy": {
      "enabled": config.proxy.enabled,
      "host": config.proxy.host,
      "port": config.proxy.port
    },
    "database": {
      "database": config.database.database,
      "collection": config.database.collection,
      "honeypot_database": config.database.honeypot_database,
      "honeypot_collection": config.database.honeypot_collection
    }
  })
}

fn read_potstore() -> Result<Value, String> {
  let path = std::env::var("HONEYBEE_POTSTORE_PATH").unwrap_or_else(|_| "../honeybee_potstore/potstore.json".to_string());
  let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
  serde_json::from_str(&contents).map_err(|e| e.to_string())
}
