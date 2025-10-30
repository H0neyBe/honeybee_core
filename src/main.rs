#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

mod node_manager;
mod utils;

use std::path::Path;
use std::time::Duration;
use std::{env, thread};

use bee_config::Config;
#[cfg(feature = "tracing")]
use tracy_client::{frame_mark, span, Client};
use utils::logger;

// Enable Tracy's global allocator for memory tracking
#[cfg(feature = "tracing")]
#[global_allocator]
static GLOBAL: tracy_client::ProfiledAllocator<std::alloc::System> =
  tracy_client::ProfiledAllocator::new(std::alloc::System, 100);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  #[cfg(feature = "tracing")]
  let client = Client::start();

  #[cfg(feature = "tracing")]
  let _span = span!("Main");

  println!("Starting Honeybee Core Node");

  let config = Config::load_or_create(Path::new("bee_config.toml"))?;

  match logger::init_logger(&config) {
    Ok(_) => log::debug!("Logger initialized successfully"),
    Err(e) => {
      eprintln!("Failed to initialize logger: {}", e);
      return Err(e);
    }
  }

  log::debug!("Config loaded successfully");
  log::debug!("Loaded config: {:#?}", config);

  log::debug!(
    "Starting Node Manager On Thread: {}",
    thread::current().name().unwrap_or("main")
  );
  let node_manager = node_manager::NodeManager::start_node_manager(&config).await?;
  let node_manager = std::sync::Arc::new(node_manager);

  // Optional: Spawn monitoring task
  if config.server.debug {
    log::info!("Starting monitoring task");
    let node_manager_clone = node_manager.clone();
    let monitor_handle = {
      let nm = node_manager_clone.clone();
      tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
          interval.tick().await;
          let active = nm.active_connections().await;
          let nodes = nm.get_nodes().await;
          let node_count = nodes.read().await.len();
          log::info!("Active connections: {}, Registered nodes: {}", active, node_count);
          
          #[cfg(feature = "tracing")]
          {
            tracy_client::plot!("monitor.active_connections", active as f64);
            tracy_client::plot!("monitor.registered_nodes", node_count as f64);
          }
        }
      })
    };
  }

  node_manager.listen().await?;

  #[cfg(feature = "tracing")]
  frame_mark();

  Ok(())
}