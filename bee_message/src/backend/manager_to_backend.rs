use serde::{
  Deserialize,
  Serialize,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ManagerToBackendMessage {
  ManagerRegistrationAck(BackendRegistrationAck),
  CommandResponse,
  BackendResponse(BackendResponse),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendRegistrationAck {
  pub backend_id: u64,
  pub accepted:   bool,
  pub message:    Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackendResponse {
  Success { message: Option<String>, data: Option<serde_json::Value> },
  Failure(String),
}


