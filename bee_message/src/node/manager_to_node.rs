use std::collections::HashMap;

use serde::{
  Deserialize,
  Serialize,
};

use crate::PotId;

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
  Restart,
  UpdateConfig,
  InstallPot(InstallPot),
  DeployPot(PotId),
  GetPotStatus(PotId),
  RestartPot(PotId),
  StopPot(PotId),
  GetPotLogs(PotId),
  GetPotMetrics(PotId),
  GetPotInfo(PotId),
  GetInstalledPots,
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
