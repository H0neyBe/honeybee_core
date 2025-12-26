# Stage 1: Build with latest Rust nightly
FROM alpine:3.19 AS builder

# Install build dependencies
RUN apk add --no-cache curl gcc musl-dev pkgconfig openssl-dev openssl-libs-static

# Install rustup and nightly toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app
COPY . .
RUN cargo build --release

# Stage 2: Runtime
FROM alpine:3.19
RUN apk add --no-cache ca-certificates netcat-openbsd
WORKDIR /app
RUN mkdir -p /app/logs
COPY --from=builder /app/target/release/honeybee_core /app/honeybee_core
# Copy config file (will be overridden by volume mount in docker-compose if needed)
COPY --from=builder /app/bee_config.toml /app/bee_config.toml
EXPOSE 9001 9002 9003
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD nc -z localhost 9001 || exit 1
CMD ["./honeybee_core"]
