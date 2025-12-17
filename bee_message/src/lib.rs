pub mod backend;
pub mod common;
pub mod core;
pub mod node;

// Re-export commonly used types
pub use core::BidirectionalMessage;

pub type PotId = String;

pub use backend::lib::BackendType;
pub use backend::{
  backend_to_manager,
  manager_to_backend,
};
pub use backend_to_manager::{
  BackendCommand,
  BackendRegistration,
  BackendToManagerMessage,
};
pub use common::{
  MessageEnvelope,
  NodeStatus,
  NodeType,
  PROTOCOL_VERSION,
};
pub use manager_to_backend::{
  BackendRegistrationAck,
  BackendResponse,
  ManagerToBackendMessage,
};
pub use manager_to_node::{
  InstallPot,
  ManagerToNodeMessage,
  NodeCommand,
  NodeCommandType,
  RegistrationAck,
};
pub use node::{
  manager_to_node,
  node_to_manager,
};
pub use node_to_manager::{
  NodeEvent,
  NodeRegistration,
  NodeStatusUpdate,
  NodeToManagerMessage,
};
