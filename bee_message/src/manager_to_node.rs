use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Messages sent from Manager → Node

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ManagerToNodeMessage {
  NodeCommand(NodeCommand),
  RegistrationAck(RegistrationAck),
  // Honeypot commands
  InstallHoneypot(InstallHoneypotCmd),
  StartHoneypot(StartHoneypotCmd),
  StopHoneypot(StopHoneypotCmd),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeCommand {
  pub node_id: u64,
  pub command: String,
}

impl NodeCommand {
  /// Standard command: request status update
  pub fn status(node_id: u64) -> Self {
    Self {
      node_id,
      command: "status".to_string(),
    }
  }

  /// Standard command: stop node operations
  pub fn stop(node_id: u64) -> Self {
    Self {
      node_id,
      command: "stop".to_string(),
    }
  }

  /// Standard command: restart node
  pub fn restart(node_id: u64) -> Self {
    Self {
      node_id,
      command: "restart".to_string(),
    }
  }

  /// Custom command
  pub fn custom(node_id: u64, command: String) -> Self {
    Self { node_id, command }
  }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistrationAck {
  pub node_id: u64,
  pub accepted: bool,
  pub message: Option<String>,
}

// =============================================================================
// Honeypot Commands (Manager → Node)
// =============================================================================

/// Instructs the node to install a honeypot from a Git repository
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

impl InstallHoneypotCmd {
  /// Create a new Cowrie installation command
  pub fn cowrie(honeypot_id: String, git_url: String) -> Self {
    Self {
      honeypot_id,
      honeypot_type: "cowrie".to_string(),
      git_url,
      git_branch: None,
      config: None,
      ssh_port: Some(2222),
      telnet_port: Some(2223),
      auto_start: true,
    }
  }

  /// Set custom branch
  pub fn with_branch(mut self, branch: &str) -> Self {
    self.git_branch = Some(branch.to_string());
    self
  }

  /// Set SSH port
  pub fn with_ssh_port(mut self, port: u16) -> Self {
    self.ssh_port = Some(port);
    self
  }

  /// Set Telnet port
  pub fn with_telnet_port(mut self, port: u16) -> Self {
    self.telnet_port = Some(port);
    self
  }

  /// Disable auto-start
  pub fn without_auto_start(mut self) -> Self {
    self.auto_start = false;
    self
  }
}

/// Instructs the node to start a honeypot
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StartHoneypotCmd {
  pub honeypot_id: String,
}

impl StartHoneypotCmd {
  pub fn new(honeypot_id: String) -> Self {
    Self { honeypot_id }
  }
}

/// Instructs the node to stop a honeypot
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StopHoneypotCmd {
  pub honeypot_id: String,
}

impl StopHoneypotCmd {
  pub fn new(honeypot_id: String) -> Self {
    Self { honeypot_id }
  }
}