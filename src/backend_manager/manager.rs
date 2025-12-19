use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use bee_config::Config;
use bee_message::{
  BackendCommand,
  BackendRegistration,
  BackendResponse,
  BackendToManagerMessage,
  BidirectionalMessage,
  ManagerToBackendMessage,
  ManagerToNodeMessage,
  MessageEnvelope,
  NodeCommand,
  NodeCommandType,
  PROTOCOL_VERSION,
};
use tokio::io::{
  AsyncBufReadExt,
  AsyncWriteExt,
  BufReader,
};
use tokio::net::TcpListener;
use tokio::sync::{
  Mutex,
  RwLock,
};
use tokio::task::JoinHandle;

use super::backend::Backend;
use crate::node_manager::manager::NodeManager;

#[derive(Debug)]
pub struct BackendManager {
  backends:     Arc<RwLock<HashMap<u64, Backend>>>,
  listener:     TcpListener,
  address:      SocketAddr,
  tasks:        Arc<RwLock<HashMap<u64, JoinHandle<()>>>>,
  reader:       tokio::sync::mpsc::UnboundedReceiver<BidirectionalMessage>,
  writer:       tokio::sync::mpsc::UnboundedSender<BidirectionalMessage>,
  node_manager: Arc<NodeManager>,
}

impl BackendManager {
  pub async fn build(
    config: &Config, reader: tokio::sync::mpsc::UnboundedReceiver<BidirectionalMessage>,
    writer: tokio::sync::mpsc::UnboundedSender<BidirectionalMessage>, node_manager: Arc<NodeManager>,
  ) -> Result<Self, std::io::Error> {
    let address = format!("{}:{}", config.server.host, config.server.backend_port);
    let listener = TcpListener::bind(&address).await?;
    let addr = listener.local_addr()?;

    log::info!("Backend Manager listening on: {}", addr);

    Ok(BackendManager {
      backends: Arc::new(RwLock::new(HashMap::new())),
      listener,
      address: addr,
      tasks: Arc::new(RwLock::new(HashMap::new())),
      reader,
      writer,
      node_manager,
    })
  }

  pub async fn listen(&self) -> Result<(), std::io::Error> {
    log::info!("Starting TCP backend listener on {}", self.address);

    loop {
      let (stream, addr) = self.listener.accept().await?;
      let backends = Arc::clone(&self.backends);
      let tasks = Arc::clone(&self.tasks);
      let node_manager = Arc::clone(&self.node_manager);

      log::debug!("New backend connection from: {}", addr);

      tokio::spawn(async move {
        if let Err(e) = Self::handle_backend_connection(stream, backends, tasks, node_manager).await {
          log::error!("Backend connection error from {}: {}", addr, e);
        }
      });
    }
  }

