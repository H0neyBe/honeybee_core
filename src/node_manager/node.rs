use std::error::Error;
use std::fmt;
use std::time::{
  SystemTime,
  UNIX_EPOCH,
};

use bee_message::{
  NodeRegistration,
  NodeStatus,
  NodeStatusUpdate,
  NodeToManagerMessage,
  NodeType,
};
use serde::{
  Deserialize,
  Serialize,
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

  pub async fn handle_message(&mut self, message: NodeToManagerMessage, _socket: &mut TcpStream) {
    log::debug!("Handling message for node: {}", self.id);
    match message {
      NodeToManagerMessage::NodeStatusUpdate(status) => {
        log::debug!("Received status update for node {}: {:?}", self.id, status.status);
        self.status = status.status;
      }
      NodeToManagerMessage::NodeEvent(event) => {
        log::debug!("Received event from node {}: {:?}", self.id, event);
      }
      NodeToManagerMessage::NodeDrop => {
        log::info!("Node {} requested disconnect", self.id);
        self.status = NodeStatus::Stopped;
      }
      _ => {
        log::warn!("Unhandled message type for node {}: {:?}", self.id, message);
      }
    }
  }
}

impl From<NodeRegistration> for Node {
  fn from(reg: NodeRegistration) -> Self {
    let config = NodeConfig {
      address:   reg.address,
      port:      reg.port,
      node_type: reg.node_type,
    };

    Node {
      id: reg.node_id,
      name: reg.node_name,
      config,
      status: NodeStatus::Deploying,
      created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
    }
  }
}
