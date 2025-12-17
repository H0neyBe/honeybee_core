pub mod backend;
pub mod common;
pub mod core;
pub mod node;

// Re-export commonly used types
pub use core::BidirectionalMessage;

pub type PotId = String;

pub use backend::{
  backend_to_manager,
  manager_to_backend,
  lib::BackendType,
};
pub use backend_to_manager::{
  BackendCommand,
  BackendToManagerMessage,
  BackendRegistration,
};

pub use manager_to_backend::{
  ManagerToBackendMessage,
  BackendRegistrationAck,
  BackendResponse,
};


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
  NodeCommandType,
  InstallPot,

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
