use serde::{
  Deserialize,
  Serialize,
};

use crate::backend::lib::BackendType;
use crate::common::NodeStatus;
// use crate::PotId;
use crate::node::manager_to_node::NodeCommandType;

/// Messages sent from Backend → Manager
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackendToManagerMessage {
  BackendRegistration(BackendRegistration),
  BackendCommand(BackendCommand),
  BackendDrop,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendRegistration {
  pub backend_name: String,
  pub backend_type: BackendType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackendCommand {
  NodeCommand { node_id: u64, command: NodeCommandType },
  BroadcastCommand { command: NodeCommandType },
  GetNodes,
  GetNode { node_id: u64 },
  GetNodesByStatus { status: NodeStatus },
  GetNodeCount,
  RemoveNode { node_id: u64 },
  GetActiveConnections,
}
