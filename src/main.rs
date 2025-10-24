#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

mod utils;
mod node_manager;

use bee_config::Config;
use bee_message::node;
use std::path::Path;
use std::{env, thread};
use utils::logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let config = Config::load_or_create(Path::new("bee_config.toml"))?;
  logger::init_logger(&config)?;

  log::debug!("Config loaded successfully");
  log::debug!("Loaded config: {:#?}", config);

  log::debug!("Starting Node Manager Thread: {}", thread::current().name().unwrap_or("main"));
  let node_manager = node_manager::NodeManager::start_node_manager(&config).await?;

  node_manager.listen().await?;

  Ok(())
}
