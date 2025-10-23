#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

mod node;
mod utils;

use bee_config::Config;
use std::path::Path;
use std::{env, thread};
use utils::logger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let config = Config::load_or_create(Path::new("bee_config.toml"))?;
  logger::init_logger(&config)?;

  log::debug!("Config loaded successfully");
  log::debug!("Loaded config: {:#?}", config);

  thread::spawn(move || {
    log::debug!("Hello from the spawned thread!");
  });

  Ok(())
}
