# Stage 1: Build with Rust nightly
FROM rustlang/rust:nightly-alpine AS builder

RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static

WORKDIR /app
COPY . .
RUN cargo build --release

# Stage 2: Runtime
FROM alpine:3.19
RUN apk add --no-cache ca-certificates
WORKDIR /app
RUN mkdir -p /app/logs
COPY --from=builder /app/target/release/honeybee_core /app/honeybee_core
COPY --from=builder /app/bee_config.toml /app/bee_config.toml
EXPOSE 9001
CMD ["./honeybee_core"]
