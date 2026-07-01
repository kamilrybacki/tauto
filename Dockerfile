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

# Stage 3: download and warm up the Lean toolchain.
# Running `lake --version` forces elan to download the stable toolchain into
# this layer so the runtime image never fetches anything at start.
FROM debian:bookworm-slim AS lean-installer
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh \
    | sh -s -- -y --default-toolchain leanprover/lean4:stable \
    && /root/.elan/bin/lake --version

# Stage 4: minimal runtime image
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder   /build/target/release/tauto /usr/local/bin/tauto
COPY --from=ui-builder     /build/ui/dist               /opt/tauto/ui/dist
COPY --from=lean-installer /root/.elan                  /root/.elan

ENV PATH="/root/.elan/bin:$PATH"

EXPOSE 4000

ENTRYPOINT ["tauto"]
CMD ["serve", "/contracts", "--port", "4000", "--ui-dist", "/opt/tauto/ui/dist"]
