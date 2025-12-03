use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use bee_config::Config;
use bee_message::{
  ManagerToNodeMessage,
  MessageEnvelope,
  NodeToManagerMessage,
  PROTOCOL_VERSION,
  RegistrationAck,
};
use tokio::io::{
  AsyncReadExt,
  AsyncWriteExt,
};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use super::node::Node;

#[derive(Debug)]
pub struct NodeManager {
  nodes:    Arc<RwLock<HashMap<u64, Node>>>,
  listener: TcpListener,
  address:  SocketAddr,
  // Track spawned tasks to prevent leaks
  tasks:    Arc<RwLock<HashMap<u64, JoinHandle<()>>>>,
}

impl NodeManager {
  pub async fn start_node_manager(config: &Config) -> Result<Self, std::io::Error> {
    let address = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&address).await?;
    let addr = listener.local_addr()?;

    log::info!("Node Manager listening on: {}", addr);

    Ok(NodeManager {
      nodes: Arc::new(RwLock::new(HashMap::new())),
      listener,
      address: addr,
      tasks: Arc::new(RwLock::new(HashMap::new())),
    })
  }

  pub async fn listen(&self) -> Result<(), std::io::Error> {
    log::info!("Starting listener on {}", self.address);
    loop {
      let (mut socket, addr) = self.listener.accept().await?;
      let nodes = Arc::clone(&self.nodes);
      let tasks = Arc::clone(&self.tasks);

      tokio::spawn(async move {
        log::debug!("Got a new connection from: {}", addr);

        // Read initial registration message
        let mut buf = vec![0u8; 4096];
        let n = match socket.read(&mut buf).await {
          Ok(0) => {
            log::warn!("Connection closed by {} before sending registration", addr);
            return;
          }
          Ok(n) => n,
          Err(e) => {
            log::error!("Failed to read registration from {}: {}", addr, e);
            return;
          }
        };

        let msg = String::from_utf8_lossy(&buf[..n]);
        log::debug!("Received registration message: {:#?}", msg);

        // Parse registration envelope
        let registration: MessageEnvelope<NodeToManagerMessage> = match serde_json::from_str(&msg) {
          Ok(reg) => reg,
          Err(e) => {
            log::error!("Failed to parse registration from {}: {}", addr, e);
            return;
          }
        };

        // Validate protocol version
        if registration.version != PROTOCOL_VERSION {
          log::warn!(
            "Version mismatch from {}: got {}, expected {}",
            addr,
            registration.version,
            PROTOCOL_VERSION
          );
        }

        // Extract node from registration
        let node: Node = match registration.message {
          NodeToManagerMessage::NodeRegistration(node_reg) => Node::from(node_reg),
          _ => {
            log::error!(
              "Received unexpected message type during registration: {:#?}",
              registration.message
            );
            return;
          }
        };

        let node_id = node.id;
        log::info!("Node {} ({}) registered from {}", node_id, node.name, addr);

        // Send registration acknowledgment
        let ack = ManagerToNodeMessage::RegistrationAck(RegistrationAck {
          node_id,
          accepted: true,
          message: Some(format!("Node {} successfully registered", node.name)),
        });
        log::debug!("Sending registration ACK to node {}: {:#?}", node_id, ack);

        let ack_envelope = MessageEnvelope::new(PROTOCOL_VERSION, ack);

        if let Ok(ack_json) = serde_json::to_string(&ack_envelope) {
          if let Err(e) = socket.write_all(ack_json.as_bytes()).await {
            log::error!("Failed to send registration ACK to node {}: {}", node_id, e);
          } else {
            log::debug!("Sent registration ACK to node {}", node_id);
          }
        }

        nodes.write().await.insert(node.id, node);

        // Spawn task to handle node messages and track it
        let nodes_clone = Arc::clone(&nodes);
        let tasks_clone = Arc::clone(&tasks);
        let handle = tokio::spawn(async move {
          log::debug!("Started message handler for node: {} (task: {:?})", node_id, tokio::task::id());

          let mut buf = vec![0u8; 4096];
          loop {
            // Read message from node
            let n = match socket.read(&mut buf).await {
              Ok(0) => {
                log::info!("Node {} disconnected (connection closed)", node_id);
                break;
              }
              Ok(n) => n,
              Err(e) => {
                log::error!("Error reading from node {}: {}", node_id, e);
                break;
              }
            };

            // Parse message
            let message: MessageEnvelope<NodeToManagerMessage> = match serde_json::from_slice(&buf[..n]) {
              Ok(msg) => msg,
              Err(e) => {
                log::error!("Failed to parse message from node {}: {}", node_id, e);
                continue;
              }
            };

            log::debug!("Received message from node {}: {:?}", node_id, message.message);

            // Handle message
            match message.message {
              NodeToManagerMessage::NodeDrop => {
                log::info!("Node {} requested disconnect", node_id);
                break;
              }
              _ => {
                if let Some(node) = nodes_clone.write().await.get_mut(&node_id) {
                  node.handle_message(message.message, &mut socket).await;
                } else {
                  log::warn!("Received message for unknown node: {}", node_id);
                  break;
                }
              }
            }
          }

          // Clean up: remove node from registry
          if nodes_clone.write().await.remove(&node_id).is_some() {
            log::info!("Removed node {} from registry", node_id);
          }

          // Remove task handle
          tasks_clone.write().await.remove(&node_id);
        });

        // Store task handle
        tasks.write().await.insert(node_id, handle);
      });
    }
  }

  pub async fn get_nodes(&self) -> Arc<RwLock<HashMap<u64, Node>>> { Arc::clone(&self.nodes) }

  pub async fn active_connections(&self) -> usize { self.tasks.read().await.len() }
}
