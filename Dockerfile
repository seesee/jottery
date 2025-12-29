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
# Stage 2: Build the Admin Dashboard
# -----------------------------
FROM node:20-slim AS admin-builder
WORKDIR /app/admin

# Copy admin dashboard deps first for caching
COPY admin/package.json admin/package-lock.json ./
RUN npm install

# Copy rest of admin dashboard source
COPY admin/src ./src
COPY admin/index.html .
COPY admin/svelte.config.js .
COPY admin/tailwind.config.js .
COPY admin/postcss.config.js .
COPY admin/vite.config.ts .

RUN npm run build

# -----------------------------
# Stage 3: Build the Rust server (using cargo-chef for dependency caching)
# -----------------------------
FROM rust:1.85 AS chef
WORKDIR /app
RUN cargo install cargo-chef

FROM chef AS planner
WORKDIR /app/server
COPY server/ .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS server-builder
WORKDIR /app/server

# Tell SQLx to run in offline mode during compile
ENV SQLX_OFFLINE=true

# Copy the recipe and build dependencies (this layer will be cached)
COPY --from=planner /app/server/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Now copy the actual source code and build
COPY server/ .
RUN cargo build --release --bin jottery-server

# -----------------------------
# Stage 4: Final unified runtime image (Caddy + Server)
# -----------------------------
FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates \
      caddy \
    && rm -rf /var/lib/apt/lists/*

# -----------------------------
# Copy server + web client + admin dashboard
# -----------------------------
COPY --from=server-builder /app/server/target/release/jottery-server /usr/local/bin/jottery-server
COPY --from=web-builder    /app/dist ./dist
COPY --from=admin-builder  /app/admin/dist ./admin/dist

# -----------------------------
# Copy TUI binaries (added by GitHub Actions in /releases)
# -----------------------------
COPY releases ./releases
RUN chmod -R +x /app/releases || true

# -----------------------------
# Persistent data directory
# -----------------------------
RUN mkdir -p /app/data
VOLUME ["/app/data"]

# -----------------------------
# Embedded Caddyfile (no external mounts needed)
# -----------------------------
RUN mkdir -p /etc/caddy

COPY <<'EOF' /etc/caddy/Caddyfile
:8088 {
    # Admin dashboard
    handle_path /admin* {
        root * /app/admin/dist
        try_files {path} /index.html
        file_server
    }

    # API proxy to jottery-server
    handle_path /api/* {
        reverse_proxy localhost:3030
    }

    # Web UI (default)
    root * /app/dist
    file_server

    @notStatic {
        not path /api*
        not path /admin*
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
# Start both jottery-server & Caddy
# -----------------------------
CMD ["/bin/sh", "-c", "mkdir -p /app/data && touch /app/data/jottery.db && jottery-server & exec caddy run --config /etc/caddy/Caddyfile --adapter caddyfile"]
