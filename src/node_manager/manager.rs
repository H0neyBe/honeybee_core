use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use bee_config::Config;
use bee_message::node::{
    MessageEnvelope, MessageType, NodeRegistration, RegistrationAck,
    InstallHoneypotCmd, NodeStatus, HoneypotEvent, HoneypotStatusUpdate,
};
use bee_message::PROTOCOL_VERSION;
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, tcp::OwnedWriteHalf};
use tokio::sync::{RwLock, mpsc};
use tokio::task::JoinHandle;

use super::node::Node;

// Channel for sending commands to nodes
type NodeWriter = Arc<RwLock<Option<OwnedWriteHalf>>>;
type NodeWriters = Arc<RwLock<HashMap<u64, NodeWriter>>>;

#[derive(Debug)]
pub struct NodeManager {
  nodes: Arc<RwLock<HashMap<u64, Node>>>,
  listener: TcpListener,
  address: SocketAddr,
  // Track spawned tasks to prevent leaks
  tasks: Arc<RwLock<HashMap<u64, JoinHandle<()>>>>,
  // Writers to send commands to nodes
  writers: NodeWriters,
}

impl NodeManager {
  pub async fn start_node_manager(config: &Config) -> Result<Self, std::io::Error> {
    let address = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&address).await?;
    let addr = listener.local_addr()?;

    log::info!("Node Manager listening on: {}", addr);

