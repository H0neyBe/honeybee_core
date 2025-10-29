use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;

use bee_config::config::Config;
use bee_message::node::{
  MessageEnvelope,
  MessageType,
  NodeRegistration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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
  pub async fn start_node_manager(config: &Config) -> Result<NodeManager, std::io::Error> {
    let listener = TcpListener::bind(config.get_address()).await?;
    let address = config.get_address();
    let node_manager = NodeManager {
      nodes: Arc::new(RwLock::new(HashMap::new())),
      listener,
      address,
      tasks: Arc::new(RwLock::new(HashMap::new())),
    };

    Ok(node_manager)
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
        log::debug!("Received registration message: {}", msg);
        
        // Parse registration
        let registration: MessageEnvelope = match serde_json::from_str(&msg) {
          Ok(reg) => reg,
          Err(e) => {
            log::error!("Failed to parse registration from {}: {}", addr, e);
            return;
          }
        };

        // Extract node from registration
        let node: Node = match registration.message {
          MessageType::NodeRegistration(node_reg) => Node::from(node_reg),
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
            let message: MessageEnvelope = match serde_json::from_slice(&buf[..n]) {
              Ok(msg) => msg,
              Err(e) => {
                log::error!("Failed to parse message from node {}: {}", node_id, e);
                continue;
              }
            };
            
            log::debug!("Received message from node {}: {:?}", node_id, message.message);
            
            // Handle message
            match message.message {
              MessageType::NodeDrop => {
                log::info!("Node {} requested disconnect", node_id);
                break;
              }
              _ => {
                if let Some(node) = nodes_clone.write().await.get_mut(&node_id) {
                  node.handle_message(message, &mut socket).await;
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
          
          // Remove task from tracking
          tasks_clone.write().await.remove(&node_id);
          
          // Ensure socket is properly closed
          if let Err(e) = socket.shutdown().await {
            log::debug!("Error shutting down socket for node {}: {}", node_id, e);
          }
          
          log::info!("Connection handler for node {} terminated", node_id);
        });
        
        // Track the spawned task
        tasks.write().await.insert(node_id, handle);
      });
    }
  }

  pub async fn shutdown(&self) {
    log::info!("Shutting down NodeManager...");
    
    // Abort all active tasks
    let mut tasks = self.tasks.write().await;
    for (node_id, handle) in tasks.drain() {
      log::debug!("Aborting task for node {}", node_id);
      handle.abort();
    }
    
    // Clear all nodes
    self.nodes.write().await.clear();
    
    log::info!("NodeManager shutdown complete");
  }

  pub async fn get_address(&self) -> SocketAddr { self.address }

  pub async fn get_nodes(&self) -> Arc<RwLock<HashMap<u64, Node>>> { Arc::clone(&self.nodes) }

  pub async fn get_node(&self, id: u64) -> Option<Node> {
    let nodes = self.get_nodes().await;
    let read_lock = nodes.read().await;
    read_lock.get(&id).cloned()
  }

  pub async fn is_running(&self) -> bool {
    self.listener.local_addr().is_ok()
  }
  
  pub async fn active_connections(&self) -> usize {
    self.tasks.read().await.len()
  }
}

impl Drop for NodeManager {
  fn drop(&mut self) {
    log::debug!("NodeManager dropped");
  }
}