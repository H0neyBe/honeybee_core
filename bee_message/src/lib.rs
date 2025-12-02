pub mod common;
pub mod node_to_manager;
pub mod manager_to_node;
pub mod core;
pub mod node;

// Re-export commonly used types
pub use common::{MessageEnvelope, NodeStatus, NodeType, HoneypotStatus, PROTOCOL_VERSION};
pub use node_to_manager::{
    NodeEvent, NodeRegistration, NodeStatusUpdate, NodeToManagerMessage,
    HoneypotStatusUpdate, HoneypotEvent,
};
pub use manager_to_node::{
    ManagerToNodeMessage, NodeCommand, RegistrationAck,
    InstallHoneypotCmd, StartHoneypotCmd, StopHoneypotCmd,
};
pub use core::BidirectionalMessage;
