#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

mod node_manager;
mod utils;

use std::path::Path;
use std::{
  env,
  thread,
};

use bee_config::Config;
use bee_message::node;
#[cfg(feature = "tracing")]
use tracy_client::{
  Client,
  frame_mark,
  span,
};
use utils::logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  #[cfg(feature = "tracing")]
  let client = Client::start();

  #[cfg(feature = "tracing")]
  let _span = span!("Main");

  let config = Config::load_or_create(Path::new("bee_config.toml"))?;
  logger::init_logger(&config)?;

  log::debug!("Config loaded successfully");
  log::debug!("Loaded config: {:#?}", config);

  log::debug!(
    "Starting Node Manager On Thread: {}",
    thread::current().name().unwrap_or("main")
  );
  let node_manager = node_manager::NodeManager::start_node_manager(&config).await?;

  node_manager.listen().await?;

  #[cfg(feature = "tracing")]
  frame_mark();

  Ok(())
}
