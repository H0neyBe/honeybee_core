use std::time::{SystemTime, UNIX_EPOCH};

use bee_message::node::{NodeRegistration, NodeStatus, NodeType};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
  pub id: u64,
  pub name: String,
  pub config: NodeConfig,
  pub status: NodeStatus,
  pub created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeConfig {
  pub address: String,
  pub port: u16,
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

  fn current_timestamp() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs()
  }
}

impl From<NodeRegistration> for Node {
  fn from(reg: NodeRegistration) -> Self {
    let config = NodeConfig {
      address: reg.address,
      port: reg.port,
      node_type: reg.node_type,
    };

    Node {
      id: reg.node_id,
      name: reg.node_name,
      config,
      status: NodeStatus::Deploying,
      created_at: SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs(),
    }
  }
}
