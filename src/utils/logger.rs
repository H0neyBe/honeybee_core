use std::fs::OpenOptions;
use std::thread;

use bee_config::Config;
use colored::Colorize;

pub fn init_logger(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
  let log_level: log::LevelFilter = config.logging.level.parse().unwrap();

  if config.logging.force_color {
    colored::control::set_override(true);
  }

  // Console output with colors
  let console_dispatch = fern::Dispatch::new()
    .format(|out, message, record| {
      let level_colored = match record.level() {
        log::Level::Error => record.level().to_string().red().bold(),
        log::Level::Warn => record.level().to_string().yellow().bold(),
        log::Level::Info => record.level().to_string().green().bold(),
        log::Level::Debug => record.level().to_string().blue().bold(),
        log::Level::Trace => record.level().to_string().purple().bold(),
      };

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
      #[cfg(debug_assertions)]
      let location = if let Some(line) = record.line() {
        format!("{}:{}", record.target(), line)
      } else {
        record.target().to_string()
      };

      #[cfg(debug_assertions)]
      out.finish(format_args!(
        "[{}][{}][{}][{}] {}",
        chrono::Local::now()
          .format("%Y-%m-%d %H:%M:%S.%3f")
          .to_string()
          .black()
          .dimmed(),
        level_colored,
        thread_info.cyan(),
        location.bright_black(),
        message
      ));

      #[cfg(not(debug_assertions))]
      out.finish(format_args!(
        "[{}][{}][{}] {}",
        chrono::Local::now()
          .format("%Y-%m-%d %H:%M:%S.%3f")
          .to_string()
          .black()
          .dimmed(),
        level_colored,
        thread_info.cyan(),
        message
      ))
    })
    .level(log_level)
    .chain(std::io::stdout());

  let mut base_dispatch = fern::Dispatch::new().chain(console_dispatch);

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

  Ok(())
}