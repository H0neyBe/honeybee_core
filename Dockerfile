# Stage 1: Build the Rust application
FROM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static

WORKDIR /app

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./
COPY bee_config/Cargo.toml ./bee_config/
COPY bee_message/Cargo.toml ./bee_message/

# Create dummy src files for dependency caching
RUN mkdir -p src bee_config/src bee_message/src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn dummy() {}" > bee_config/src/lib.rs && \
    echo "pub fn dummy() {}" > bee_message/src/lib.rs

# Build dependencies only (ignore errors from dummy files)
RUN cargo build --release 2>/dev/null || true

# Copy actual source code
COPY src ./src
COPY bee_config ./bee_config
COPY bee_message ./bee_message

# Touch to invalidate cache and rebuild
RUN touch src/main.rs bee_config/src/lib.rs bee_message/src/lib.rs

# Build the application
RUN cargo build --release

# Stage 2: Create minimal runtime image
FROM alpine:3.19

RUN apk add --no-cache ca-certificates

WORKDIR /app
RUN mkdir -p /app/logs

# Copy the binary
COPY --from=builder /app/target/release/honeybee_core /app/honeybee_core

# Copy default config
COPY bee_config.toml /app/bee_config.toml

EXPOSE 9001

CMD ["./honeybee_core"]

