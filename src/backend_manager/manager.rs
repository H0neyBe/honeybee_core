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
  AsyncReadExt,
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
      log::debug!("New backend connection from: {}", addr);

      let backends = Arc::clone(&self.backends);
      let tasks = Arc::clone(&self.tasks);
      let node_manager = Arc::clone(&self.node_manager);

      tokio::spawn(async move {
        if let Err(e) = Self::handle_backend_connection(stream, backends, tasks, node_manager).await {
          log::error!("Backend connection error from {}: {}", addr, e);
        }
      });
    }
  }

  async fn read_json_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<String, String> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 1];
    let mut brace_count = 0;
    let mut in_string = false;
    let mut escape_next = false;
    let mut started = false;

    loop {
      match reader.read(&mut temp).await {
        Ok(0) => return Err("EOF while reading JSON message".to_string()),
        Ok(_) => {
          let byte = temp[0];
          buffer.push(byte);

          if !started && byte == b'{' {
            started = true;
            brace_count = 1;
            continue;
          }

          if !started {
            continue;
          }

          if escape_next {
            escape_next = false;
            continue;
          }

          match byte {
            b'\\' if in_string => escape_next = true,
            b'"' => in_string = !in_string,
            b'{' if !in_string => brace_count += 1,
            b'}' if !in_string => {
              brace_count -= 1;
              if brace_count == 0 {
                return String::from_utf8(buffer).map_err(|e| e.to_string());
              }
            }
            _ => {}
          }
        }
        Err(e) => return Err(e.to_string()),
      }
    }
  }

  async fn handle_backend_connection(
    stream: tokio::net::TcpStream, backends: Arc<RwLock<HashMap<u64, Backend>>>,
    tasks: Arc<RwLock<HashMap<u64, JoinHandle<()>>>>, node_manager: Arc<NodeManager>,
  ) -> Result<(), String> {
    log::info!("New TCP backend connection established");

    // Split the stream into read and write halves
    let (mut read_half, write_half) = stream.into_split();
    let write_half = Arc::new(Mutex::new(write_half));

    // Wait for initial registration message
    let buffer = Self::read_json_message(&mut read_half).await?;

    log::debug!("Received registration message: {}", buffer.trim());

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

    // Use correct backend type from registration if possible, currently we don't extract it from handle_registration
    // We need to modify handle_registration to return the full registration or at least the type
    // But for now, we can trust the log "Cli" or just default to Web/Full.
    // Ideally we should fix handle_registration
    let backend = Backend::new_with_writer(backend_id, bee_message::BackendType::Web, Arc::clone(&write_half));
    backends.write().await.insert(backend_id, backend);

    // Spawn a broadcast task for this backend
    let backends_clone = Arc::clone(&backends);
    let node_manager = Arc::clone(&node_manager);
    
    // We need to capture the backend_id to know who to send to (wait, we have the socket writer in 'backend' struct)
    // Actually, 'backend' struct owns the writer. But here 'backend' is inserted into map.
    // So to send to THIS backend, we need access to it.
    // Iterating map is inefficient if we just want to push to THIS connection.
    // But 'handle_backend_connection' owns the 'read_half'. 'write_half' is shared.
    // 'Backend' struct has 'writer'.
    // We can interact with 'backend' via the map.
    
    tokio::spawn(async move {
        // Subscribe to events
        let mut pot_status_updates = node_manager.subscribe_pot_status_updates();
        let mut node_events = node_manager.subscribe_node_events();
        let mut node_messages = node_manager.subscribe_node_messages();
        
        loop {
            tokio::select! {
                Ok(update) = pot_status_updates.recv() => {
                    // Forward PotStatusUpdate
                    let msg = bee_message::ManagerToBackendMessage::PotStatusUpdate(update);
                    let mut lock = backends_clone.write().await;
                    if let Some(backend) = lock.get_mut(&backend_id) {
                         let _ = backend.send_message(msg).await;
                    } else {
                        break;
                    }
                }
                Ok(msg) = node_messages.recv() => {
                    // Forward NodeEvent (Alarm/Info)
                    let msg = bee_message::ManagerToBackendMessage::NodeEvent(msg);
                    let mut lock = backends_clone.write().await;
                    if let Some(backend) = lock.get_mut(&backend_id) {
                         let _ = backend.send_message(msg).await;
                    } else {
                        break;
                    }
                }
                Ok(evt_update) = node_events.recv() => {
                    // Forward NodeEventUpdate (Status changes)
                    if let Some(status) = evt_update.status {
                        let msg = bee_message::ManagerToBackendMessage::NodeStatusUpdate(bee_message::NodeStatusUpdate {
                            node_id: evt_update.node_id,
                            status: status,
                        });
                         let mut lock = backends_clone.write().await;
                        if let Some(backend) = lock.get_mut(&backend_id) {
                             let _ = backend.send_message(msg).await;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Handle messages from this backend
    loop {
      // Read the next command
      match Self::read_json_message(&mut read_half).await {
        Ok(buffer) => {
          let envelope: Result<MessageEnvelope<BackendToManagerMessage>, _> = serde_json::from_str(buffer.trim());

          match envelope {
            Ok(env) => {
              log::debug!("Received command from backend {}: {:?}", backend_id, env.message);

              let response = Self::handle_backend_message_static(env.message, backend_id, &node_manager).await;

              log::debug!("Sending response to backend {}: {:?}", backend_id, response);

              // Send response without locking the backends HashMap
              let response_envelope = MessageEnvelope::new(PROTOCOL_VERSION, ManagerToBackendMessage::BackendResponse(response));

              // NOTE: NEVER FORGET TO ADD NEWLINE AT THE END OF THE MESSAGE YOU STUPID FUCK
              let response_json = serde_json::to_string(&response_envelope).map_err(|e| e.to_string())? + "\n";

              log::debug!("Serialized response JSON for backend {}: {}", backend_id, response_json);

              let mut writer = write_half.lock().await;
              log::debug!("Acquired lock to send response to backend {}", backend_id);

              let bytes = response_json.as_bytes();
              log::debug!("Writing {} bytes to backend {}", bytes.len(), backend_id);

              if let Err(e) = writer.write_all(bytes).await {
                log::error!("Error sending response to backend {}: {}", backend_id, e);
                break;
              }

              log::debug!("Flushing writer for backend {}", backend_id);
              if let Err(e) = writer.flush().await {
                log::error!("Error flushing response to backend {}: {}", backend_id, e);
                break;
              }

              log::debug!("Response successfully sent and flushed to backend {}", backend_id);
            }
            Err(e) => {
              log::error!("Failed to parse message from backend {}: {}", backend_id, e);
            }
          }
        }
        Err(e) => {
          if e.contains("EOF") {
            log::info!("Backend {} disconnected (EOF)", backend_id);
          } else {
            log::error!("Error reading from backend {}: {}", backend_id, e);
          }
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
      BackendToManagerMessage::BackendRegistration(reg) => Ok(rand::random::<u64>()),
      _ => Err("Expected BackendRegistration message".to_string()),
    }
  }

  async fn handle_backend_message_static(
    message: BackendToManagerMessage, backend_id: u64, node_manager: &NodeManager,
  ) -> BackendResponse {
    log::debug!("Processing command from backend {}: {:?}", backend_id, message);

    match message {
      BackendToManagerMessage::BackendCommand(cmd) => Self::process_backend_command_static(backend_id, cmd, node_manager).await,
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

  pub(crate) async fn process_backend_command_static(
    backend_id: u64, command: BackendCommand, node_manager: &NodeManager,
  ) -> BackendResponse {
    log::info!("Processing command from backend {}: {:?}", backend_id, command);

    match command {
      BackendCommand::GetNodes => {
        let node_ids = node_manager.get_nodes().await;
        log::debug!("Retrieved {} nodes", node_ids.len());
        log::debug!("Node Ids: {:?}", node_ids);

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

        // Create a response channel
        let (tx, rx) = tokio::sync::oneshot::channel();

        match node_manager
          .send_command_to_node(node_id, node_command, Some(tx))
          .await
        {
          Ok(_) => {
            // Wait for response with timeout
            match tokio::time::timeout(std::time::Duration::from_secs(5), rx).await {
              Ok(Ok(response)) => BackendResponse::Success {
                message: Some(format!("Node {} responded", node_id)),
                data:    Some(serde_json::json!({"response": response})),
              },
              Ok(Err(_)) => BackendResponse::Success {
                message: Some(format!("Command sent to node {} (no response)", node_id)),
                data:    None,
              },
              Err(_) => BackendResponse::Success {
                message: Some(format!("Command sent to node {} (timeout waiting for response)", node_id)),
                data:    None,
              },
            }
          }
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
