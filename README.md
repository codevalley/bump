# Bump Service

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://github.com/rust-lang/rust/workflows/CI/badge.svg)](https://github.com/rust-lang/rust/actions)

A fast and lightweight proximity-based data exchange service that enables secure data transfer between devices through a simple "bump" gesture. Built with Rust for high performance and reliability.

<p align="center">
  <img src="docs/assets/bump-logo.png" alt="Bump Service Logo" width="200"/>
</p>

## 🚀 Quick Start
```bash
# Clone the repository
git clone https://github.com/yourusername/bump-service.git
cd bump-service

# Build and run
cargo run

# The service will start on localhost:8080
```

## ✨ Features

### Core Features
- 🔄 Simple API with just two endpoints: `/bump/send` and `/bump/receive`
- 🏃 Stateless, ephemeral architecture with no persistent storage
- 📍 Smart matching based on:
  - Geographic proximity
  - Temporal proximity (configurable window)
  - Optional custom keys for exact matching
- 📦 Flexible payload support:
  - Text data
  - JSON structures
  - URLs and URIs
  - Base64 encoded binary data

### Performance & Reliability
- ⚡ High-performance Rust implementation
- 🔒 Thread-safe queue management
- 🎯 Configurable matching parameters
- ⏰ Automatic request cleanup
- 📊 Queue size limits for resource management

### Developer Experience
- 🛠️ Simple configuration via environment variables
- 📝 Comprehensive logging
- 🧪 Extensive test coverage
- 📚 Detailed API documentation

## 🔧 Installation

