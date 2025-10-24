use bee_config::config::Config;

use super::node::Node;
use bee_message::node::NodeRegistration;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct NodeManager {
  nodes: Arc<RwLock<HashMap<u64, Node>>>,
  listener: TcpListener,
  address: SocketAddr,
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
        log::debug!("Got a enw connection from: {}", addr);
        let mut buf = [0; 1024];
        let n = socket.read(&mut buf).await.unwrap();
        let msg = String::from_utf8_lossy(&buf[..n]);
        let registration: NodeRegistration = serde_json::from_str(&msg).unwrap();
        let node: Node = Node::from(registration);
        log::debug!("New node registered: {}", node.id);
        let node_id = node.id;
        nodes.write().await.insert(node.id, node);
        // Start a new task for the node
        let nodes_clone = Arc::clone(&nodes);
        log::debug!("Starting a new task for node: {}", node_id);
        tokio::spawn(async move {
          // Task logic here
          let read_lock = nodes_clone.read().await;
          if let Some(node) = read_lock.get(&node_id) {
            // Process messages for the specific node
          }
        });
      });
    }
  }

  pub async fn get_address(&self) -> SocketAddr {
    self.address
  }

  pub async fn get_nodes(&self) -> Arc<RwLock<HashMap<u64, Node>>> {
    Arc::clone(&self.nodes)
  }

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
