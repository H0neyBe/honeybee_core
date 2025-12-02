use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::common::{NodeStatus, NodeType, HoneypotStatus};

/// Messages sent from Node → Manager

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeToManagerMessage {
  NodeRegistration(NodeRegistration),
  NodeStatusUpdate(NodeStatusUpdate),
  NodeEvent(NodeEvent),
  NodeDrop,
  // Honeypot-specific messages
  HoneypotStatusUpdate(HoneypotStatusUpdate),
  HoneypotEvent(HoneypotEvent),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeRegistration {
  pub node_id: u64,
  pub node_name: String,
  pub address: String,
  pub port: u16,
  pub node_type: NodeType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeStatusUpdate {
  pub node_id: u64,
  pub status: NodeStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeEvent {
  Started,
  Stopped,
  Alarm { description: String },
  Error { message: String },
}

// =============================================================================
// Honeypot-specific Messages (Node → Manager)
// =============================================================================

/// Reports the current state of a honeypot
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

/// Represents events captured by the honeypot (attacks, sessions, etc.)
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