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

# Stage 3: minimal runtime image — the Rust binary + built UI only.
#
# The Lean toolchain is intentionally NOT shipped here: it added ~800 MB (slow
# pulls, long Recreate rollouts) and running `lake build` in the serving pod
# starved the liveness probe. The Proofs endpoint degrades gracefully when
# `lake` is absent (build_available:false) — it still generates and displays the
# sorry-stubbed proof obligations in-process. The real `lake build` check runs
# in CI (the lean-verify job), not on the live pod.
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /build/target/release/tauto /usr/local/bin/tauto
COPY --from=ui-builder   /build/ui/dist              /opt/tauto/ui/dist

# No Lean toolchain in this image, so skip the startup lake availability check.
ENV TAUTO_SKIP_LEAN_CHECK=1

EXPOSE 4000

ENTRYPOINT ["tauto"]
CMD ["serve", "/contracts", "--port", "4000", "--ui-dist", "/opt/tauto/ui/dist"]
