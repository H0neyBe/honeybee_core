use std::fs;
use std::path::Path;

use serde::{
  Deserialize,
  Serialize,
};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
  pub server:   ServerConfig,
  pub database: DatabaseConfig,
  pub logging:  LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
  pub host: String,
  pub port: u16,
  pub debug: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
  pub url:             String,
  pub max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
  pub level: String,
  pub folder:  Option<String>,
}

impl Default for ServerConfig {
  fn default() -> Self {
    ServerConfig {
      host: "127.0.0.1".to_string(),
      port: 9001,
      debug: false,
    }
  }
}

impl From<ServerConfig> for std::net::SocketAddr {
  fn from(config: ServerConfig) -> Self { format!("{}:{}", config.host, config.port).parse().unwrap() }
}

impl Default for DatabaseConfig {
  fn default() -> Self {
    DatabaseConfig {
      url:             "postgres://localhost/mydb".to_string(),
      max_connections: 10,
    }
  }
}

impl Default for LoggingConfig {
  fn default() -> Self {
    LoggingConfig {
      level: "info".to_string(),
      folder:  None,
    }
  }
}

impl Config {
  fn parse(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let config = toml::from_str(&contents)?;
    Ok(config)
  }

  pub fn load_or_create(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
    if path.exists() {
      Self::parse(path)
    } else {
      let default_config = Self::default();
      default_config.save(path)?;
      Ok(default_config)
    }
  }

  pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let toml_str = toml::to_string_pretty(self)?;
    fs::write(path, toml_str)?;
    Ok(())
  }

  pub fn get_address(&self) -> std::net::SocketAddr { self.server.clone().into() }
}
