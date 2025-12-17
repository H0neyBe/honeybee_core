use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::{
  SystemTime,
  UNIX_EPOCH,
};

use bee_message::{
  ManagerToNodeMessage,
  MessageEnvelope,
  NodeRegistration,
  NodeStatus,
  NodeToManagerMessage,
  NodeType,
  PROTOCOL_VERSION,
  RegistrationAck,
};
use serde::{
  Deserialize,
  Serialize,
};
use tokio::io::{
  AsyncReadExt,
  AsyncWriteExt,
};
use tokio::net::TcpStream;
use tokio::sync::{
  Mutex,
  RwLock,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HoneypotError {
  DeploymentFailed(String),
  NodeNotFound(String),
  CommandFailed(String),
  DataCollectionFailed(String),
  InvalidConfiguration(String),
}

impl fmt::Display for HoneypotError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      HoneypotError::DeploymentFailed(msg) => write!(f, "Deployment failed: {}", msg),
      HoneypotError::NodeNotFound(msg) => write!(f, "Node not found: {}", msg),
      HoneypotError::CommandFailed(msg) => write!(f, "Command failed: {}", msg),
      HoneypotError::DataCollectionFailed(msg) => write!(f, "Data collection failed: {}", msg),
      HoneypotError::InvalidConfiguration(msg) => write!(f, "Invalid configuration: {}", msg),
    }
  }
}

impl Error for HoneypotError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
  pub id:         u64,
  pub name:       String,
  #[serde(skip)]
  pub stream:     Option<Arc<Mutex<TcpStream>>>,
  pub config:     NodeConfig,
  pub status:     NodeStatus,
  pub created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeConfig {
  pub address:   String,
  pub port:      u16,
  pub node_type: NodeType,
}

impl Node {
  pub fn new(id: u64, name: String, config: NodeConfig, stream: TcpStream) -> Self {
    Node {
      id,
      name,
      config,
      status: NodeStatus::Unknown,
      created_at: Self::current_timestamp(),
      stream: Some(Arc::new(Mutex::new(stream))),
    }
  }

  fn current_timestamp() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs()
  }

  pub fn update_status(&mut self, status: NodeStatus) { self.status = status; }

  pub fn set_stream(&mut self, stream: TcpStream) { self.stream = Some(Arc::new(Mutex::new(stream))); }

  pub fn take_stream(&mut self) -> Option<TcpStream> {
    self.stream.take().and_then(|arc_lock| {
      Arc::try_unwrap(arc_lock)
        .ok()
        .map(|rw_lock| rw_lock.into_inner())
    })
  }

  /// Send registration acknowledgment to the node
  pub async fn send_registration_ack(&mut self) -> Result<(), Box<dyn Error>> {
    let stream = self
      .stream
      .as_mut()
      .ok_or("No stream available")?;

    let ack = ManagerToNodeMessage::RegistrationAck(RegistrationAck {
      node_id:  self.id,
      accepted: true,
      message:  Some(format!("Node {} successfully registered", self.name)),
    });

    log::debug!("Sending registration ACK to node {}: {:#?}", self.id, ack);

    self
      .send(ManagerToNodeMessage::RegistrationAck(RegistrationAck {
        node_id:  self.id,
        accepted: true,
        message:  Some(format!("Node {} successfully registered", self.name)),
      }))
      .await?;
    log::debug!("Sent registration ACK to node {}", self.id);
    Ok(())
  }

  /// Read a message from the node's stream
  pub async fn read_message(&mut self) -> Result<MessageEnvelope<NodeToManagerMessage>, Box<dyn Error>> {
    let stream = self
      .stream
      .as_mut()
      .ok_or("No stream available")?;

    let mut buf = vec![0u8; 4096];
    let n = stream.lock().await.read(&mut buf).await?;

    if n == 0 {
      return Err("Connection closed".into());
    }

    let message: MessageEnvelope<NodeToManagerMessage> = serde_json::from_slice(&buf[..n])?;
    Ok(message)
  }

  /// Handle incoming messages from the node
  pub async fn handle_message(&mut self, message: NodeToManagerMessage) {
    log::debug!("Handling message for node: {}", self.id);
    match message {
      NodeToManagerMessage::NodeStatusUpdate(status) => {
        log::debug!("Received status update for node {}: {:?}", self.id, status.status);
        self.status = status.status;
      }
      NodeToManagerMessage::NodeEvent(event) => {
        log::debug!("Received event from node {}: {:?}", self.id, event);
      }
      NodeToManagerMessage::NodeDrop => {
        log::info!("Node {} requested disconnect", self.id);
        self.status = NodeStatus::Stopped;
      }
      _ => {
        log::warn!("Unhandled message type for node {}: {:?}", self.id, message);
      }
    }
  }

  /// Send a command to the node
  pub async fn send(&mut self, command: ManagerToNodeMessage) -> Result<(), Box<dyn Error>> {
    let stream = self
      .stream
      .as_ref()
      .ok_or("No stream available")?
      .clone();
    let node_id = self.id;

    let envelope = MessageEnvelope::new(PROTOCOL_VERSION, command);
    let json = serde_json::to_string(&envelope)? + "\n";

    tokio::spawn(async move {
      let mut locked_stream = stream.lock().await;
      if let Err(e) = locked_stream.write_all(json.as_bytes()).await {
        log::error!("Failed to send command to node {}: {}", node_id, e);
      }
      if let Err(e) = locked_stream.flush().await {
        log::error!("Failed to flush stream for node {}: {}", node_id, e);
      }
    });

    Ok(())
  }

  /// Check if the node's connection is still alive
  pub fn is_connected(&self) -> bool { self.stream.is_some() }

  /// Gracefully disconnect the node
  pub async fn disconnect(&mut self) {
    if let Some(stream) = self.stream.take() {
      log::debug!("Disconnecting node {}", self.id);
      let mut locked_stream = stream.lock().await;
      let _ = locked_stream.shutdown().await;
    }
    self.status = NodeStatus::Stopped;
  }
}

impl From<NodeRegistration> for Node {
  fn from(reg: NodeRegistration) -> Self {
    let config = NodeConfig {
      address:   reg.address,
      port:      reg.port,
      node_type: reg.node_type,
    };

    Node {
      id: reg.node_id,
      name: reg.node_name,
      config,
      status: NodeStatus::Deploying,
      created_at: SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs(),
      stream: None,
    }
  }
}
