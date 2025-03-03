# Simple, clean Dockerfile using the latest Rust version
FROM rust:1.85 as builder

WORKDIR /app
COPY . .

# Build with the latest stable Rust
RUN rustup update stable && \
    cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install only necessary runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

# Copy health check script
COPY healthcheck.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/healthcheck.sh

# Define health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 CMD ["/usr/local/bin/healthcheck.sh"]

# Copy the binary from builder
COPY --from=builder /app/target/release/bump /usr/local/bin/bump

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
