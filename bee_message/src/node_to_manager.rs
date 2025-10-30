use serde::{Deserialize, Serialize};
use crate::common::{NodeStatus, NodeType};

/// Messages sent from Node → Manager

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeToManagerMessage {
  NodeRegistration(NodeRegistration),
  NodeStatusUpdate(NodeStatusUpdate),
  NodeEvent(NodeEvent),
  NodeDrop,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRegistration {
  pub node_id: u64,
  pub node_name: String,
  pub address: String,
  pub port: u16,
  pub node_type: NodeType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeStatusUpdate {
  pub node_id: u64,
  pub status: NodeStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeEvent {
  Started,
  Stopped,
  Alarm { description: String },
  Error { message: String },
}