  async fn handle_backend_connection(
    stream: tokio::net::TcpStream, backends: Arc<RwLock<HashMap<u64, Backend>>>,
    tasks: Arc<RwLock<HashMap<u64, JoinHandle<()>>>>, node_manager: Arc<NodeManager>,
  ) -> Result<(), String> {
    log::info!("New TCP backend connection established");

    // Split the stream into read and write halves
    let (read_half, write_half) = stream.into_split();
    let write_half = Arc::new(Mutex::new(write_half));
    let mut reader = BufReader::new(read_half);
    let mut buffer = String::new();

    // Wait for initial registration message
    reader
      .read_line(&mut buffer)
      .await
      .map_err(|e| e.to_string())?;

    let backend_id = Self::handle_registration(&buffer).await?;
    log::info!("Backend {} registered successfully", backend_id);

    // Send registration acknowledgment
    let ack = BackendResponse::Success {
      message: Some("Registration successful".to_string()),
      data:    Some(serde_json::json!({ "backend_id": backend_id })),
    };
    let ack_envelope = MessageEnvelope::new(PROTOCOL_VERSION, ManagerToBackendMessage::BackendResponse(ack));

    let response = serde_json::to_string(&ack_envelope).map_err(|e| e.to_string())? + "\n";
    {
      let mut writer = write_half.lock().await;
      writer
        .write_all(response.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
      writer.flush().await.map_err(|e| e.to_string())?;
    }

    // Create backend entry with write half only
    let backend = Backend::new_with_writer(backend_id, bee_message::BackendType::Web, Arc::clone(&write_half));
    backends.write().await.insert(backend_id, backend);

    // Handle messages from this backend
    loop {
      buffer.clear();

      // Read the next command
      match reader.read_line(&mut buffer).await {
        Ok(0) => {
          log::info!("Backend {} disconnected (EOF)", backend_id);
          break;
        }
        Ok(_) => {
          let envelope: Result<MessageEnvelope<BackendToManagerMessage>, _> = serde_json::from_str(buffer.trim());

          match envelope {
            Ok(env) => {
              log::debug!("Received command from backend {}: {:?}", backend_id, env.message);

              let response = Self::handle_backend_message(env.message, backend_id, &backends, &node_manager).await;

              // Send response without locking the backends HashMap
              let response_envelope = MessageEnvelope::new(PROTOCOL_VERSION, ManagerToBackendMessage::BackendResponse(response));
              let response_json = serde_json::to_string(&response_envelope).map_err(|e| e.to_string())? + "\n";

              let mut writer = write_half.lock().await;
              if let Err(e) = writer.write_all(response_json.as_bytes()).await {
                log::error!("Error sending response to backend {}: {}", backend_id, e);
                break;
              }
              if let Err(e) = writer.flush().await {
                log::error!("Error flushing response to backend {}: {}", backend_id, e);
                break;
              }
            }
            Err(e) => {
              log::error!("Failed to parse message from backend {}: {}", backend_id, e);
            }
          }
        }
        Err(e) => {
          log::error!("Error reading from backend {}: {}", backend_id, e);
          break;
        }
      }
    }

    // Cleanup
    log::info!("Backend {} disconnected", backend_id);
    backends.write().await.remove(&backend_id);
    tasks.write().await.remove(&backend_id);

    Ok(())
  }

  async fn handle_registration(msg: &str) -> Result<u64, String> {
    let envelope: MessageEnvelope<BackendToManagerMessage> =
      serde_json::from_str(msg.trim()).map_err(|e| format!("Failed to parse registration: {}", e))?;

    if envelope.version != PROTOCOL_VERSION {
      return Err(format!(
        "Protocol version mismatch: got {}, expected {}",
        envelope.version, PROTOCOL_VERSION
      ));
    }

    match envelope.message {
      BackendToManagerMessage::BackendRegistration(reg) => Ok(reg.backend_id),
      _ => Err("Expected BackendRegistration message".to_string()),
    }
  }

  async fn handle_backend_message(
    message: BackendToManagerMessage, backend_id: u64, backends: &Arc<RwLock<HashMap<u64, Backend>>>,
    node_manager: &Arc<NodeManager>,
  ) -> BackendResponse {
    log::debug!("Processing command from backend {}: {:?}", backend_id, message);

    match message {
      BackendToManagerMessage::BackendCommand(cmd) => {
        Self::process_backend_command(backend_id, cmd, backends, node_manager).await
      }
      BackendToManagerMessage::BackendDrop => {
        log::info!("Backend {} requested disconnect", backend_id);
        BackendResponse::Success {
          message: Some("Disconnect acknowledged".to_string()),
          data:    None,
        }
      }
      _ => BackendResponse::Failure("Unexpected message type after registration".to_string()),
    }
  }

  async fn process_backend_command(
    backend_id: u64, command: BackendCommand, backends: &Arc<RwLock<HashMap<u64, Backend>>>, node_manager: &Arc<NodeManager>,
  ) -> BackendResponse {
    log::info!("Processing command from backend {}: {:?}", backend_id, command);

    match command {
      BackendCommand::GetNodes => {
        let node_ids = node_manager.get_nodes().await;
        log::debug!("Retrieved {} nodes", node_ids.len());

        BackendResponse::Success {
          message: Some(format!("Retrieved {} nodes", node_ids.len())),
          data:    Some(serde_json::json!({"nodes": node_ids})),
        }
      }
      BackendCommand::GetNode { node_id } => match node_manager.get_node(node_id).await {
        Some(node) => BackendResponse::Success {
          message: Some(format!("Node {} retrieved", node_id)),
          data:    Some(serde_json::json!(node)),
        },
        None => BackendResponse::Failure(format!("Node {} not found", node_id)),
      },
      BackendCommand::NodeCommand { node_id, command } => {
        let node_command = ManagerToNodeMessage::NodeCommand(NodeCommand { node_id, command });

        match node_manager
          .send_command_to_node(node_id, node_command)
          .await
        {
          Ok(_) => BackendResponse::Success {
            message: Some(format!("Command sent to node {}", node_id)),
            data:    None,
          },
          Err(e) => BackendResponse::Failure(format!("Failed to send command to node {}: {}", node_id, e)),
        }
      }
      BackendCommand::BroadcastCommand { command } => {
        let node_command = ManagerToNodeMessage::NodeCommand(NodeCommand {
          node_id: 0, // Will be ignored in broadcast
          command,
        });

        node_manager.broadcast_command(node_command).await;

        BackendResponse::Success {
          message: Some("Command broadcast to all nodes".to_string()),
          data:    None,
        }
      }
      BackendCommand::GetNodesByStatus { status } => {
        let nodes = node_manager
          .get_nodes_by_status(status.clone())
          .await;

        BackendResponse::Success {
          message: Some(format!("Retrieved {} nodes with status {:?}", nodes.len(), status)),
          data:    Some(serde_json::json!({"nodes": nodes})),
        }
      }
      BackendCommand::GetNodeCount => {
        let count = node_manager.node_count().await;

        BackendResponse::Success {
          message: Some(format!("Total nodes: {}", count)),
          data:    Some(serde_json::json!({"count": count})),
        }
      }
      BackendCommand::RemoveNode { node_id } => match node_manager.remove_node(node_id).await {
        Some(_) => BackendResponse::Success {
          message: Some(format!("Node {} removed", node_id)),
          data:    None,
        },
        None => BackendResponse::Failure(format!("Node {} not found", node_id)),
      },
      BackendCommand::GetActiveConnections => {
        let count = node_manager.active_connections().await;

        BackendResponse::Success {
          message: Some(format!("Active connections: {}", count)),
          data:    Some(serde_json::json!({"count": count})),
        }
      }
    }
  }

  // Helper methods for backend management
  pub async fn get_backends(&self) -> Arc<RwLock<HashMap<u64, Backend>>> { Arc::clone(&self.backends) }

  pub async fn active_connections(&self) -> usize { self.tasks.read().await.len() }

  pub async fn get_backend(&self, backend_id: u64) -> Option<Backend> {
    self
      .backends
      .read()
      .await
      .get(&backend_id)
      .cloned()
  }

  pub async fn remove_backend(&self, backend_id: u64) -> Option<Backend> {
    let backend = self.backends.write().await.remove(&backend_id);

    if let Some(handle) = self.tasks.write().await.remove(&backend_id) {
      handle.abort();
    }

    backend
  }

  pub async fn backend_count(&self) -> usize { self.backends.read().await.len() }

  pub async fn get_backends_by_type(&self, backend_type: bee_message::BackendType) -> Vec<Backend> {
    self
      .backends
      .read()
      .await
      .values()
      .filter(|b| b.backend_type == backend_type)
      .cloned()
      .collect()
  }
}
