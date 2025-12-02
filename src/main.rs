#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

mod node_manager;
mod utils;

use std::path::Path;
use std::time::Duration;
use std::{env, thread};
use std::io::{self, Write, BufRead};

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

  println!("🐝 Starting HoneyBee Core");
  println!("════════════════════════════════════════");

  let config = Config::load_or_create(Path::new("bee_config.toml"))?;

  match logger::init_logger(&config) {
    Ok(_) => log::debug!("Logger initialized successfully"),
    Err(e) => {
      eprintln!("Failed to initialize logger: {}", e);
      return Err(e);
    }
  }

  log::debug!("Config loaded successfully");

  let node_manager = node_manager::NodeManager::start_node_manager(&config).await?;
  let node_manager = std::sync::Arc::new(node_manager);

  // Spawn listener in background
  let nm_listener = node_manager.clone();
  tokio::spawn(async move {
    if let Err(e) = nm_listener.listen().await {
      log::error!("Listener error: {}", e);
    }
  });

  // Print help
  println!("\n📋 Commands:");
  println!("  list              - List connected nodes");
  println!("  install <node_id> - Install Cowrie on node");
  println!("  quit              - Exit\n");

  // Simple command loop
  let nm_cmd = node_manager.clone();
  tokio::spawn(async move {
    let stdin = io::stdin();
    loop {
      print!("honeybee> ");
      io::stdout().flush().ok();

      let mut input = String::new();
      if stdin.lock().read_line(&mut input).is_err() {
        break;
      }

      let parts: Vec<&str> = input.trim().split_whitespace().collect();
      if parts.is_empty() {
        continue;
      }

      match parts[0] {
        "list" | "ls" => {
          let nodes = nm_cmd.list_nodes().await;
          if nodes.is_empty() {
            println!("  No nodes connected");
          } else {
            println!("  Connected nodes:");
            for (id, name) in nodes {
              println!("    {} - {}", id, name);
            }
          }
        }
        "install" => {
          if parts.len() < 2 {
            println!("  Usage: install <node_id>");
            continue;
          }
          let node_id: u64 = match parts[1].parse() {
            Ok(id) => id,
            Err(_) => {
              println!("  Invalid node ID");
              continue;
            }
          };
          
          println!("  Installing Cowrie on node {}...", node_id);
          match nm_cmd.install_honeypot(node_id, "cowrie-01", "cowrie").await {
            Ok(_) => println!("  ✅ Install command sent!"),
            Err(e) => println!("  ❌ Error: {}", e),
          }
        }
        "quit" | "exit" => {
          println!("Goodbye!");
          std::process::exit(0);
        }
        _ => {
          println!("  Unknown command: {}", parts[0]);
        }
      }
    }
  }).await?;

  #[cfg(feature = "tracing")]
  frame_mark();

  Ok(())
}