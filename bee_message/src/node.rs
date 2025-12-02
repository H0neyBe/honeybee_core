use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageEnvelope {
  pub version: u64,
  pub message: MessageType,
}

/// MessageType represents all possible message types in the protocol
/// This mirrors the Go protocol.MessageType for JSON compatibility
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct MessageType {
  // Node to Manager messages
  #[serde(skip_serializing_if = "Option::is_none")]
  pub NodeRegistration: Option<NodeRegistration>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub NodeStatusUpdate: Option<NodeStatusUpdate>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub NodeEvent: Option<NodeEvent>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub NodeDrop: Option<bool>,

  // Honeypot-specific messages (Node → Manager)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub HoneypotStatusUpdate: Option<HoneypotStatusUpdate>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub HoneypotEvent: Option<HoneypotEvent>,

  // Manager to Node messages
  #[serde(skip_serializing_if = "Option::is_none")]
  pub NodeCommand: Option<NodeCommand>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub RegistrationAck: Option<RegistrationAck>,

  // Honeypot commands (Manager → Node)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub InstallHoneypot: Option<InstallHoneypotCmd>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub StartHoneypot: Option<StartHoneypotCmd>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub StopHoneypot: Option<StopHoneypotCmd>,
}

impl Eq for MessageType {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRegistration {
  pub node_id: u64,
  pub node_name: String,
  pub address: String,
  pub port: u16,
  pub node_type: NodeType,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub totp_code: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeStatusUpdate {
  pub node_id: u64,
  pub status: NodeStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeEvent {
  #[serde(rename = "type")]
  pub event_type: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub message: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeCommand {
  pub node_id: u64,
  pub command: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistrationAck {
  pub node_id: u64,
  pub accepted: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub message: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub totp_key: Option<String>,
}

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

// =============================================================================
// Honeypot Types
// =============================================================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum HoneypotStatus {
  Installing,
  Running,
  Stopped,
  Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HoneypotStatusUpdate {
  pub node_id: u64,
  pub honeypot_id: String,
  pub honeypot_type: String,
  pub status: HoneypotStatus,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub message: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ssh_port: Option<u16>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub telnet_port: Option<u16>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct HoneypotEvent {
  pub node_id: u64,
  pub honeypot_id: String,
  pub honeypot_type: String,
  pub eventid: String,
  pub timestamp: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub session: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub src_ip: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub src_port: Option<u16>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dst_ip: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dst_port: Option<u16>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub protocol: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub username: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub password: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub input: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub message: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub success: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub raw_event: Option<HashMap<String, serde_json::Value>>,
}

impl Eq for HoneypotEvent {}

// Honeypot Commands (Manager → Node)

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallHoneypotCmd {
  pub honeypot_id: String,
  pub honeypot_type: String,
  pub git_url: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub git_branch: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub config: Option<HashMap<String, String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ssh_port: Option<u16>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub telnet_port: Option<u16>,
  #[serde(default)]
  pub auto_start: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartHoneypotCmd {
  pub honeypot_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StopHoneypotCmd {
  pub honeypot_id: String,
}
