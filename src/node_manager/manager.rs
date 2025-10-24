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
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use super::node::Node;

#[derive(Debug)]
pub struct NodeManager {
  nodes:    Arc<RwLock<HashMap<u64, Node>>>,
  listener: TcpListener,
  address:  SocketAddr,
}

impl NodeManager {
  pub async fn start_node_manager(config: &Config) -> Result<NodeManager, std::io::Error> {
    let listener = TcpListener::bind(config.get_address()).await?;
    let address = config.get_address();
    let node_manager = NodeManager {
      nodes: Arc::new(RwLock::new(HashMap::new())),
      listener,
      address,
    };

    Ok(node_manager)
  }

  pub async fn listen(&self) -> Result<(), std::io::Error> {
    log::debug!("Starting listener");
    loop {
      let (mut socket, addr) = self.listener.accept().await?;
      let nodes = Arc::clone(&self.nodes);
      tokio::spawn(async move {
        // Handle the incoming node connection
        // You can parse node registration, add to nodes HashMap, etc.
        log::debug!("Got a new connection from: {}", addr);
        let mut buf = [0; 1024];
        let n = socket.read(&mut buf).await.unwrap();
        let msg = String::from_utf8_lossy(&buf[..n]);

        log::debug!("decoded a message: {:#?}", msg);
        let registration: MessageEnvelope = serde_json::from_str(&msg).unwrap();

        let node: Node = match registration.message {
          bee_message::node::MessageType::NodeRegistration(node_reg) => Node::from(node_reg),
          _ => {
            log::error!(
              "Received unexpected message type during registration: {:#?}",
              registration.message
            );
            return;
          } // Handle other message types or return early
        };
        log::debug!("New node registered: {}", node.id);
        let node_id = node.id;
        nodes.write().await.insert(node.id, node);
        // Start a new task for the node

        log::debug!(
          "Listening for messages from node: {} from task: {:?}",
          node_id,
          tokio::task::id()
        );
        let nodes_clone = Arc::clone(&nodes);
        tokio::spawn(async move {
          // Task logic here
          log::debug!("Spawned task for node: {} Id: {:?}", node_id, tokio::task::id());
          let mut buf = [0; 1024];
          loop {
            // Wait for messages from the node
            let message = socket.read(&mut buf).await.unwrap();
            let message: Result<MessageEnvelope, serde_json::Error> = serde_json::from_slice(&buf[..message]);
            match message {
              Ok(msg) => {
                log::debug!("Received message from node: {}: {:#?}", node_id, msg);
                // Handle the message
                match msg.message {
                  MessageType::NodeDrop => {
                    log::debug!("closing connection with Node: ");
                    break;
                  }
                  _ => {
                    if let Some(node) = nodes_clone.write().await.get_mut(&node_id) {
                      node.handle_message(msg, &mut socket).await;
                    }
                  }
                }
              }
              Err(e) => {
                log::error!("Error receiving message from node: {}: {}", node_id, e);
                break;
              }
            }
          }
          log::debug!("Closing connection: {}", node_id);
        });
      });
    }
  }

  pub async fn get_address(&self) -> SocketAddr { self.address }

  pub async fn get_nodes(&self) -> Arc<RwLock<HashMap<u64, Node>>> { Arc::clone(&self.nodes) }

  pub async fn get_node(&self, id: u64) -> Option<Node> {
    let nodes = self.get_nodes().await;
    let read_lock = nodes.read().await;
    read_lock.get(&id).cloned()
  }

  pub async fn is_running(&self) -> bool {
    // Check if the NodeManager is running
    self.listener.local_addr().is_ok()
  }
}
