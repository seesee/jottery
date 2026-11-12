# Base images are pinned by digest, so a retagged image can't change a release and a
# tag can be rebuilt identically. Dependabot's docker updates keep them current.

# -----------------------------
# Stage 1: Build the Svelte web client
# -----------------------------
FROM node:20-slim@sha256:2cf067cfed83d5ea958367df9f966191a942351a2df77d6f0193e162b5febfc0 AS web-builder
WORKDIR /app

# Registry fetches are slow under QEMU emulation (arm builds) and intermittently
# time out; raise npm's fetch timeouts/retries and retry the install once.
ENV npm_config_fetch_retries=5 \
    npm_config_fetch_retry_mintimeout=20000 \
    npm_config_fetch_retry_maxtimeout=120000 \
    npm_config_fetch_timeout=600000

# Only copy web client deps first for caching
COPY package.json package-lock.json ./
# Lifecycle scripts stay off: the build doesn't need them, and they would run
# third-party code with access to everything in this stage.
RUN npm ci --ignore-scripts || npm ci --ignore-scripts

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
FROM node:20-slim@sha256:2cf067cfed83d5ea958367df9f966191a942351a2df77d6f0193e162b5febfc0 AS admin-builder
WORKDIR /app/admin

# Same npm network hardening as the web-builder stage (see above)
ENV npm_config_fetch_retries=5 \
    npm_config_fetch_retry_mintimeout=20000 \
    npm_config_fetch_retry_maxtimeout=120000 \
    npm_config_fetch_timeout=600000

# Copy admin dashboard deps first for caching
COPY admin/package.json admin/package-lock.json ./
RUN npm ci --ignore-scripts || npm ci --ignore-scripts

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
# The version has to match server/rust-toolchain.toml, so that the tests run on the
# compiler that builds the release. scripts/check-rust-toolchain.sh checks it.
# bookworm, to match the runtime image below: a newer distribution's glibc would stop
# the binaries starting there.
FROM rust:1.99.1-bookworm@sha256:e6f2cff35d0f309acf8433b4f05878ed77092602c5065cc81801177da0bb3a9c AS chef
WORKDIR /app
# Pinned, with its lock file, so that a release can be rebuilt
RUN cargo install cargo-chef --locked --version 0.1.75

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
RUN cargo chef cook --release --locked --recipe-path recipe.json

# Now copy the actual source code and build
COPY server/ .
RUN cargo build --release --locked --bin jottery-server --bin jottery-admin

# -----------------------------
# Stage 4: Caddy, the only component that faces the internet
# -----------------------------
# Taken from the official image because Debian's package is frozen at 2.6.2 from
# 2022. The binary is static, so it runs on bookworm-slim on all three platforms.
FROM caddy:2.11.9-alpine@sha256:0bbb055bda6793c999bde78b1b5ce3e89f829fb031af2cf50056ea1246559fee AS caddy
# Copying the contents leaves behind the official binary's file capability, which
# it only needs to bind low ports. This image listens on 8088.
RUN cat /usr/bin/caddy > /tmp/caddy && chmod 0755 /tmp/caddy

# -----------------------------
# Stage 5: Final unified runtime image (Caddy + Server)
# -----------------------------
FROM debian:bookworm-slim@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Both services run as this user. The id is fixed so that a volume's ownership
# survives an image rebuild, and scripts/docker-entrypoint.sh has to match it.
RUN groupadd --gid 10001 jottery \
    && useradd --uid 10001 --gid 10001 --no-create-home --home-dir /app/data \
         --shell /usr/sbin/nologin jottery

# -----------------------------
# Copy server + web client + admin dashboard
# -----------------------------
COPY --from=caddy          /tmp/caddy /usr/bin/caddy
COPY --from=server-builder /app/server/target/release/jottery-server /usr/local/bin/jottery-server
# The supported way back in if the dashboard can't be used: see "Recovering access" in the README
COPY --from=server-builder /app/server/target/release/jottery-admin /usr/local/bin/jottery-admin
COPY --from=web-builder    /app/dist ./dist
COPY --from=admin-builder  /app/admin/dist ./admin/dist

# -----------------------------
# Persistent data directory
# -----------------------------
# Everything else stays owned by root, so the services can't rewrite the web bundle
# that every client loads. This comes before VOLUME so a new volume inherits it.
RUN mkdir -p /app/data && chown jottery:jottery /app/data
VOLUME ["/app/data"]

# -----------------------------
# Caddyfile
# -----------------------------
RUN mkdir -p /etc/caddy
COPY Caddyfile /etc/caddy/Caddyfile

# -----------------------------
# Runtime environment
# -----------------------------
ENV DATABASE_URL=/app/data/jottery.db
# The bundled Caddy is the one proxy in front of the server. It replaces any
# X-Forwarded-For a client sends, and port 3030 is never published.
ENV TRUSTED_PROXY_HOPS=1
# Caddy needs somewhere writable. It keeps nothing there worth persisting: this image
# doesn't terminate TLS, and the Caddyfile is the whole configuration.
ENV XDG_CONFIG_HOME=/tmp/caddy/config
ENV XDG_DATA_HOME=/tmp/caddy/data

# Expose only the Caddy HTTP port
EXPOSE 8088

# -----------------------------
# Start both jottery-server & Caddy
# -----------------------------
COPY scripts/docker-entrypoint.sh /usr/local/bin/docker-entrypoint
COPY scripts/docker-healthcheck.sh /usr/local/bin/docker-healthcheck

# The start period allows for database migrations after an upgrade
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
  CMD ["/usr/local/bin/docker-healthcheck"]

# No USER here: the entrypoint starts as root only to take ownership of volumes
# written by earlier images, then drops to the jottery user before anything runs.
ENTRYPOINT ["/usr/local/bin/docker-entrypoint"]
