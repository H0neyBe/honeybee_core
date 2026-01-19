use std::collections::HashMap;

use serde::{
  Deserialize,
  Serialize,
};

use crate::PotId;

/// Empty unit type that can deserialize from both null and {}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Unit;

/// Messages sent from Manager → Node

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ManagerToNodeMessage {
  NodeCommand(NodeCommand),
  RegistrationAck(RegistrationAck),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistrationAck {
  pub node_id:  u64,
  pub accepted: bool,
  pub message:  Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeCommand {
  pub node_id: u64,
  pub command: NodeCommandType,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeCommandType {
  Restart(Unit),
  UpdateConfig(Unit),
  InstallPot(InstallPot),
  DeployPot(PotId),
  GetPotStatus(PotId),
  RestartPot(PotId),
  StopPot(PotId),
  GetPotLogs(PotId),
  GetPotMetrics(PotId),
  GetPotInfo(PotId),
  GetInstalledPots(Unit),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstallPot {
  pub pot_id:        PotId,
  pub honeypot_type: String,
  pub git_url:       Option<String>,
  pub git_branch:    Option<String>,
  pub config:        Option<HashMap<String, String>>,
  pub auto_start:    bool,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_node_command_serialization() {
    // Test unit variant serialization
    let restart = NodeCommandType::Restart(Unit);
    let json = serde_json::to_string(&restart).unwrap();
    assert_eq!(json, r#"{"Restart":{}}"#);

    let get_installed = NodeCommandType::GetInstalledPots(Unit);
    let json = serde_json::to_string(&get_installed).unwrap();
    assert_eq!(json, r#"{"GetInstalledPots":{}}"#);

    // Test string variant serialization
    let get_status = NodeCommandType::GetPotStatus("cowrie-01".to_string());
    let json = serde_json::to_string(&get_status).unwrap();
    assert_eq!(json, r#"{"GetPotStatus":"cowrie-01"}"#);

    let deploy = NodeCommandType::DeployPot("test-pot".to_string());
    let json = serde_json::to_string(&deploy).unwrap();
    assert_eq!(json, r#"{"DeployPot":"test-pot"}"#);

    // Test struct variant serialization
    let install = NodeCommandType::InstallPot(InstallPot {
      pot_id:        "cowrie-01".to_string(),
      honeypot_type: "cowrie".to_string(),
      git_url:       None,
      git_branch:    None,
      config:        None,
      auto_start:    true,
    });
    let json = serde_json::to_string(&install).unwrap();
    assert!(json.contains(r#""InstallPot":"#));
    assert!(json.contains(r#""pot_id":"cowrie-01""#));
    assert!(json.contains(r#""honeypot_type":"cowrie""#));
    assert!(json.contains(r#""auto_start":true"#));
  }

  #[test]
  fn test_node_command_deserialization() {
    // Test unit variant deserialization from {}
    let json = r#"{"Restart":{}}"#;
    let cmd: NodeCommandType = serde_json::from_str(json).unwrap();
    assert_eq!(cmd, NodeCommandType::Restart(Unit));

    // Test unit variant deserialization from null
    let json = r#"{"GetInstalledPots":null}"#;
    let cmd: NodeCommandType = serde_json::from_str(json).unwrap();
    assert_eq!(cmd, NodeCommandType::GetInstalledPots(Unit));

    // Test string variant deserialization
    let json = r#"{"GetPotStatus":"cowrie-01"}"#;
    let cmd: NodeCommandType = serde_json::from_str(json).unwrap();
    assert_eq!(cmd, NodeCommandType::GetPotStatus("cowrie-01".to_string()));
  }
}
