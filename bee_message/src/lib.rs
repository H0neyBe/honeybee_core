pub mod common;
pub mod node_to_manager;
pub mod manager_to_node;
pub mod core;

// Re-export commonly used types
pub use common::{MessageEnvelope, NodeStatus, NodeType, PROTOCOL_VERSION};
pub use node_to_manager::{NodeEvent, NodeRegistration, NodeStatusUpdate, NodeToManagerMessage};
pub use manager_to_node::{ManagerToNodeMessage, NodeCommand, RegistrationAck};
pub use core::BidirectionalMessage;
