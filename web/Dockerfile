# Use a Rust image to build the backend
FROM rust:1.84.0 AS builder
WORKDIR /usr/src/app

# Clone the backend repository. When cloned from the repository, cargo build results in error with main file data_sources handler.
#RUN git clone https://github.com/gunalmel/mteam-dashboard-backend.git /usr/src/app
COPY . .

# Clone the action processor repository
RUN git clone https://github.com/gunalmel/mteam-dashboard-action-processor.git /usr/src/mteam-dashboard-action-processor

# Update the path dependency in Cargo.toml
RUN sed -i 's|path = "../mteam-dashboard-action-processor"|path = "/usr/src/mteam-dashboard-action-processor"|' Cargo.toml

RUN cargo fetch

# Build the backend binary
RUN cargo build --release

# Create the runtime image
FROM debian:bookworm-slim
WORKDIR /app

# Install OpenSSL and other necessary dependencies
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary
COPY --from=builder /usr/src/app/target/release/mteam-dashboard-backend /app/
COPY ../plot-config /app/plot-config
COPY ../config.json /app/config.json

# Expose the backend port
EXPOSE 8080

# Run the application
CMD ["/app/mteam-dashboard-backend"]
