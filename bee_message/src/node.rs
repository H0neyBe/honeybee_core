use serde::{
  Deserialize,
  Serialize,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageEnvelope {
  pub version: u64,
  pub message: MessageType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageType {
  NodeRegistration(NodeRegistration),
  NodeDrop,
  NodeStatusUpdate(NodeStatusUpdate),
  Event(NodeEvent),
  NodeCommand(NodeCommand),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRegistration {
  pub node_id:   u64,
  pub node_name: String,
  pub address:   String,
  pub port:      u16,
  pub node_type: NodeType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeStatusUpdate {
  pub node_id: u64,
  pub status:  NodeStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeEvent {
  Started,
  Stopped,
  Alarm,
  Error(String),
}


#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeCommand {
  pub node_id: u64,
  pub command: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeType {
  Full,
  Agent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeStatus {
  Deploying,
  Running,
  Stopped,
  Failed,
  Unknown,
}
