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

# Stage 3 (target: lake-worker) — the Lean build service.
#
# This is the ONLY image that ships the ~800 MB Lean toolchain. It runs the
# generic build service (`tauto lake-worker`) and is deployed as a separate,
# rarely-redeployed workload, so the Lean bloat never touches the web pod's
# rollout. It intentionally does NOT set TAUTO_SKIP_LEAN_CHECK — the startup
# check should fail loudly if this image lacks a working lake.
FROM debian:bookworm-slim AS lake-worker
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh \
    | sh -s -- -y --default-toolchain leanprover/lean4:stable \
    && /root/.elan/bin/lake --version
ENV PATH="/root/.elan/bin:$PATH"
COPY --from=rust-builder /build/target/release/tauto /usr/local/bin/tauto
EXPOSE 4001
ENTRYPOINT ["tauto"]
CMD ["lake-worker", "--port", "4001"]

# Stage 4 (default target: runtime) — minimal web image: Rust binary + built UI.
#
# The Lean toolchain is intentionally NOT shipped here: it added ~800 MB (slow
# pulls, long Recreate rollouts) and running `lake build` in the serving pod
# starved the liveness probe. Compilation now runs in the separate lake-worker
# service (set TAUTO_LAKE_URL); without it the Proofs endpoint degrades
# gracefully (build_available:false) but still shows the sorry-stubbed
# obligations. The real `lake build` gate also runs in CI (lean-verify).
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /build/target/release/tauto /usr/local/bin/tauto
COPY --from=ui-builder   /build/ui/dist              /opt/tauto/ui/dist
# Bundle the capability benchmark so the deployment can seed it as an example
# project (see the seed initContainer in the Helm chart).
COPY examples/benchmark /opt/tauto/benchmark

# No Lean toolchain in this image, so skip the startup lake availability check.
ENV TAUTO_SKIP_LEAN_CHECK=1

EXPOSE 4000

ENTRYPOINT ["tauto"]
CMD ["serve", "/contracts", "--port", "4000", "--ui-dist", "/opt/tauto/ui/dist"]
