use std::sync::Arc;

use mongodb::{
  bson::doc,
  Client,
  Collection,
};
use serde::{
  Deserialize,
  Serialize,
};
use tokio::sync::Mutex;

use bee_message::PotLog;

#[derive(Clone)]
pub struct HoneypotLogger {
  collection: Arc<Mutex<Collection<PotLog>>>,
}

impl HoneypotLogger {
  pub async fn new(uri: &str, database: &str, collection_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
    let client = Client::with_uri_str(uri).await?;
    let db = client.database(database);
    let collection = db.collection::<PotLog>(collection_name);

    Ok(HoneypotLogger {
      collection: Arc::new(Mutex::new(collection)),
    })
  }

  pub async fn log(&self, pot_log: PotLog) {
    let collection = self.collection.lock().await;
    if let Err(e) = collection.insert_one(pot_log).await {
      eprintln!("Failed to insert honeypot log to MongoDB: {}", e);
    }
  }
}

pub static HONEYPOT_LOGGER: tokio::sync::OnceCell<HoneypotLogger> = tokio::sync::OnceCell::const_new();

pub async fn init_honeypot_logger(uri: &str, database: &str, collection: &str) -> Result<(), Box<dyn std::error::Error>> {
  let logger = HoneypotLogger::new(uri, database, collection).await?;
  HONEYPOT_LOGGER
    .set(logger)
    .map_err(|_| "Honeypot logger already initialized")?;
  Ok(())
}

pub async fn log_honeypot_event(pot_log: PotLog) {
  if let Some(logger) = HONEYPOT_LOGGER.get() {
    logger.log(pot_log).await;
  }
}
