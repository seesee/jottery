# -----------------------------
# Stage 1: Build the Svelte web client
# -----------------------------
FROM node:20-slim AS web-builder
WORKDIR /app/web

# Only copy web client deps first for caching
COPY web/package.json web/package-lock.json ./
RUN npm install

# Copy rest of web client source
COPY web/ .
RUN npm run build


# -----------------------------
# Stage 2: Build the Rust server
# -----------------------------
FROM rust:1.85 AS server-builder
WORKDIR /app/server

# Copy server source code
COPY server/ .

# Build server binary
RUN cargo build --release --bin jottery-server


# -----------------------------
# Stage 3: Final runtime image
# -----------------------------
FROM debian:bookworm-slim
WORKDIR /app

# Install runtime dependencies only
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# -----------------------------
# Copy server + web client
# -----------------------------
COPY --from=server-builder /app/server/target/release/jottery-server /usr/local/bin/jottery-server
COPY --from=web-builder    /app/web/dist                         ./dist

# -----------------------------
# Copy TUI binaries (added by GitHub Actions in /releases)
# -----------------------------
COPY releases ./releases

# -----------------------------
# Persistent data directory for DB + attachments
# -----------------------------
RUN mkdir -p /app/data
VOLUME ["/app/data"]

# -----------------------------
# Runtime configuration
# -----------------------------
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=3030
ENV DATABASE_URL=/app/data/jottery.db

# Expose the sync server's internal port
EXPOSE 3030

CMD ["/usr/local/bin/jottery-server"]
