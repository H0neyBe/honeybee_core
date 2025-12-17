use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use bee_config::Config;
use bee_message::{
  BidirectionalMessage, BackendCommand, BackendToManagerMessage, BackendType, ManagerToBackendMessage, ManagerToNodeMessage, MessageEnvelope, PROTOCOL_VERSION
};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use super::backend::Backend;
use crate::node_manager::NodeManager;

#[derive(Debug)]
pub struct BackendManager {
  backends: Arc<RwLock<HashMap<u64, Backend>>>,
  listener: TcpListener,
  address: SocketAddr,
  tasks: Arc<RwLock<HashMap<u64, JoinHandle<()>>>>,
  // channel reader and writer for manager to manager communication
  reader:  tokio::sync::mpsc::UnboundedReceiver<BidirectionalMessage>,
  writer:  tokio::sync::mpsc::UnboundedSender<BidirectionalMessage>,

}

impl BackendManager {
  pub async fn build(config: &Config, reader: tokio::sync::mpsc::UnboundedReceiver<BidirectionalMessage>, writer: tokio::sync::mpsc::UnboundedSender<BidirectionalMessage>) -> Result<Self, std::io::Error> {
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
    })
  }

  pub async fn listen(&self) -> Result<(), std::io::Error> {
    log::info!("Starting backend listener on {}", self.address);
    loop {
      let (mut socket, addr) = self.listener.accept().await?;
      let backends = Arc::clone(&self.backends);
      let tasks = Arc::clone(&self.tasks);


      
      tokio::spawn(async move {
        log::debug!("Got a new backend connection from: {}", addr);

        // Read initial message
        let mut buf = vec![0u8; 8192];
        let n = match socket.read(&mut buf).await {
          Ok(0) => {
            log::warn!("Connection closed by {} before sending initial message", addr);
            return;
          }
          Ok(n) => n,
          Err(e) => {
            log::error!("Failed to read from backend {}: {}", addr, e);
            return;
          }
        };

        let msg = String::from_utf8_lossy(&buf[..n]);
        log::debug!("Received backend message: {:#?}", msg);

        // Parse message envelope
        let envelope: MessageEnvelope<BackendToManagerMessage> = match serde_json::from_str(&msg) {
          Ok(env) => env,
          Err(e) => {
            log::error!("Failed to parse backend message from {}: {}", addr, e);
            return;
          }
        };

        // Validate protocol version
        if envelope.version != PROTOCOL_VERSION {
          log::warn!(
            "Version mismatch from backend {}: got {}, expected {}",
            addr,
            envelope.version,
            PROTOCOL_VERSION
          );
        }

        // Generate backend ID
        let backend_id = rand::random::<u64>();

        // Create backend
        let backend = Backend::new(backend_id, BackendType::Web, socket);
        log::info!("Backend {} registered from {}", backend_id, addr);

        // Insert backend into the registry
        backends.write().await.insert(backend_id, backend);

        // Spawn task to handle backend commands
        let backends_clone = Arc::clone(&backends);
        let tasks_clone = Arc::clone(&tasks);
        let handle = tokio::spawn(async move {
          log::debug!("Started command handler for backend: {}", backend_id);

          loop {
            // Read command from backend
            let command_envelope = {
              let mut backends_guard = backends_clone.write().await;
              let backend = match backends_guard.get_mut(&backend_id) {
                Some(b) => b,
                None => {
                  log::warn!("Backend {} not found in registry", backend_id);
                  break;
                }
              };

              match backend.read_command().await {
                Ok(cmd) => cmd,
                Err(e) => {
                  let err_msg = e.to_string();
                  if err_msg.contains("Connection closed") {
                    log::info!("Backend {} disconnected (connection closed)", backend_id);
                  } else {
                    log::error!("Error reading from backend {}: {}", backend_id, e);
                  }
                  break;
                }
              }
            };

            log::debug!("Received command from backend {}: {:?}", backend_id, command_envelope.message);

            // Process command and send response
            match command_envelope.message {
              BackendToManagerMessage::BackendCommand(cmd) => {
                Self::handle_backend_command(
                  backend_id,
                  cmd,
                  &backends_clone,
                ).await;
              }
              _ => {
                log::warn!("Unexpected message type from backend {}", backend_id);
                let mut backends_guard = backends_clone.write().await;
                if let Some(backend) = backends_guard.get_mut(&backend_id) {
                  let _ = backend.send_error("Unexpected message type".to_string()).await;
                }
              }
            }
          }

          // Clean up: remove backend from registry
          if let Some(mut backend) = backends_clone.write().await.remove(&backend_id) {
            backend.disconnect().await;
            log::info!("Removed backend {} from registry", backend_id);
          }

          // Remove task handle
          tasks_clone.write().await.remove(&backend_id);
        });

        // Store task handle
        tasks.write().await.insert(backend_id, handle);
      });
    }
  }


  /// Handle a command from a backend
  async fn handle_backend_command(
    &mut self,
    backend_id: u64,
    command: BackendCommand,
    backends: &Arc<RwLock<HashMap<u64, Backend>>>,
  ) {
    log::info!("Processing command from backend {}: {:?}", backend_id, command);

    let result = match command {
      BackendCommand::GetNodes => {
        // Get all nodes
        let nodes = node_manager.get_nodes().await;
        log::debug!("Retrieved nodes: {:?}", nodes);
        match serde_json::to_value(&nodes) {
          Ok(data) => Ok((Some("Retrieved nodes successfully".to_string()), Some(data))),
          Err(e) => Err(format!("Failed to serialize nodes: {}", e)),
        }
      }
      BackendCommand::GetNode { node_id } => {
        // Get specific node
        match node_manager.get_node(node_id).await {
          Some(node) => {
            match serde_json::to_value(&node) {
              Ok(data) => Ok((Some(format!("Retrieved node {}", node_id)), Some(data))),
              Err(e) => Err(format!("Failed to serialize node: {}", e)),
            }
          }
          None => Err(format!("Node {} not found", node_id)),
        }
      }
      BackendCommand::GetNodesByStatus { status } => {
        // Get nodes by status
        let nodes = node_manager.get_nodes_by_status(status.clone()).await;
        match serde_json::to_value(&nodes) {
          Ok(data) => Ok((Some(format!("Retrieved {} nodes with status {:?}", nodes.len(), status)), Some(data))),
          Err(e) => Err(format!("Failed to serialize nodes: {}", e)),
        }
      }
      BackendCommand::NodeCommand { node_id, command: node_command } => {
        // Send command to specific node
        let cmd = bee_message::NodeCommand {
          node_id,
          command: node_command,
        };
        match node_manager.send_command_to_node(node_id, ManagerToNodeMessage::NodeCommand(cmd)).await {
          Ok(_) => Ok((Some(format!("Command sent to node {}", node_id)), None)),
          Err(e) => Err(e),
        }
      }
      BackendCommand::BroadcastCommand { command: node_command } => {
        // Broadcast command to all nodes
        let cmd = bee_message::NodeCommand {
          node_id: 0, // 0 indicates broadcast
          command: node_command,
        };
        node_manager.broadcast_command(ManagerToNodeMessage::NodeCommand(cmd)).await;
        Ok((Some("Command broadcast to all nodes".to_string()), None))
      }
      BackendCommand::GetNodeCount => {
        // Get node count
        let count = node_manager.node_count().await;
        match serde_json::to_value(&count) {
          Ok(data) => Ok((Some(format!("Total nodes: {}", count)), Some(data))),
          Err(e) => Err(format!("Failed to serialize count: {}", e)),
        }
      }
      BackendCommand::RemoveNode { node_id } => {
        // Remove a node
        match node_manager.remove_node(node_id).await {
          Some(_) => Ok((Some(format!("Node {} removed", node_id)), None)),
          None => Err(format!("Node {} not found", node_id)),
        }
      }
      BackendCommand::GetActiveConnections => {
        // Get active connections count
        let count = node_manager.active_connections().await;
        match serde_json::to_value(&count) {
          Ok(data) => Ok((Some(format!("Active connections: {}", count)), Some(data))),
          Err(e) => Err(format!("Failed to serialize count: {}", e)),
        }
      }
      _ => Err("Command not implemented".to_string()),
    };

    // Send response back to backend
    let mut backends_guard = backends.write().await;
    if let Some(backend) = backends_guard.get_mut(&backend_id) {
      match result {
        Ok((message, data)) => {
          if let Err(e) = backend.send_success(message, data).await {
            log::error!("Failed to send success response to backend {}: {}", backend_id, e);
          }
        }
        Err(error) => {
          if let Err(e) = backend.send_error(error).await {
            log::error!("Failed to send error response to backend {}: {}", backend_id, e);
          }
        }
      }
    }
  }

  /// Get a reference to all registered backends
  pub async fn get_backends(&self) -> Arc<RwLock<HashMap<u64, Backend>>> {
    Arc::clone(&self.backends)
  }

  /// Get the number of active backend connections
  pub async fn active_connections(&self) -> usize {
    self.tasks.read().await.len()
  }

  /// Get a specific backend by ID
  pub async fn get_backend(&self, backend_id: u64) -> Option<Backend> {
    self.backends.read().await.get(&backend_id).cloned()
  }

  /// Remove a backend from the registry
  pub async fn remove_backend(&self, backend_id: u64) -> Option<Backend> {
    let mut backend = self.backends.write().await.remove(&backend_id);
    if let Some(ref mut b) = backend {
      b.disconnect().await;
    }

    // Also remove the task handle
    if let Some(handle) = self.tasks.write().await.remove(&backend_id) {
      handle.abort();
    }

    backend
  }

  /// Get count of registered backends
  pub async fn backend_count(&self) -> usize {
    self.backends.read().await.len()
  }

  /// Get backends by type
  pub async fn get_backends_by_type(&self, backend_type: BackendType) -> Vec<Backend> {
    self.backends
      .read()
      .await
      .values()
      .filter(|b| b.backend_type == backend_type)
      .cloned()
      .collect()
  }
}