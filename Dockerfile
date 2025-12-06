# Dockerfile

# Stage 1: Build the Svelte web client
FROM node:20-slim as web-builder
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm install
COPY . .
RUN npm run build

# Stage 2: Build the Rust server
FROM rust:1.78 as server-builder
WORKDIR /app
COPY server/ ./server/
WORKDIR /app/server
# Create a dummy file for the database to be present during the build
RUN mkdir -p data
RUN touch data/jottery.db
RUN cargo build --release

# Stage 3: Create the final image
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=server-builder /app/server/target/release/jottery-server /usr/local/bin/
COPY --from=web-builder /app/dist ./dist
ENV ROCKET_ADDRESS=0.0.0.0
ENV DATABASE_URL=data/jottery.db
EXPOSE 8000
CMD ["/usr/local/bin/jottery-server"]
