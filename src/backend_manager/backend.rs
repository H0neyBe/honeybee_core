use std::fmt;
use std::sync::Arc;
use std::time::{
  SystemTime,
  UNIX_EPOCH,
};

use bee_message::{
  BackendResponse,
  BackendToManagerMessage,
  BackendType,
  ManagerToBackendMessage,
  MessageEnvelope,
  PROTOCOL_VERSION,
};
use serde::{
  Deserialize,
  Serialize,
};
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackendError {
  ConnectionFailed(String),
  CommandFailed(String),
  InvalidConfiguration(String),
  NotFound(String),
  SendFailed(String),
}

impl fmt::Display for BackendError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      BackendError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
      BackendError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
      BackendError::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {}", msg),
      BackendError::NotFound(msg) => write!(f, "Backend not found: {}", msg),
      BackendError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
    }
  }
}

impl std::error::Error for BackendError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Backend {
  pub id:           u64,
  pub backend_type: BackendType,
  pub created_at:   u64,
  #[serde(skip)]
  pub writer:       Option<Arc<Mutex<OwnedWriteHalf>>>,
}

impl Backend {
  pub fn new_with_writer(id: u64, backend_type: BackendType, writer: Arc<Mutex<OwnedWriteHalf>>) -> Self {
    Backend {
      id,
      backend_type,
      writer: Some(writer),
      created_at: Self::current_timestamp(),
    }
  }

  fn current_timestamp() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs()
  }

  pub fn is_connected(&self) -> bool { self.writer.is_some() }

  pub async fn disconnect(&mut self) {
    if let Some(writer) = self.writer.take() {
      log::debug!("Disconnecting backend {}", self.id);
      if let Ok(writer) = Arc::try_unwrap(writer) {
        let mut locked_writer = writer.into_inner();
        let _ = locked_writer.shutdown().await;
      }
    }
  }

  /// Send a response back to the backend
  pub async fn send_response(&mut self, response: BackendResponse) -> Result<(), String> {
    let writer = self
      .writer
      .as_ref()
      .ok_or_else(|| "No writer available".to_string())?;
    let mut locked_writer = writer.lock().await;

    let message = ManagerToBackendMessage::BackendResponse(response);
    let envelope = MessageEnvelope::new(PROTOCOL_VERSION, message);
    let json = serde_json::to_string(&envelope).map_err(|e| e.to_string())? + "\n";

    locked_writer
      .write_all(json.as_bytes())
      .await
      .map_err(|e| e.to_string())?;
    locked_writer
      .flush()
      .await
      .map_err(|e| e.to_string())?;

    log::debug!("Sent response to backend {}", self.id);
    Ok(())
  }

  /// Send an error response back to the backend
  pub async fn send_error(&mut self, error_msg: String) -> Result<(), String> {
    let response = BackendResponse::Failure(error_msg);
    self.send_response(response).await
  }

  /// Send a success response back to the backend
  pub async fn send_success(&mut self, message: Option<String>, data: Option<serde_json::Value>) -> Result<(), String> {
    let response = BackendResponse::Success { message, data };
    self.send_response(response).await
  }
}
