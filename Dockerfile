# Build stage
FROM rust:1.70-slim as builder

WORKDIR /usr/src/bump
COPY . .

# Create a script to handle dependencies for Rust 1.70
RUN echo '#!/bin/bash\n\
# Update dependencies to be compatible with Rust 1.70\n\
cargo update -p bytestring@1.4.0 --precise 1.3.0\n\
cargo update -p geo-types@0.7.15 --precise 0.7.11\n\
cargo update -p is-terminal@0.4.15 --precise 0.4.7\n\
cargo update -p dashmap@5.5.3 --precise 5.4.0\n\
\n\
# Handle any additional dependency issues that may appear during the build\n\
if ! cargo check --quiet; then\n\
  echo "Initial dependency resolution failed. Attempting to fix..."\n\
  # Parse error messages to downgrade problematic packages\n\
  cargo check 2>&1 | grep -oE "package .* cannot be built because" | grep -oE "`.*`" | tr -d "`" | while read package; do\n\
    version=$(echo $package | cut -d" " -f2)\n\
    name=$(echo $package | cut -d" " -f1)\n\
    echo "Attempting to downgrade $name@$version"\n\
    # Try downgrading to an older version (this is a simplistic approach)\n\
    major=$(echo $version | cut -d"." -f1)\n\
    minor=$(echo $version | cut -d"." -f2)\n\
    patch=$(echo $version | cut -d"." -f3)\n\
    new_patch=$((patch - 1))\n\
    new_version="$major.$minor.$new_patch"\n\
    cargo update -p $name@$version --precise $new_version\n\
  done\n\
fi\n\
\n\
# Final check\n\
cargo check --quiet || echo "Warning: Dependencies still have issues, but continuing build anyway."\n\
' > /usr/src/bump/fix-deps.sh && chmod +x /usr/src/bump/fix-deps.sh

# Run the dependency fixer script
RUN /usr/src/bump/fix-deps.sh

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