### Prerequisites
- [Rust](https://www.rust-lang.org/) 1.70 or higher
- [Cargo](https://doc.rust-lang.org/cargo/) (comes with Rust)
- Unix-like operating system (Linux, macOS)

### From Source
1. Install the Rust toolchain:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. Clone and build:
   ```bash
   # Clone the repository
   git clone https://github.com/yourusername/bump-service.git
   cd bump-service

   # Build in release mode
   cargo build --release
   ```

3. Run the service:
   ```bash
   # Run with default configuration
   ./target/release/bump-service

   # Or with custom configuration
   BUMP_MAX_QUEUE_SIZE=2000 RUST_LOG=info ./target/release/bump-service
   ```

### Using Docker
```bash
# Build the image
docker build -t bump .

# Run the container
docker run -p 8080:8080 bump
```

### Deployment Options

#### Option 1: Railway (Docker-based)
You can deploy to [Railway](https://railway.app/) using the provided Dockerfile:

1. Fork this repository to your GitHub account
2. Create a new project on Railway and connect it to your GitHub repository
3. Railway will automatically detect the `Dockerfile.railway` using the `railway.toml` configuration
4. The service will be deployed with health check settings
5. Access your service at the URL provided by Railway

#### Option 2: Linux VM or Droplet (Recommended)
For greater control and reliability, deploy directly to a Linux server:

1. Build the application locally: `cargo build --release`
2. Transfer the binary to your server: 
   ```bash
   scp target/release/bump user@your-server:/path/to/app/
   ```
3. Set up a systemd service:
   ```bash
   # /etc/systemd/system/bump.service
   [Unit]
   Description=Bump proximity data exchange service
   After=network.target
   
   [Service]
   User=bump
   WorkingDirectory=/path/to/app
   ExecStart=/path/to/app/bump
   Restart=on-failure
   
   [Install]
   WantedBy=multi-user.target
   ```
4. Enable and start the service:
   ```bash
   sudo systemctl enable bump
   sudo systemctl start bump
   ```

#### Option 3: Fly.io (Alternative PaaS)
Fly.io typically has better Rust support:

1. Install the Fly CLI: `curl -L https://fly.io/install.sh | sh`
2. Authenticate: `fly auth login`
3. Create a new app: `fly launch`
4. Deploy: `fly deploy`

See the [Fly.io Rust documentation](https://fly.io/docs/languages-and-frameworks/rust/) for details.

## 📚 API Documentation

### Endpoints

#### GET /bump/health
Get service health status and metrics.

```bash
curl -X GET http://localhost:8080/bump/health
```

Example response:
```json
{
  "status": "ok",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "metrics": {
    "send_queue_capacity": 1000,
    "receive_queue_capacity": 1000,
    "cleanup_interval_ms": 1000,
    "max_time_diff_ms": 500,
    "max_distance_meters": 5
  },
  "queue_stats": {
    "send_queue_size": 12,
    "receive_queue_size": 5,
    "matches_count": 256,
    "expired_count": 18,
    "match_rate": 4.27
  }
}
```

**Response Codes:**
- `200 OK`: Service is healthy

#### POST /bump/send
Send data to a matching receive request.

```bash
curl -X POST http://localhost:8080/bump/send \
  -H "Content-Type: application/json" \
  -d '{
    "matchingData": {
      "location": {"lat": 37.7749, "long": -122.4194},
      "timestamp": 1646078423000,
      "customKey": "optional-key"
    },
    "payload": "https://example.com/shared-document",
    "ttl": 500
  }'
```

**Response Codes:**
- `200 OK`: Successfully matched with a receive request
- `408 Request Timeout`: No match found within TTL
- `429 Too Many Requests`: Send queue is full

#### POST /bump/receive
Receive data from a matching send request.

```bash
curl -X POST http://localhost:8080/bump/receive \
  -H "Content-Type: application/json" \
  -d '{
    "matchingData": {
      "location": {"lat": 37.7749, "long": -122.4194},
      "timestamp": 1646078424000,
      "customKey": "optional-key"
    },
    "ttl": 500
  }'
```

**Response Codes:**
- `200 OK`: Successfully matched with a send request
- `408 Request Timeout`: No match found within TTL
- `429 Too Many Requests`: Receive queue is full

### Configuration
All parameters can be configured via environment variables:

| Variable | Description | Default |
|----------|-------------|----------|
| `BUMP_MAX_QUEUE_SIZE` | Maximum requests per queue | 1000 |
| `BUMP_MAX_DISTANCE_METERS` | Maximum matching distance | 5.0 |
| `BUMP_MAX_TIME_DIFF_MS` | Maximum time difference | 500 |
| `BUMP_DEFAULT_TTL_MS` | Default request TTL | 500 |
| `RUST_LOG` | Log level (error,warn,info,debug,trace) | info |

## 🏗️ Project Structure

```
bump-service/
├── src/
│   ├── main.rs       # Application entry point and server setup
│   ├── api.rs        # HTTP endpoint handlers
│   ├── models.rs     # Data structures and types
│   ├── service.rs    # Core matching service implementation
│   ├── error.rs      # Error types and handling
│   └── config.rs     # Configuration management
├── tests/            # Integration tests
├── docs/            # Documentation and assets
├── Cargo.toml       # Rust package manifest
├── Dockerfile       # Container definition
└── README.md        # This file
```

## 👩‍💻 Development

### Setup Development Environment
```bash
# Install development tools
cargo install cargo-watch cargo-audit cargo-outdated

# Start service with hot reload
cargo watch -x run
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with logging
RUST_LOG=debug cargo test
```

### Code Quality
```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Check for security vulnerabilities
cargo audit

# Check for outdated dependencies
cargo outdated
```

### Performance Testing
```bash
# Run benchmarks
cargo bench

# Profile with flamegraph
cargo flamegraph
```

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md) for details on how to:
- Report bugs
- Suggest enhancements
- Submit pull requests

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Actix Web](https://actix.rs/) - The high-performance web framework
- [Tokio](https://tokio.rs/) - The async runtime that powers our service
- All our [contributors](https://github.com/yourusername/bump-service/graphs/contributors)

## 📞 Contact

For questions and support:
- 📧 Email: codevalley@live.com
- 💬 Discord: [Join our server](https://discord.gg/yourinvite)
- 🐦 Twitter: [@BumpService](https://twitter.com/bumpserviceeeee)
