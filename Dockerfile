# Stage 1: build the React frontend
FROM node:20-alpine AS ui-builder
WORKDIR /build/ui
COPY ui/package*.json ./
RUN npm ci --prefer-offline
COPY ui/ ./
RUN npm run build

# Stage 2: build the Rust binary
FROM rust:1-slim-bookworm AS rust-builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN cargo build --release

# Stage 3: minimal runtime image
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /build/target/release/tauto /usr/local/bin/tauto
COPY --from=ui-builder   /build/ui/dist              /opt/tauto/ui/dist

# The serve subcommand never calls lake; skip the startup Lean check.
ENV TAUTO_SKIP_LEAN_CHECK=1

EXPOSE 4000

ENTRYPOINT ["tauto"]
CMD ["serve", "/contracts", "--port", "4000", "--ui-dist", "/opt/tauto/ui/dist"]
