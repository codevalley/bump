# Bump Service

A fast and lightweight proximity-based data exchange service that enables data transfer between devices through a simple "bump" gesture.

## Features
- Simple API with just two endpoints: `/bump/send` and `/bump/receive`
- Stateless, ephemeral architecture with no persistent storage
- Matching based on location, timestamp, and optional custom key
- Support for text, JSON, or URL payloads
- Synchronous request model with configurable timeouts

## Prerequisites
- Rust toolchain (install via [rustup](https://rustup.rs/))
- Cargo (Rust's package manager, comes with rustup)

## Setup
1. Install Rust and Cargo using rustup:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. Clone the repository and build:
```bash
cargo build
```

3. Run the service:
```bash
cargo run
```

The service will start on `localhost:8080`.

## API Usage

### Send Endpoint
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

### Receive Endpoint
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

## Project Structure
- `src/main.rs` - Application entry point and server setup
- `src/api.rs` - API endpoint handlers
- `src/models.rs` - Data structures and types
- `src/service.rs` - Core matching service implementation
- `src/error.rs` - Error types and handling

## Development
- Run tests: `cargo test`
- Run with logging: `RUST_LOG=debug cargo run`
- Format code: `cargo fmt`
- Check lints: `cargo clippy`
