use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::{
  SystemTime,
  UNIX_EPOCH,
};

use bee_message::{
  BackendCommand,
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
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
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

impl Error for BackendError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Backend {
  pub id: u64,
  pub backend_type: BackendType,
  #[serde(skip)]
  pub stream: Option<Arc<Mutex<TcpStream>>>,
  pub created_at: u64,
}

impl Backend {
  pub fn new(id: u64, backend_type: BackendType, stream: TcpStream) -> Self {
    Backend {
      id,
      backend_type,
      stream: Some(Arc::new(Mutex::new(stream))),
      created_at: Self::current_timestamp(),
    }
  }

  fn current_timestamp() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs()
  }

  pub fn set_stream(&mut self, stream: TcpStream) {
    self.stream = Some(Arc::new(Mutex::new(stream)));
  }

  pub fn take_stream(&mut self) -> Option<TcpStream> {
    self.stream.take().and_then(|arc_lock| {
      Arc::try_unwrap(arc_lock)
        .ok()
        .map(|mutex| mutex.into_inner())
    })
  }

  /// Read a command from the backend's stream
  pub async fn read_command(&mut self) -> Result<MessageEnvelope<BackendToManagerMessage>, Box<dyn Error>> {
    let stream = self.stream.as_ref().ok_or("No stream available")?;
    let mut locked_stream = stream.lock().await;

    let mut buf = vec![0u8; 8192];
    let n = locked_stream.read(&mut buf).await?;

    if n == 0 {
      return Err("Connection closed".into());
    }

    let message: MessageEnvelope<BackendToManagerMessage> = serde_json::from_slice(&buf[..n])?;
    Ok(message)
  }

  /// Send a response back to the backend
  pub async fn send_response(&mut self, response: BackendResponse) -> Result<(), Box<dyn Error>> {
    let stream = self.stream.as_ref().ok_or("No stream available")?;
    let mut locked_stream = stream.lock().await;

    let message = ManagerToBackendMessage::BackendResponse(response);
    let envelope = MessageEnvelope::new(PROTOCOL_VERSION, message);
    let json = serde_json::to_string(&envelope)? + "\n";

    locked_stream.write_all(json.as_bytes()).await?;
    locked_stream.flush().await?;

    log::debug!("Sent response to backend {}", self.id);
    Ok(())
  }

  /// Send an error response back to the backend
  pub async fn send_error(&mut self, error_msg: String) -> Result<(), Box<dyn Error>> {
    let response = BackendResponse::Failure(error_msg);
    self.send_response(response).await
  }

  /// Send a success response back to the backend
  pub async fn send_success(&mut self, message: Option<String>, data: Option<serde_json::Value>) -> Result<(), Box<dyn Error>> {
    let response = BackendResponse::Success {
      message,
      data,
    };
    self.send_response(response).await
  }

  /// Check if the backend's connection is still alive
  pub fn is_connected(&self) -> bool {
    self.stream.is_some()
  }

  /// Gracefully disconnect the backend
  pub async fn disconnect(&mut self) {
    if let Some(stream) = self.stream.take() {
      log::debug!("Disconnecting backend {}", self.id);
      if let Ok(stream) = Arc::try_unwrap(stream) {
        let mut locked_stream = stream.into_inner();
        let _ = locked_stream.shutdown().await;
      }
    }
  }
}

impl From<BackendCommand> for Backend {
  fn from(cmd: BackendCommand) -> Self {
    Backend {
      id: rand::random::<u64>(),
      backend_type: BackendType::Web,
      stream: None,
      created_at: SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs(),
    }
  }
}