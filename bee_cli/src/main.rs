// /home/hqnw/Desktop/project_4/code/honeybee_core/bee_cli/src/main.rs
use std::error::Error;
use std::io::{
  self,
  BufRead,
  Write,
};

use bee_message::{
  BackendCommand,
  BackendRegistration,
  BackendToManagerMessage,
  BackendType,
  InstallPot,
  ManagerToBackendMessage,
  MessageEnvelope,
  NodeCommand,
  NodeCommandType,
  PROTOCOL_VERSION,
  PotId,
};
use tokio::io::{
  AsyncBufReadExt,
  AsyncWriteExt,
};
use tokio::net::TcpStream;

struct HoneybeeCliClient {
  server_address: String,
}

impl HoneybeeCliClient {
  fn new(server_address: String) -> Self { Self { server_address } }

  async fn connect(&self) -> Result<TcpStream, Box<dyn Error>> {
    println!("Connecting to manager at {}...", self.server_address);
    let mut stream = TcpStream::connect(&self.server_address).await?;
    println!("Connected successfully!");
    println!("Starting registration process...");
    let registration_command = BackendToManagerMessage::BackendRegistration(BackendRegistration {
      backend_id:   1,
      backend_name: "Honeybee CLI Client".to_string(),
      address:      "<address>".to_string(),
      port:         0,
      backend_type: BackendType::Cli,
    });
    self
      .send(&mut stream, registration_command)
      .await?;
    // let response = self.receive_response(&mut stream).await?;
    // println!("Registration completed: {:?}", response);
    println!("Registration completed");

    Ok(stream)
  }
  async fn send(&self, stream: &mut TcpStream, message: BackendToManagerMessage) -> Result<(), Box<dyn Error>> {
    let envelope = MessageEnvelope::new(PROTOCOL_VERSION, message);

    let json = serde_json::to_string(&envelope)?;
    stream.write_all(json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    println!("Message sent");
    Ok(())
  }
  async fn send_command(&self, stream: &mut TcpStream, command: BackendCommand) -> Result<(), Box<dyn Error>> {
    let cmd = BackendToManagerMessage::BackendCommand(command);

    let envelope = MessageEnvelope::new(PROTOCOL_VERSION, cmd);

    let json = serde_json::to_string(&envelope)?;
    stream.write_all(json.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    println!("Command sent");
    let response = self.receive_response(stream).await?;
    println!("Response received: {:?}", response);
    Ok(())
  }

  async fn receive_response(&self, stream: &mut TcpStream) -> Result<ManagerToBackendMessage, Box<dyn Error>> {
    let mut reader = tokio::io::BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;
    println!("Raw response: {}", response_line.trim());
    let envelope: MessageEnvelope<ManagerToBackendMessage> = serde_json::from_str(&response_line)?;
    Ok(envelope.message)
  }

  async fn run(&self) -> Result<(), Box<dyn Error>> {
    let mut stream = self.connect().await?;

    let stdin = io::stdin();

    for line in stdin.lock().lines() {
      let line = line?;
      let parts: Vec<&str> = line.trim().split_whitespace().collect();
      if parts.is_empty() {
        continue;
      }
      let command = match parts.get(0) {
        Some(&"list") => BackendCommand::GetNodes,
        Some(&"GetInstalledPots") => BackendCommand::NodeCommand {
          node_id: parts
            .get(1)
            .expect("Node ID required")
            .parse()
            .expect("Invalid Node ID"),
          command: NodeCommandType::GetInstalledPots,
        },
        Some(&"DeployPot") => {
          if let Some(pot_id_str) = parts.get(2) {
            match pot_id_str.parse() {
              Ok(pot_id) => BackendCommand::NodeCommand {
                node_id: parts
                  .get(1)
                  .expect("Node ID required")
                  .parse()
                  .expect("Invalid Node ID"),
                command: NodeCommandType::DeployPot(pot_id),
              },
              Err(_) => {
                println!("Invalid Pot ID: {}", pot_id_str);
                continue;
              }
            }
          } else {
            println!("Pot ID required for DeployPot command");
            continue;
          }
        }
        _ => {
          println!("Unknown command or insufficient arguments");
          continue;
        }
      };
      self.send_command(&mut stream, command).await?;
    }

    Ok(())
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let server_address = std::env::var("MANAGER_ADDRESS").unwrap_or_else(|_| "127.0.0.1:9002".to_string());

  let client = HoneybeeCliClient::new(server_address);
  client.run().await
}
