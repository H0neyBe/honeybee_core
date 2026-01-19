#![allow(dead_code, unused_imports, unused_variables)]
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
      backend_name: "Honeybee CLI Client".to_string(),
      backend_type: BackendType::Cli,
    });
    self
      .send(&mut stream, registration_command)
      .await?;
    let response = self.receive_response(&mut stream).await?;
    println!("Registration completed: {:?}", response);
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
        Some(&"InstallPot") => {
          // InstallPot <node_id> <pot_id> <honeypot_type>
          if parts.len() < 4 {
            println!("Usage: InstallPot <node_id> <pot_id> <honeypot_type>");
            println!("Example: InstallPot 8108114413124017714 cowrie-01 cowrie");
            continue;
          }
          
          let node_id = match parts.get(1).unwrap().parse() {
            Ok(id) => id,
            Err(_) => {
              println!("Invalid Node ID");
              continue;
            }
          };
          
          let pot_id = PotId(parts.get(2).unwrap().to_string());
          let honeypot_type = parts.get(3).unwrap().to_string();
          
          BackendCommand::NodeCommand {
            node_id,
            command: NodeCommandType::InstallPot(InstallPot {
              pot_id,
              honeypot_type,
              git_url: None,
              git_branch: None,
              config: None,
              auto_start: true,
            }),
          }
        }
        Some(&"DeployPot") => {
          // DeployPot <node_id> <pot_id>
          if parts.len() < 3 {
            println!("Usage: DeployPot <node_id> <pot_id>");
            println!("Example: DeployPot 8108114413124017714 cowrie-01");
            continue;
          }
          
          let node_id = match parts.get(1).unwrap().parse() {
            Ok(id) => id,
            Err(_) => {
              println!("Invalid Node ID");
              continue;
            }
          };
          
          let pot_id = PotId(parts.get(2).unwrap().to_string());
          
          BackendCommand::NodeCommand {
            node_id,
            command: NodeCommandType::DeployPot(pot_id),
          }
        }
        Some(&"StopPot") => {
          // StopPot <node_id> <pot_id>
          if parts.len() < 3 {
            println!("Usage: StopPot <node_id> <pot_id>");
            println!("Example: StopPot 8108114413124017714 cowrie-01");
            continue;
          }
          
          let node_id = match parts.get(1).unwrap().parse() {
            Ok(id) => id,
            Err(_) => {
              println!("Invalid Node ID");
              continue;
            }
          };
          
          let pot_id = PotId(parts.get(2).unwrap().to_string());
          
          BackendCommand::NodeCommand {
            node_id,
            command: NodeCommandType::StopPot(pot_id),
          }
        }
        Some(&"RestartPot") => {
          // RestartPot <node_id> <pot_id>
          if parts.len() < 3 {
            println!("Usage: RestartPot <node_id> <pot_id>");
            println!("Example: RestartPot 8108114413124017714 cowrie-01");
            continue;
          }
          
          let node_id = match parts.get(1).unwrap().parse() {
            Ok(id) => id,
            Err(_) => {
              println!("Invalid Node ID");
              continue;
            }
          };
          
          let pot_id = PotId(parts.get(2).unwrap().to_string());
          
          BackendCommand::NodeCommand {
            node_id,
            command: NodeCommandType::RestartPot(pot_id),
          }
        }
        Some(&"GetPotStatus") => {
          // GetPotStatus <node_id> <pot_id>
          if parts.len() < 3 {
            println!("Usage: GetPotStatus <node_id> <pot_id>");
            println!("Example: GetPotStatus 8108114413124017714 cowrie-01");
            continue;
          }
          
          let node_id = match parts.get(1).unwrap().parse() {
            Ok(id) => id,
            Err(_) => {
              println!("Invalid Node ID");
              continue;
            }
          };
          
          let pot_id = PotId(parts.get(2).unwrap().to_string());
          
          BackendCommand::NodeCommand {
            node_id,
            command: NodeCommandType::GetPotStatus(pot_id),
          }
        }
        Some(&"help") | Some(&"?") => {
          println!("\n=== HoneyBee CLI Commands ===\n");
          println!("Node Management:");
          println!("  list                                    - List all connected nodes");
          println!("  GetInstalledPots <node_id>              - Get installed honeypots on a node");
          println!();
          println!("Honeypot Management:");
          println!("  InstallPot <node_id> <pot_id> <type>   - Install a honeypot");
          println!("  DeployPot <node_id> <pot_id>           - Deploy/start a honeypot");
          println!("  StopPot <node_id> <pot_id>             - Stop a running honeypot");
          println!("  RestartPot <node_id> <pot_id>          - Restart a honeypot");
          println!("  GetPotStatus <node_id> <pot_id>        - Get honeypot status");
          println!();
          println!("Available honeypot types:");
          println!("  cowrie       - SSH/Telnet honeypot");
          println!("  honnypotter  - WordPress login honeypot");
          println!();
          println!("Examples:");
          println!("  list");
          println!("  InstallPot 8108114413124017714 cowrie-01 cowrie");
          println!("  DeployPot 8108114413124017714 cowrie-01");
          println!("  GetInstalledPots 8108114413124017714");
          println!("  StopPot 8108114413124017714 cowrie-01\n");
          continue;
        }
        _ => {
          println!("Unknown command. Type 'help' for available commands.");
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
