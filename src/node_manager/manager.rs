use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use bee_config::Config;
use bee_message::{
  BackendToManagerMessage,
  BidirectionalMessage,
  ManagerToBackendMessage,
  ManagerToNodeMessage,
  MessageEnvelope,
  NodeToManagerMessage,
  PROTOCOL_VERSION,
};
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use super::node::Node;

#[derive(Debug)]
pub struct NodeManager {
  nodes:             Arc<RwLock<HashMap<u64, Node>>>,
  listener:          TcpListener,
  address:           SocketAddr,
  tasks:             Arc<RwLock<HashMap<u64, JoinHandle<()>>>>,
  reader:            tokio::sync::mpsc::UnboundedReceiver<BidirectionalMessage>,
  writer:            tokio::sync::mpsc::UnboundedSender<BidirectionalMessage>,
  response_channels: Arc<RwLock<HashMap<u64, oneshot::Sender<String>>>>,
}

impl NodeManager {
  pub async fn build(
    config: &Config, reader: tokio::sync::mpsc::UnboundedReceiver<BidirectionalMessage>,
    writer: tokio::sync::mpsc::UnboundedSender<BidirectionalMessage>,
  ) -> Result<Self, std::io::Error> {
    let address = format!("{}:{}", config.server.host, config.server.node_port);
    let listener = TcpListener::bind(&address).await?;
    let addr = listener.local_addr()?;

    log::info!("Node Manager listening on: {}", addr);

    Ok(NodeManager {
      nodes:             Arc::new(RwLock::new(HashMap::new())),
      listener,
      address:           addr,
      tasks:             Arc::new(RwLock::new(HashMap::new())),
      reader,
      writer,
      response_channels: Arc::new(RwLock::new(HashMap::new())),
    })
  }

  pub async fn listen(&self) -> Result<(), std::io::Error> {
    log::info!("Starting listener on {}", self.address);
    loop {
      let (mut socket, addr) = self.listener.accept().await?;
      let nodes = Arc::clone(&self.nodes);
      let tasks = Arc::clone(&self.tasks);
      let response_channels = Arc::clone(&self.response_channels);

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
        let mut node: Node = match registration.message {
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

        // Attach the TCP stream to the node
        node.set_stream(socket);

        // Send registration acknowledgment
        if let Err(e) = node.send_registration_ack().await {
          log::error!("Failed to send registration ACK to node {}: {}", node_id, e);
          return;
        }

        // Insert node into the registry
        nodes.write().await.insert(node_id, node);

        // Spawn task to handle node messages and track it
        let nodes_clone = Arc::clone(&nodes);
        let tasks_clone = Arc::clone(&tasks);
        let response_channels_clone = Arc::clone(&response_channels);
        let handle = tokio::spawn(async move {
          log::debug!("Started message handler for node: {} (task: {:?})", node_id, tokio::task::id());

          loop {
            // Read message from node without holding the lock
            let message_result = {
              // why the fuck did you do a write here instead of a read
              let nodes_lock = nodes_clone.read().await;
              if let Some(node) = nodes_lock.get(&node_id) {
                node.read_message().await
              } else {
                log::warn!("Node {} not found in registry", node_id);
                break;
              }
            };

            match message_result {
              Ok(envelope) => {
                log::debug!("Received message from node {}: {:?}", node_id, envelope);
                
                // Handle different message types
                match envelope.message {
                  NodeToManagerMessage::PotLog(pot_log) => {
                    log::debug!("Received honeypot log from node {}: {:?}", node_id, pot_log);
                    // Forward to MongoDB honeypot logger
                    tokio::spawn(async move {
                      crate::utils::honeypot_logger::log_honeypot_event(pot_log).await;
                    });
                  }
                  NodeToManagerMessage::PotStatusUpdate(status_update) => {
                    log::info!("Pot status update from node {}: {:?}", node_id, status_update);
                    // Check if there's a waiting response channel
                    if let Some(sender) = response_channels_clone.write().await.remove(&node_id) {
                      let response = format!("Pot '{}' status: {:?} - {}", 
                        status_update.pot_id, 
                        status_update.status,
                        status_update.message.as_deref().unwrap_or(""));
                      let _ = sender.send(response);
                    }
                  }
                  NodeToManagerMessage::NodeStatusUpdate(status_update) => {
                    log::info!("Node status update: {:?}", status_update);
                  }
                  NodeToManagerMessage::NodeEvent(event) => {
                    log::info!("Node event from {}: {:?}", node_id, event);
                    // Check if there's a waiting response channel and send the event
                    if let Some(sender) = response_channels_clone.write().await.remove(&node_id) {
                      let response = format!("{:?}", event);
                      let _ = sender.send(response);
                    }
                  }
                  _ => {
                    log::debug!("Unhandled message type from node {}", node_id);
                  }
                }
              }
              Err(e) => {
                log::error!("Error reading from node {}: {}", node_id, e);
                break;
              }
            }
          }

          // Cleanup when node disconnects
          log::info!("Node {} disconnected, cleaning up", node_id);
          nodes_clone.write().await.remove(&node_id);
          tasks_clone.write().await.remove(&node_id);
        });

        tasks.write().await.insert(node_id, handle);
      });
    }
  }

  // Public API methods that don't block
  pub async fn get_nodes(&self) -> Vec<u64> {
    log::debug!("Getting list of registered nodes");
    self.nodes.read().await.keys().copied().collect()
  }

  pub async fn get_node(&self, node_id: u64) -> Option<Node> { self.nodes.read().await.get(&node_id).cloned() }

  pub async fn node_count(&self) -> usize { self.nodes.read().await.len() }

  pub async fn active_connections(&self) -> usize { self.tasks.read().await.len() }

  pub async fn remove_node(&self, node_id: u64) -> Option<Node> {
    let node = self.nodes.write().await.remove(&node_id);

    if let Some(handle) = self.tasks.write().await.remove(&node_id) {
      handle.abort();
    }

    node
  }

  pub async fn send_command_to_node(
    &self, 
    node_id: u64, 
    command: ManagerToNodeMessage,
    response_channel: Option<oneshot::Sender<String>>
  ) -> Result<(), String> {
    // Store the response channel if provided
    if let Some(sender) = response_channel {
      self.response_channels.write().await.insert(node_id, sender);
    }
    
    let nodes = self.nodes.read().await;

    if let Some(node) = nodes.get(&node_id) {
      node.send_message(command).await
    } else {
      Err(format!("Node {} not found", node_id))
    }
  }

  pub async fn broadcast_command(&self, command: ManagerToNodeMessage) {
    let nodes = self.nodes.read().await;

    for (node_id, node) in nodes.iter() {
      if let Err(e) = node.send_message(command.clone()).await {
        log::error!("Failed to send command to node {}: {}", node_id, e);
      }
    }
  }

  pub async fn get_nodes_by_status(&self, status: bee_message::NodeStatus) -> Vec<u64> {
    self
      .nodes
      .read()
      .await
      .iter()
      .filter(|(_, node)| node.status == status)
      .map(|(id, _)| *id)
      .collect()
  }
}
