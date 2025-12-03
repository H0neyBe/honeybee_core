use serde::{
  Deserialize,
  Serialize,
};

use crate::manager_to_node::ManagerToNodeMessage;
use crate::node_to_manager::NodeToManagerMessage;

/// Core bidirectional message type supporting both directions
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum BidirectionalMessage {
  NodeToManager(NodeToManagerMessage),
  ManagerToNode(ManagerToNodeMessage),
}

impl From<NodeToManagerMessage> for BidirectionalMessage {
  fn from(msg: NodeToManagerMessage) -> Self { BidirectionalMessage::NodeToManager(msg) }
}

impl From<ManagerToNodeMessage> for BidirectionalMessage {
  fn from(msg: ManagerToNodeMessage) -> Self { BidirectionalMessage::ManagerToNode(msg) }
}
