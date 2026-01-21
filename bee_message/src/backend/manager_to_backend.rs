use serde::{
  Deserialize,
  Serialize,
};
use crate::{
  NodeEvent,
  NodeStatusUpdate,
  PotStatusUpdate,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ManagerToBackendMessage {
  ManagerRegistrationAck(BackendRegistrationAck),
  CommandResponse,
  BackendResponse(BackendResponse),
  NodeEvent {
    event:   NodeEvent,
    node_id: u64,
  },
  NodeStatusUpdate(NodeStatusUpdate),
  PotStatusUpdate(PotStatusUpdate),
  CoreLog(CoreLogMessage),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoreLogMessage {
  pub level: String,
  pub target: String,
  pub message: String,
  pub timestamp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendRegistrationAck {
  pub backend_id: u64,
  pub accepted:   bool,
  pub message:    Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackendResponse {
  Success {
    message: Option<String>,
    data:    Option<serde_json::Value>,
  },
  Failure(String),
}
