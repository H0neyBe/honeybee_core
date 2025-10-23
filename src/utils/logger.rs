use bee_config::Config;
use colored::Colorize;
use std::fs::OpenOptions;

pub fn init_logger(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
  let log_level: log::LevelFilter = config.logging.level.parse().unwrap();

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

      out.finish(format_args!(
        "[{}][{}] {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string().dimmed(),
        level_colored,
        message
      ))
    })
    .level(log_level)
    .chain(std::io::stdout());

  let mut base_dispatch = fern::Dispatch::new().chain(console_dispatch);

  // File output without colors
  if let Some(file) = &config.logging.file {
    let file_dispatch = fern::Dispatch::new()
      .format(|out, message, record| {
        out.finish(format_args!(
          "[{}][{}] {}",
          chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
          record.level(),
          message
        ))
      })
      .level(log_level)
      .chain(
        OpenOptions::new()
          .create(true)
          .write(true)
          // .append(true)
          .open(file)?,
      );

    base_dispatch = base_dispatch.chain(file_dispatch);
  }

  base_dispatch.apply()?;
  Ok(())
}