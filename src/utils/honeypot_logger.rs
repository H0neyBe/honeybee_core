use std::sync::Arc;

use mongodb::{
  bson::{
    doc,
    to_bson,
    Bson,
    Document,
  },
  Client,
  Collection,
};
use log::{
  debug,
  error,
};
use serde::{
  Deserialize,
  Serialize,
};
use tokio::sync::Mutex;

use bee_message::PotLog;

#[derive(Clone)]
pub struct HoneypotLogger {
  collection: Arc<Mutex<Collection<Document>>>,
}

impl HoneypotLogger {
  pub async fn new(uri: &str, database: &str, collection_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
    let client = Client::with_uri_str(uri).await?;
    let db = client.database(database);
    let collection = db.collection::<Document>(collection_name);

    Ok(HoneypotLogger {
      collection: Arc::new(Mutex::new(collection)),
    })
  }

  pub async fn log(&self, pot_log: bee_message::PotLog) {
    let collection = self.collection.lock().await;
    let mut doc = Document::new();
    let pot_id = pot_log.pot_id.clone();
    let log_type = pot_log.log_type.clone();
    doc.insert("node_id", pot_log.node_id.to_string());
    doc.insert("pot_id", pot_log.pot_id);
    doc.insert("pot_type", pot_log.pot_type);
    doc.insert("log_type", pot_log.log_type);
    doc.insert("timestamp", pot_log.timestamp);

    match to_bson(&pot_log.data) {
      Ok(Bson::Document(data_doc)) => {
        let normalized = normalize_pot_log_data(data_doc);
        for (k, v) in normalized.top_level {
          doc.insert(k, v);
        }
        if !normalized.remaining.is_empty() {
          doc.insert("data", Bson::Document(normalized.remaining));
        }
      }
      Ok(other) => {
        doc.insert("data", other);
      }
      Err(e) => {
        error!("Failed to serialize pot log data: {}", e);
        eprintln!("Failed to serialize pot log data: {}", e);
        return;
      }
    }

    if let Err(e) = collection.insert_one(doc).await {
      error!(
        "Failed to insert honeypot log to MongoDB (node_id={}, pot_id={}): {}",
        pot_log.node_id,
        pot_id,
        e
      );
      eprintln!("Failed to insert honeypot log to MongoDB: {}", e);
    } else {
      debug!(
        "Inserted honeypot log (node_id={}, pot_id={}, log_type={})",
        pot_log.node_id,
        pot_id,
        log_type
      );
    }
  }
}

struct NormalizedPotLogData {
  top_level: Document,
  remaining: Document,
}

fn normalize_pot_log_data(mut data: Document) -> NormalizedPotLogData {
  let mut top_level = Document::new();

  // Drop noisy or duplicate keys
  data.remove("event");
  if let Some(source) = data.remove("source") {
    top_level.insert("stream", source);
  }
  if let Some(stream) = data.remove("stream") {
    top_level.insert("stream", stream);
  }
  data.remove("pot_type");

  // Promote common fields to top-level
  promote_field(&mut data, &mut top_level, "message");
  promote_field(&mut data, &mut top_level, "timestamp");
  promote_field(&mut data, &mut top_level, "system");
  promote_field(&mut data, &mut top_level, "component");
  promote_field(&mut data, &mut top_level, "level");
  promote_field(&mut data, &mut top_level, "src_ip");
  promote_field(&mut data, &mut top_level, "src_port");
  promote_field(&mut data, &mut top_level, "dst_ip");
  promote_field(&mut data, &mut top_level, "dst_port");
  promote_field(&mut data, &mut top_level, "session");
  promote_field(&mut data, &mut top_level, "username");
  promote_field(&mut data, &mut top_level, "password");
  promote_field(&mut data, &mut top_level, "auth");
  promote_field(&mut data, &mut top_level, "protocol");
  promote_field(&mut data, &mut top_level, "version");
  promote_field(&mut data, &mut top_level, "hassh");
  promote_field(&mut data, &mut top_level, "kex");
  promote_field(&mut data, &mut top_level, "kex_alg");
  promote_field(&mut data, &mut top_level, "key_alg");
  promote_field(&mut data, &mut top_level, "cipher_in");
  promote_field(&mut data, &mut top_level, "mac_in");
  promote_field(&mut data, &mut top_level, "comp_in");
  promote_field(&mut data, &mut top_level, "cipher_out");
  promote_field(&mut data, &mut top_level, "mac_out");
  promote_field(&mut data, &mut top_level, "comp_out");
  promote_field(&mut data, &mut top_level, "service");
  promote_field(&mut data, &mut top_level, "duration");
  promote_field(&mut data, &mut top_level, "uuid");
  promote_field(&mut data, &mut top_level, "sensor");
  promote_field(&mut data, &mut top_level, "time");

  NormalizedPotLogData {
    top_level,
    remaining: data,
  }
}

fn promote_field(source: &mut Document, target: &mut Document, key: &str) {
  if let Some(value) = source.remove(key) {
    target.insert(key, value);
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

pub async fn log_honeypot_event(pot_log: bee_message::PotLog) {
  if let Some(logger) = HONEYPOT_LOGGER.get() {
    logger.log(pot_log).await;
  }
}
