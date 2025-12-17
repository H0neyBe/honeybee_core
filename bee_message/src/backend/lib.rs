use serde::{
  Deserialize,
  Serialize,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackendType {
  Web,
  Api,
  Monitoring,
  Cli,
  Custom(String),
}
