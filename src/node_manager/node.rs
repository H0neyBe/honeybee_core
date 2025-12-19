use std::sync::Arc;

use bee_message::{
  ManagerToNodeMessage,
  MessageEnvelope,
  NodeRegistration,
  NodeStatus,
  NodeToManagerMessage,
  PROTOCOL_VERSION,
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
use tokio::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
  pub id:         u64,
  pub name:       String,
  pub status:     NodeStatus,
  #[serde(skip)]
  pub tcp_writer: Option<Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>>,
  #[serde(skip)]
  pub tcp_reader: Option<Arc<Mutex<tokio::net::tcp::OwnedReadHalf>>>,
}

impl Node {
  pub fn new(id: u64, name: String) -> Self {
    Node {
      id,
      name,
      status: NodeStatus::Connected,
      tcp_writer: None,
      tcp_reader: None,
    }
  }

  pub fn set_stream(&mut self, stream: TcpStream) {
    let (read_half, write_half) = stream.into_split();
    self.tcp_writer = Some(Arc::new(Mutex::new(write_half)));
    self.tcp_reader = Some(Arc::new(Mutex::new(read_half)));
  }

  pub async fn send_registration_ack(&mut self) -> Result<(), String> {
    let ack = ManagerToNodeMessage::RegistrationAck(bee_message::RegistrationAck {
      node_id:  self.id,
      accepted: true,
      message:  Some(String::from("Registration successful")),
    });
    log::debug!("Sending registration ack to node {}: {:?}", self.id, ack);
    self.send_message(ack).await
  }

  pub async fn send_message(&mut self, message: ManagerToNodeMessage) -> Result<(), String> {
    let writer = self
      .tcp_writer
      .as_ref()
      .ok_or("No writer available")?;
    let mut locked_writer = writer.lock().await;

    let envelope = MessageEnvelope::new(PROTOCOL_VERSION, message);
    let json = serde_json::to_string(&envelope).map_err(|e| e.to_string())? + "\n";
    let data = json.as_bytes();

    locked_writer
      .write_all(data)
      .await
      .map_err(|e| e.to_string())?;
    locked_writer
      .flush()
      .await
      .map_err(|e| e.to_string())?;

    Ok(())
  }

  pub async fn read_message(&self) -> Result<MessageEnvelope<NodeToManagerMessage>, String> {
    let reader = self
      .tcp_reader
      .as_ref()
      .ok_or("No reader available")?;
    let mut locked_reader = reader.lock().await;

    let mut buf = vec![0u8; 4096];
    let n = locked_reader
      .read(&mut buf)
      .await
      .map_err(|e| e.to_string())?;

    if n == 0 {
      return Err("Connection closed".to_string());
    }

    let msg = String::from_utf8_lossy(&buf[..n]);
    serde_json::from_str(&msg).map_err(|e| e.to_string())
  }

  pub async fn disconnect(&mut self) {
    if let Some(writer) = self.tcp_writer.take() {
      log::debug!("Disconnecting node {}", self.id);
      if let Ok(writer) = Arc::try_unwrap(writer) {
        let mut locked_writer = writer.into_inner();
        let _ = locked_writer.shutdown().await;
      }
    }
    self.status = NodeStatus::Stopped;
  }
}

impl From<NodeRegistration> for Node {
  fn from(reg: NodeRegistration) -> Self {
    Node {
      id:         reg.node_id,
      name:       reg.node_name,
      status:     NodeStatus::Connected,
      tcp_writer: None,
      tcp_reader: None,
    }
  }
}
