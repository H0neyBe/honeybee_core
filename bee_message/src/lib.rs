pub mod common;
pub mod core;
pub mod manager_to_node;
pub mod node_to_manager;

// Re-export commonly used types
pub use core::BidirectionalMessage;

pub use common::{
  MessageEnvelope,
  NodeStatus,
  NodeType,
  PROTOCOL_VERSION,
};
pub use manager_to_node::{
  ManagerToNodeMessage,
  NodeCommand,
  RegistrationAck,
};
pub use node_to_manager::{
  NodeEvent,
  NodeRegistration,
  NodeStatusUpdate,
  NodeToManagerMessage,
};
