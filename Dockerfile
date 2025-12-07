# -----------------------------
# Stage 1: Build the Svelte web client
# -----------------------------
FROM node:20-slim AS web-builder
WORKDIR /app

# Only copy web client deps first for caching
COPY package.json package-lock.json ./
RUN npm install

# Copy rest of web client source
COPY src ./src
COPY public ./public
COPY index.html .
COPY tsconfig.json .
COPY svelte.config.js .
COPY tailwind.config.js .
COPY postcss.config.js .
COPY tsconfig.node.json .
COPY vite.config.ts .

RUN npm run build

# -----------------------------
# Stage 2: Build the Rust server
# -----------------------------
FROM rust:1.85 AS server-builder
WORKDIR /app/server

# Tell SQLx to run in offline mode during compile
ENV SQLX_OFFLINE=true

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
COPY --from=web-builder    /app/dist ./dist

# -----------------------------
# Copy TUI binaries (added by GitHub Actions in /releases)
# -----------------------------
COPY releases ./releases
RUN chmod -R +x /app/releases || true

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
