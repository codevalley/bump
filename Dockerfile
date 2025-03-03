# Build stage
FROM rust:1.70-slim as builder

WORKDIR /usr/src/bump-service
COPY . .

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install necessary runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /usr/src/bump-service/target/release/bump-service /usr/local/bin/

# Set environment variables
ENV RUST_LOG=info
ENV BUMP_MAX_QUEUE_SIZE=1000
ENV BUMP_MAX_DISTANCE_METERS=5.0
ENV BUMP_MAX_TIME_DIFF_MS=500
ENV BUMP_DEFAULT_TTL_MS=500

# Expose the service port
EXPOSE 8080

# Run the service
CMD ["bump-service"]