    Ok(NodeManager {
      nodes: Arc::new(RwLock::new(HashMap::new())),
      listener,
      address: addr,
      tasks: Arc::new(RwLock::new(HashMap::new())),
      writers: Arc::new(RwLock::new(HashMap::new())),
    })
  }

  /// Send InstallHoneypot command to a specific node
  pub async fn install_honeypot(&self, node_id: u64, honeypot_id: &str, honeypot_type: &str) -> Result<(), String> {
    let writers = self.writers.read().await;
    let writer_lock = writers.get(&node_id).ok_or(format!("Node {} not found", node_id))?;
    
    let mut msg = MessageType::default();
    msg.InstallHoneypot = Some(InstallHoneypotCmd {
      honeypot_id: honeypot_id.to_string(),
      honeypot_type: honeypot_type.to_string(),
      git_url: String::new(), // Will use PotStore default
      git_branch: None,
      config: None,
      ssh_port: Some(2222),
      telnet_port: Some(2223),
      auto_start: true,
    });

    let envelope = MessageEnvelope {
      version: PROTOCOL_VERSION,
      message: msg,
    };

    let mut json = serde_json::to_string(&envelope).map_err(|e| e.to_string())?;
    json.push('\n');

    let mut writer_guard = writer_lock.write().await;
    if let Some(ref mut writer) = *writer_guard {
      writer.write_all(json.as_bytes()).await.map_err(|e| e.to_string())?;
      writer.flush().await.map_err(|e| e.to_string())?;
      log::info!("Sent InstallHoneypot command to node {}", node_id);
      Ok(())
    } else {
      Err("Writer not available".to_string())
    }
  }

  /// Get list of connected node IDs
  pub async fn list_nodes(&self) -> Vec<(u64, String)> {
    let nodes = self.nodes.read().await;
    nodes.iter().map(|(id, node)| (*id, node.name.clone())).collect()
  }

  pub async fn listen(&self) -> Result<(), std::io::Error> {
    log::info!("Starting listener on {}", self.address);
    loop {
      let (socket, addr) = self.listener.accept().await?;
      let nodes = Arc::clone(&self.nodes);
      let tasks = Arc::clone(&self.tasks);
      let writers = Arc::clone(&self.writers);

      tokio::spawn(async move {
        log::debug!("Got a new connection from: {}", addr);

        let (reader, writer) = socket.into_split();
        let writer = Arc::new(RwLock::new(Some(writer)));
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        // Read initial registration message (line-delimited JSON)
        line.clear();
        match reader.read_line(&mut line).await {
          Ok(0) => {
            log::warn!("Connection closed by {} before sending registration", addr);
            return;
          }
          Ok(_) => {}
          Err(e) => {
            log::error!("Failed to read registration from {}: {}", addr, e);
            return;
          }
        }

        log::debug!("Received registration message: {}", line.trim());

        // Parse registration envelope
        let envelope: MessageEnvelope = match serde_json::from_str(&line) {
          Ok(reg) => reg,
          Err(e) => {
            log::error!("Failed to parse registration from {}: {} - raw: {}", addr, e, line.trim());
            return;
          }
        };

        // Validate protocol version
        if envelope.version != PROTOCOL_VERSION {
          log::warn!(
            "Version mismatch from {}: got {}, expected {}",
            addr,
            envelope.version,
            PROTOCOL_VERSION
          );
        }

        // Extract node from registration
        let node_reg = match envelope.message.NodeRegistration {
          Some(reg) => reg,
          None => {
            log::error!("Expected NodeRegistration message, got: {:?}", envelope.message);
            return;
          }
        };

        let node = Node::from(node_reg.clone());
        let node_id = node.id;
        log::info!("Node {} ({}) registered from {}", node_id, node.name, addr);

        // Send registration acknowledgment
        let mut ack_msg = MessageType::default();
        ack_msg.RegistrationAck = Some(RegistrationAck {
          node_id,
          accepted: true,
          message: Some(format!("Welcome {}", node.name)),
          totp_key: None,
        });

        let ack_envelope = MessageEnvelope {
          version: PROTOCOL_VERSION,
          message: ack_msg,
        };

        if let Ok(mut ack_json) = serde_json::to_string(&ack_envelope) {
          ack_json.push('\n');
          
          let mut writer_guard = writer.write().await;
          if let Some(ref mut w) = *writer_guard {
            if let Err(e) = w.write_all(ack_json.as_bytes()).await {
              log::error!("Failed to send ACK to node {}: {}", node_id, e);
            } else if let Err(e) = w.flush().await {
              log::error!("Failed to flush ACK to node {}: {}", node_id, e);
            } else {
              log::debug!("Sent registration ACK to node {}", node_id);
            }
          }
        }

        // Store node and writer
        nodes.write().await.insert(node.id, node);
        writers.write().await.insert(node_id, Arc::clone(&writer));

        // Spawn task to handle node messages
        let nodes_clone = Arc::clone(&nodes);
        let tasks_clone = Arc::clone(&tasks);
        let writers_clone = Arc::clone(&writers);
        let handle = tokio::spawn(async move {
          log::debug!("Started message handler for node: {}", node_id);

          loop {
            line.clear();
            match reader.read_line(&mut line).await {
              Ok(0) => {
                log::info!("Node {} disconnected", node_id);
                break;
              }
              Ok(_) => {}
              Err(e) => {
                log::error!("Error reading from node {}: {}", node_id, e);
                break;
              }
            }

            // Parse message
            let envelope: MessageEnvelope = match serde_json::from_str(&line) {
              Ok(msg) => msg,
              Err(e) => {
                log::warn!("Failed to parse message from node {}: {}", node_id, e);
                continue;
              }
            };

            // Handle different message types
            let msg = &envelope.message;

            // NodeDrop
            if msg.NodeDrop.is_some() {
              log::info!("Node {} requested disconnect", node_id);
              break;
            }

            // NodeStatusUpdate
            if let Some(ref status) = msg.NodeStatusUpdate {
              log::debug!("Node {} status: {:?}", node_id, status.status);
              if let Some(node) = nodes_clone.write().await.get_mut(&node_id) {
                node.status = status.status.clone();
              }
            }

            // NodeEvent
            if let Some(ref event) = msg.NodeEvent {
              log::info!("Node {} event: {} - {:?}", node_id, event.event_type, event.message);
            }

            // HoneypotStatusUpdate
            if let Some(ref hp_status) = msg.HoneypotStatusUpdate {
              log::info!(
                "🍯 Honeypot {} on node {}: {:?} - {}",
                hp_status.honeypot_id,
                node_id,
                hp_status.status,
                hp_status.message.as_deref().unwrap_or("")
              );
            }

            // HoneypotEvent (attack data!)
            if let Some(ref hp_event) = msg.HoneypotEvent {
              Self::handle_honeypot_event(node_id, hp_event);
            }
          }

          // Clean up
          if nodes_clone.write().await.remove(&node_id).is_some() {
            log::info!("Removed node {} from registry", node_id);
          }
          tasks_clone.write().await.remove(&node_id);
          writers_clone.write().await.remove(&node_id);
        });

        tasks.write().await.insert(node_id, handle);
      });
    }
  }

  /// Handle honeypot events (attacks, logins, commands, etc.)
  fn handle_honeypot_event(node_id: u64, event: &HoneypotEvent) {
    let severity = if event.eventid.contains("login.success") 
        || event.eventid.contains("command.input")
        || event.eventid.contains("file_download") {
      "🔴 HIGH"
    } else if event.eventid.contains("login.failed") {
      "🟡 MED"
    } else {
      "🟢 LOW"
    };

    log::info!(
      "{} [{}] {} | {} | src={}:{} | user={} pass={}",
      severity,
      event.honeypot_id,
      event.eventid,
      event.message.as_deref().unwrap_or(""),
      event.src_ip.as_deref().unwrap_or("?"),
      event.src_port.unwrap_or(0),
      event.username.as_deref().unwrap_or("-"),
      event.password.as_deref().unwrap_or("-")
    );

    // Log command input separately
    if let Some(ref input) = event.input {
      log::info!("  └─ Command: {}", input);
    }
  }

  pub async fn get_nodes(&self) -> Arc<RwLock<HashMap<u64, Node>>> {
    Arc::clone(&self.nodes)
  }

  pub async fn active_connections(&self) -> usize {
    self.tasks.read().await.len()
  }
}