use bee_message::node::{
  MessageEnvelope,
  MessageType,
  NodeRegistration,
  NodeStatus,
  NodeType,
};
use serde::{
  Deserialize,
  Serialize,
};
use std::error::Error;
use std::fmt;
use std::time::{
  SystemTime,
  UNIX_EPOCH,
};
use tokio::net::TcpStream;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HoneypotError {
  DeploymentFailed(String),
  NodeNotFound(String),
  CommandFailed(String),
  DataCollectionFailed(String),
  InvalidConfiguration(String),
}

impl fmt::Display for HoneypotError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      HoneypotError::DeploymentFailed(msg) => write!(f, "Deployment failed: {}", msg),
      HoneypotError::NodeNotFound(msg) => write!(f, "Node not found: {}", msg),
      HoneypotError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
      HoneypotError::DataCollectionFailed(msg) => write!(f, "Data collection failed: {}", msg),
      HoneypotError::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {}", msg),
    }
  }
}

impl Error for HoneypotError {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
  pub id:         u64,
  pub name:       String,
  pub config:     NodeConfig,
  pub status:     NodeStatus,
  pub created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeConfig {
  pub address:   String,
  pub port:      u16,
  pub node_type: NodeType,
}

impl Node {
  pub fn new(id: u64, name: String, config: NodeConfig) -> Self {
    Node {
      id,
      name,
      config,
      status: NodeStatus::Unknown,
      created_at: Self::current_timestamp(),
    }
  }

  fn current_timestamp() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }

  pub fn update_status(&mut self, status: NodeStatus) { self.status = status; }

  pub async fn handle_message(&mut self, message: MessageEnvelope, socket: &mut TcpStream) {
    log::debug!("Handling message for node: {}", self.id);
    match message.message {
      MessageType::NodeStatusUpdate(status) => {
        log::debug!("Received status update for node: {}: {:#?}", self.id, status);
        // Handle the status update
        self.status = status.status
      }
      _ => {
        log::error!("Received unexpected message type for node: {}", self.id);
      }
    }
  }
}

impl From<NodeRegistration> for Node {
  fn from(registration: NodeRegistration) -> Self {
    Node {
      id:         registration.node_id,
      name:       registration.node_name,
      config:     NodeConfig {
        address:   registration.address,
        port:      registration.port,
        node_type: registration.node_type,
      },
      status:     NodeStatus::Deploying,
      created_at: Self::current_timestamp(),
    }
  }
}
