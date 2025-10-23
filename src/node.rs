// /home/hqnw/Desktop/project_4/code/honeybee_core/src/node.rs
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, default, time::{SystemTime, UNIX_EPOCH}};

#[derive(Debug, Clone)]
pub struct Node {
  id: u64,
  name: String,
  deployment_config: DeploymentConfig,
  status: NodeStatus,
  created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeStatus {
  Deploying,
  Running,
  Stopped,
  Failed,
  Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
  node_type: NodeType,
  address: String,
  port: u16,
  config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
  Full,
  Agent,
}

impl Default for DeploymentConfig {
  fn default() -> Self {
    DeploymentConfig {
      node_type: NodeType::Agent,
      address: String::from("0.0.0.0"),
      port: 8080,
      config: HashMap::new(),
    }
  }
}

impl Default for Node {
  fn default() -> Self {
    Node {
      id: 0,
      name: String::from(""),
      deployment_config: DeploymentConfig::default(),
      status: NodeStatus::Unknown,
      created_at: 0,
    }
  }
}

impl Node {
  pub fn new(id: u64, name: String, deployment_config: DeploymentConfig) -> Self {
    Node {
      id,
      name,
      deployment_config,
      status: NodeStatus::Deploying,
      created_at: Self::current_timestamp(),
    }
  }

  fn current_timestamp() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs()
  }
}

