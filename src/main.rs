#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

mod backend_manager;
mod node_manager;
mod utils;
mod ws_gateway;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::{
  env,
  thread,
};

use backend_manager::BackendManager;
use ws_gateway::WsGateway;
use bee_config::Config;
use bee_message::BidirectionalMessage;
use node_manager::NodeManager;
use tokio::sync::mpsc;
#[cfg(feature = "tracing")]
use tracy_client::{
  Client,
  frame_mark,
  span,
};
use utils::logger;

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
  let config = Arc::new(config);

  match logger::init_logger(&config) {
    Ok(_) => log::debug!("Logger initialized successfully"),
    Err(e) => {
      eprintln!("Failed to initialize logger: {}", e);
      return Err(e);
    }
  }

  // Initialize MongoDB logger if enabled
  if config.logging.mongodb && !config.database.mongodb_uri.is_empty() {
    match utils::mongo_logger::init_mongo_logger(
      &config.database.mongodb_uri,
      &config.database.database,
      &config.database.collection,
    )
    .await
    {
      Ok(_) => log::info!("MongoDB logger initialized successfully"),
      Err(e) => {
        log::error!("Failed to initialize MongoDB logger: {}", e);
        eprintln!("Failed to initialize MongoDB logger: {}", e);
      }
    }

    // Initialize Honeypot logger (separate database)
    match utils::honeypot_logger::init_honeypot_logger(
      &config.database.mongodb_uri,
      &config.database.honeypot_database,
      &config.database.honeypot_collection,
    )
    .await
    {
      Ok(_) => log::info!("Honeypot MongoDB logger initialized successfully"),
      Err(e) => {
        log::error!("Failed to initialize honeypot MongoDB logger: {}", e);
        eprintln!("Failed to initialize honeypot MongoDB logger: {}", e);
      }
    }
  }

  log::debug!("Config loaded successfully");
  log::debug!("Loaded config: {:#?}", config);

  log::debug!(
    "Starting Node Manager On Thread: {}",
    thread::current().name().unwrap_or("main")
  );
  let (backend_manager_writer, node_manager_reader) = mpsc::unbounded_channel::<BidirectionalMessage>();
  let (node_manager_writer, backend_manager_reader) = mpsc::unbounded_channel::<BidirectionalMessage>();

  let node_manager = NodeManager::build(&config, node_manager_reader, backend_manager_writer).await?;
  let node_manager = Arc::new(node_manager);

  let backend_manager =
    BackendManager::build(&config, backend_manager_reader, node_manager_writer, Arc::clone(&node_manager)).await?;

  let node_manager_clone = Arc::clone(&node_manager);
  tokio::spawn(async move {
    if let Err(e) = backend_manager.listen().await {
      log::error!("Backend manager error: {}", e);
    }
  });

    // Start WebSocket gateway if enabled
  if config.proxy.enabled {
    let proxy_socket = std::net::SocketAddr::from(config.proxy.clone());
    let node_manager_ws = Arc::clone(&node_manager);
    let config_ws = Arc::clone(&config);

    tokio::spawn(async move {
      log::info!("Starting WebSocket Gateway at {}", proxy_socket);
      let gateway = WsGateway::new(proxy_socket, Arc::clone(&node_manager_ws), Arc::clone(&config_ws));
      if let Err(e) = gateway.run().await {
        log::error!("WebSocket Gateway error: {}", e);
      }
    });
  }

  // Start monitoring task if debug mode is enabled
  if config.server.debug {
    log::info!("Starting monitoring task");
    let node_manager_clone = node_manager.clone();
    tokio::spawn(async move {
      let mut interval = tokio::time::interval(Duration::from_secs(10));
      loop {
        interval.tick().await;
        let active = node_manager_clone.active_connections().await;
        let nodes = node_manager_clone.get_nodes().await;
        let node_count = nodes.len();
        log::info!("Status: {} active connections, {} registered nodes", active, node_count);
      }
    });
  }

  // Start the node manager listener
  if let Err(e) = node_manager.listen().await {
    log::error!("Node manager error: {}", e);
    return Err(e.into());
  }

  Ok(())
}
