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
# Stage 3: Final unified runtime image (Caddy + Server)
# -----------------------------
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates \
      caddy \
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
# Create non-root user & prepare data dir
# -----------------------------
RUN groupadd -r jottery && useradd -r -g jottery jottery

RUN mkdir -p /app/data && chown -R jottery:jottery /app

VOLUME ["/app/data"]

# -----------------------------
# Embedded Caddyfile (no external mounts needed)
# -----------------------------
RUN mkdir -p /etc/caddy

COPY <<'EOF' /etc/caddy/Caddyfile
:8088 {
    root * /app/dist
    file_server

    # Strip /api prefix and reverse proxy to jottery-server
    handle_path /api/* {
        reverse_proxy localhost:3030
    }

    @notStatic {
        not path /api*
        not file
    }
    rewrite @notStatic /index.html
}
EOF

# -----------------------------
# Runtime environment
# -----------------------------
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=3030
ENV DATABASE_URL=/app/data/jottery.db

# Expose only the Caddy HTTP port
EXPOSE 8088

# -----------------------------
# Switch to jottery user
# -----------------------------
RUN mkdir -p /home/jottery && chown -R jottery:jottery /home/jottery
ENV HOME=/home/jottery

USER jottery

# -----------------------------
# Start both jottery-server & Caddy (ensure DB exists as jottery)
# -----------------------------
CMD ["/bin/sh", "-c", "mkdir -p /app/data && chown -R jottery:jottery /app/data && touch /app/data/jottery.db && jottery-server & exec caddy run --config /etc/caddy/Caddyfile --adapter caddyfile"]
