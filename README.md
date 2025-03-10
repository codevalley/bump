# Bump Service

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://github.com/rust-lang/rust/workflows/CI/badge.svg)](https://github.com/rust-lang/rust/actions)

A fast, lightweight, and ephemeral proximity-based data exchange service that enables secure data transfer between devices through a simple "bump" gesture. Built with Rust for high performance and reliability.

<p align="center">
  <img src="docs/assets/bump-logo.png" alt="Bump Service Logo" width="200"/>
</p>

## What is Bump?

Bump is a modern solution to a common problem: how do you quickly share data between devices that are physically next to each other? While technologies like AirDrop, NFC, or QR codes exist, they often require specific hardware, complex setup, or are platform-dependent.

Bump makes sharing as intuitive as physically bumping two devices together:
1. Two users with devices next to each other want to share data
2. They both make a "bump" gesture with their devices
3. Our service matches these gestures based on time and location
4. Data is instantly exchanged between the devices

### Why Bump?

- 🚀 **Platform Independent**: Works on any device with a web browser
- 🎯 **No Special Hardware**: Uses standard sensors (GPS, accelerometer) available in most devices
- 🔒 **Privacy-First**: Ephemeral by design - no data storage, no tracking
- ⚡ **Lightning Fast**: Sub-second data exchange
- 🌐 **Universal**: Share any type of data - text, URLs, JSON, or encoded binary

## How Does It Work?

### The Bump Protocol

```
Device A                 Server                  Device B
   |                       |                       |
   |------ /bump -------->|                       |
   |                      |<------ /bump ---------|
   |                      |                       |
   |                      |---(match algorithm)---|
   |                      |                       |
   |<---- payload B ------|                       |
   |                      |------ payload A ----->|
   |                      |                       |
```

1. When a device initiates a bump, it sends a request to our `/bump` endpoint with:
   - Current location
   - Timestamp
   - Optional payload
   - Optional custom matching key

2. Our service uses a sophisticated matching algorithm to pair devices based on:
   - Temporal proximity (within milliseconds)
   - Spatial proximity (within meters)
   - Custom keys (if provided)

3. When a match is found, payloads are exchanged instantly between the devices

### Ephemeral, Fast, and Efficient

Our service is designed with performance and privacy in mind:

- **Zero Persistence**: No databases, no storage, everything happens in memory
- **Auto-Cleanup**: Unmatched requests automatically expire after their TTL
- **Race-Condition Protected**: Thread-safe queue management
- **Resource Efficient**: 
  - Configurable queue sizes
  - Automatic request cleanup
  - Minimal memory footprint
  - Sub-millisecond matching algorithm

## Try It Out!

### Live Demo
Visit our live demo at [bump.nyn.me](https://bump.nyn.me) to try Bump in action! Make suer you have two devices to test it with :-)

### Client Implementation
Check out our open-source client implementation at [github.com/codevalley/bump-me](https://github.com/codevalley/bump-me)

## 🚀 Quick Start
```bash
# Clone the repository
git clone https://github.com/yourusername/bump-service.git
cd bump-service

# Build and run
cargo run

# The service will start on localhost:8080
```

## API Documentation

### POST /bump

The unified endpoint for all bump operations:

```bash
curl -X POST http://localhost:8080/bump \
  -H "Content-Type: application/json" \
  -d '{
    "matching_data": {
      "location": {"lat": 37.7749, "long": -122.4194},
      "timestamp": 1646078423000,
      "custom_key": "optional-key"
    },
    "payload": "https://example.com/shared-document",
    "ttl": 500
  }'
```

**Response (Success):**
```json
{
  "status": "matched",
  "matched_with": "request-id-123",
  "timestamp": 1646078425000,
  "payload": "payload from matching request",
  "message": "Match successful"
}
```

For detailed API documentation, see our [API Guide](docs/api-guide.md).

## Configuration

Configure the service through environment variables:

| Variable | Description | Default |
|----------|-------------|----------|
| `BUMP_MAX_QUEUE_SIZE` | Maximum pending requests | 1000 |
| `BUMP_MAX_DISTANCE_METERS` | Maximum matching distance | 5.0 |
| `BUMP_MAX_TIME_DIFF_MS` | Maximum time difference | 500 |
| `BUMP_DEFAULT_TTL_MS` | Default request TTL | 500 |
| `RUST_LOG` | Log level | info |

## Project Structure

```
bump-service/
├── src/
│   ├── main.rs       # Application entry point
│   ├── api.rs        # HTTP endpoint handlers
│   ├── models.rs     # Data structures
│   ├── service.rs    # Core matching service
│   ├── queue.rs      # Thread-safe queue
│   └── config.rs     # Configuration
├── docs/            # Documentation
└── tests/           # Integration tests
```

## Development

```bash
# Start with hot reload
cargo watch -x run

# Run tests
cargo test

# Format and lint
cargo fmt
cargo clippy
```

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🤝 Contributing

We welcome contributions! See our [Contributing Guidelines](CONTRIBUTING.md) for details.

## 📞 Contact

- 📧 Email: codevalley@live.com
