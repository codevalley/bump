# Build stage
FROM rust:1.70-slim as builder

WORKDIR /usr/src/bump
COPY . .

# Downgrade dependencies to versions compatible with Rust 1.70
RUN cargo update -p bytestring@1.4.0 --precise 1.3.0 && \
    cargo update -p geo-types@0.7.15 --precise 0.7.11

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install necessary runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /usr/src/bump/target/release/bump /usr/local/bin/

# Set environment variables
ENV RUST_LOG=info
ENV PORT=8080
ENV BUMP_MAX_QUEUE_SIZE=1000
ENV BUMP_MAX_DISTANCE_METERS=5.0
ENV BUMP_MAX_TIME_DIFF_MS=500
ENV BUMP_DEFAULT_TTL_MS=500
ENV BUMP_CLEANUP_INTERVAL_MS=1000
ENV BUMP_TEMPORAL_WEIGHT=0.7
ENV BUMP_SPATIAL_WEIGHT=0.3
ENV BUMP_EARTH_RADIUS_METERS=6371000.0

# Expose the service port
EXPOSE 8080

# Run the service
CMD ["bump"]
