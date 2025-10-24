# Honeybee Core

A Honeypot orchestration framework written in Rust.

## Overview

Honeybee Core is a distributed honeypot management system that allows you to orchestrate and monitor multiple honeypot nodes from a central manager. The system uses TCP for communication between nodes and the manager, with JSON-based message passing.

## Architecture

The project consists of three main components:

- **honeybee_core**: The main orchestration server that manages honeypot nodes
- **bee_config**: Configuration management library
- **bee_message**: Message protocol definitions for node communication

### Key Components

- **NodeManager** ([src/node_manager/manager.rs](src/node_manager/manager.rs)): Handles node registration, monitoring, and communication
- **Node** ([src/node_manager/node.rs](src/node_manager/node.rs)): Represents individual honeypot nodes
- **Config** ([bee_config/src/config.rs](bee_config/src/config.rs)): Configuration management
- **MessageTypes** ([bee_message/src/node.rs](bee_message/src/node.rs)): Protocol definitions for node communication

## Features

- TCP-based node registration and communication
- Configurable logging with file and console output
- Async/await runtime using Tokio
- JSON-based message protocol
- Status tracking for deployed nodes

## Configuration

Configuration is stored in `bee_config.toml`:

```toml
[server]
host = "127.0.0.1"
port = 9001

[database]
url = "postgres://localhost/mydb"

[logging]
level = "debug"
file = "logs/debug.log"
```

## Building

```sh
cargo build
```

## Running

```sh
cargo run
```

The server will start listening on the configured address (default: 127.0.0.1:9001).

## Message Protocol

Nodes communicate with the manager using JSON messages wrapped in a [`MessageEnvelope`](bee_message/src/node.rs):

- **NodeRegistration**: Register a new node with the manager
- **NodeStatusUpdate**: Update the status of an existing node
- **NodeCommand**: Send commands to nodes

## Node Types

- **Full**: Full-featured honeypot node
- **Agent**: Lightweight monitoring agent

## Node Status

- **Deploying**: Node is being deployed
- **Running**: Node is active and running
- **Stopped**: Node has been stopped
- **Failed**: Node encountered an error
- **Unknown**: Status cannot be determined

## Development

The project uses workspace dependencies for shared crates like `serde` and `serde_json`. Additional dependencies include:

- tokio: Async runtime
- log/fern: Logging framework
- colored: Terminal output colors
- chrono: Timestamp handling
