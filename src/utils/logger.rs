use std::fs::OpenOptions;
use std::thread;

use bee_config::Config;
use colored::Colorize;

use super::mongo_logger;

use bee_message::backend::manager_to_backend::CoreLogMessage;
use tokio::sync::broadcast;

pub fn init_logger(config: &Config) -> Result<broadcast::Receiver<CoreLogMessage>, Box<dyn std::error::Error>> {
  let log_level: log::LevelFilter = config.logging.level.parse().unwrap();

  if config.logging.force_color {
    colored::control::set_override(true);
  }

  // Console output with colors
  let mongodb_enabled = config.logging.mongodb;
  let console_dispatch = fern::Dispatch::new()
    .format(move |out, message, record| {
      let level_colored = match record.level() {
        log::Level::Error => record.level().to_string().red().bold(),
        log::Level::Warn => record.level().to_string().yellow().bold(),
        log::Level::Info => record.level().to_string().green().bold(),
        log::Level::Debug => record.level().to_string().blue().bold(),
        log::Level::Trace => record.level().to_string().purple().bold(),
      };

      // Get simplified module name
      let module = record.target()
        .split("::")
        .last()
        .unwrap_or(record.target());

      // Log to MongoDB if enabled
      if mongodb_enabled {
        mongo_logger::log_to_mongo(
          record.level(),
          record.target(),
          record.line(),
          message.to_string(),
        );
      }

      // Simplified format: [TIME][LEVEL][MODULE] message
      out.finish(format_args!(
        "[{}][{}][{}] {}",
        chrono::Local::now()
          .format("%H:%M:%S")
          .to_string()
          .dimmed(),
        level_colored,
        module.cyan(),
        message
      ))
    })
    .level(log_level)
    // Filter out noisy third-party crates
    .level_for("hickory_proto", log::LevelFilter::Warn)
    .level_for("hickory_resolver", log::LevelFilter::Warn)
    .level_for("rustls", log::LevelFilter::Warn)
    .level_for("tower_http", log::LevelFilter::Info)
    .level_for("hyper", log::LevelFilter::Warn)
    .level_for("tokio", log::LevelFilter::Warn)
    .chain(std::io::stdout());

  let (tx, rx) = broadcast::channel(100);
  let tx_clone = tx.clone();

  let broadcast_dispatch = fern::Dispatch::new()
    .format(move |out, message, record| {
        out.finish(format_args!("{}", message))
    })
    .chain(fern::Output::call(move |record| {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        let msg = CoreLogMessage {
            level: record.level().to_string(),
            target: record.target().to_string(),
            message: record.args().to_string(),
            timestamp,
        };
        let _ = tx_clone.send(msg);
    }));

  let mut base_dispatch = fern::Dispatch::new()
      .chain(console_dispatch)
      .chain(broadcast_dispatch);

  // File output without colors
  if let Some(folder) = &config.logging.folder {
    let file = std::path::PathBuf::from(folder).join(format!("debug-{}.log", chrono::Local::now().format("%Y%m%d_%H%M%S")));

    let file_dispatch = fern::Dispatch::new()
      .format(|out, message, record| {
        // Get thread name or ID
        let mut thread_info = thread::current()
          .name()
          .map(|n| n.to_string())
          .unwrap_or_else(|| format!("thread-{:?}", thread::current().id()));

        // Try to get Tokio task name if available
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
          if let Some(task_id) = tokio::task::try_id() {
            thread_info = format!("{}[task-{:?}]", thread_info, task_id);
          }
        }

        // Get target (module path) and line number
        let location = if let Some(line) = record.line() {
          format!("{}:{}", record.target(), line)
        } else {
          record.target().to_string()
        };

        out.finish(format_args!(
          "[{}][{}][{}][{}] {}",
          chrono::Local::now().format("%Y-%m-%d %H:%M:%S.%3f"),
          record.level(),
          thread_info,
          location,
          message
        ))
      })
      .level(log_level)
      .chain(
        OpenOptions::new()
          .create(true)
          .truncate(true)
          .write(true)
          .open(file)?,
      );

    base_dispatch = base_dispatch.chain(file_dispatch);
  }

  base_dispatch.apply()?;

  Ok(rx)
}