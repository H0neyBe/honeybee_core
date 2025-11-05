# Stage 1: Build the Rust application
FROM rust:alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev

# Set the working directory in the container
WORKDIR /app

# Copy the current directory contents into the container at /app
COPY . .

# Build the Rust application
RUN cargo build --release

# Stage 2: Create a smaller image with the compiled binary
FROM alpine:latest

# Set the working directory in the container
WORKDIR /app

# Copy the compiled binary from the builder stage
COPY --from=builder /app/target/release/honeybee_core /app/honeybee_core

# Copy the configuration file
COPY --from=builder /app/bee_config.toml /app/bee_config.toml

# Make ports 9001 available to the world outside this container
EXPOSE 9001

# Run the compiled binary
CMD ["./honeybee_core"]