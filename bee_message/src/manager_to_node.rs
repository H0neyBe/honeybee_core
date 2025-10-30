use serde::{Deserialize, Serialize};

/// Messages sent from Manager → Node

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ManagerToNodeMessage {
  NodeCommand(NodeCommand),
  RegistrationAck(RegistrationAck),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeCommand {
  pub node_id: u64,
  pub command: String,
}

impl NodeCommand {
  /// Standard command: request status update
  pub fn status(node_id: u64) -> Self {
    Self {
      node_id,
      command: "status".to_string(),
    }
  }

  /// Standard command: stop node operations
  pub fn stop(node_id: u64) -> Self {
    Self {
      node_id,
      command: "stop".to_string(),
    }
  }

  /// Standard command: restart node
  pub fn restart(node_id: u64) -> Self {
    Self {
      node_id,
      command: "restart".to_string(),
    }
  }

  /// Custom command
  pub fn custom(node_id: u64, command: String) -> Self {
    Self { node_id, command }
  }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistrationAck {
  pub node_id: u64,
  pub accepted: bool,
  pub message: Option<String>,
}