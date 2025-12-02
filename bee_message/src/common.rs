use serde::{Deserialize, Serialize};

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

/// Honeypot lifecycle status
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HoneypotStatus {
  Installing,
  Running,
  Stopped,
  Failed,
}

impl std::fmt::Display for HoneypotStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      HoneypotStatus::Installing => write!(f, "Installing"),
      HoneypotStatus::Running => write!(f, "Running"),
      HoneypotStatus::Stopped => write!(f, "Stopped"),
      HoneypotStatus::Failed => write!(f, "Failed"),
    }
  }
}

/// Message envelope wrapping all protocol messages
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageEnvelope<T> {
  pub version: u64,
  pub message: T,
}

impl<T> MessageEnvelope<T> {
  pub fn new(version: u64, message: T) -> Self {
    Self { version, message }
  }
}

/// Protocol version constant
pub const PROTOCOL_VERSION: u64 = 2;