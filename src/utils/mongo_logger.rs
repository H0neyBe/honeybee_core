use std::sync::Arc;
use std::thread;

use mongodb::{
  Client,
  Collection,
  bson::doc,
};
use serde::{
  Deserialize,
  Serialize,
};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
  pub timestamp: String,
  pub level:     String,
  pub thread:    String,
  pub target:    String,
  pub line:      Option<u32>,
  pub message:   String,
}

#[derive(Clone)]
pub struct MongoLogger {
  collection: Arc<Mutex<Collection<LogEntry>>>,
}

impl MongoLogger {
  pub async fn new(uri: &str, database: &str, collection_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
    let client = Client::with_uri_str(uri).await?;
    let db = client.database(database);
    let collection = db.collection::<LogEntry>(collection_name);

    Ok(MongoLogger {
      collection: Arc::new(Mutex::new(collection)),
    })
  }

  pub async fn log(&self, entry: LogEntry) {
    let collection = self.collection.lock().await;
    if let Err(e) = collection.insert_one(entry).await {
      eprintln!("Failed to insert log to MongoDB: {}", e);
    }
  }
}

pub static MONGO_LOGGER: tokio::sync::OnceCell<MongoLogger> = tokio::sync::OnceCell::const_new();

pub async fn init_mongo_logger(uri: &str, database: &str, collection: &str) -> Result<(), Box<dyn std::error::Error>> {
  let logger = MongoLogger::new(uri, database, collection).await?;
  MONGO_LOGGER
    .set(logger)
    .map_err(|_| "MongoDB logger already initialized")?;
  Ok(())
}

pub fn log_to_mongo(level: log::Level, target: &str, line: Option<u32>, message: String) {
  let mut thread_info = thread::current()
    .name()
    .map(|n| n.to_string())
    .unwrap_or_else(|| format!("thread-{:?}", thread::current().id()));

  // Try to get Tokio task name if available
  if let Ok(_handle) = tokio::runtime::Handle::try_current() {
    if let Some(task_id) = tokio::task::try_id() {
      thread_info = format!("{}[task-{:?}]", thread_info, task_id);
    }
  }

  let entry = LogEntry {
    timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S.%3f").to_string(),
    level:     level.to_string(),
    thread:    thread_info,
    target:    target.to_string(),
    line,
    message,
  };

  if let Some(logger) = MONGO_LOGGER.get() {
    let logger = logger.clone();
    tokio::spawn(async move {
      logger.log(entry).await;
    });
  }
}
