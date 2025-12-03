use serde::{
  Deserialize,
  Serialize,
};

/// Common types shared between node and manager messages
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

/// Message envelope wrapping all protocol messages
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageEnvelope<T> {
  pub version: u64,
  pub message: T,
}

impl<T> MessageEnvelope<T> {
  pub fn new(version: u64, message: T) -> Self { Self { version, message } }
}

/// Protocol version constant
pub const PROTOCOL_VERSION: u64 = 2;
