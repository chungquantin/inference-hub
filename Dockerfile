# One image for every backend service of Inference Hub: the API, the worker
# and the one-shot seed import. `IH_SERVICE` picks which one runs.
#
#   docker build -t inference-hub .
#   docker run -e IH_SERVICE=api    -e DATABASE_URL=... -p 4000:4000 inference-hub
#   docker run -e IH_SERVICE=worker -e DATABASE_URL=... inference-hub
#   docker run -e IH_SERVICE=seed   -e DATABASE_URL=... inference-hub
#
# Migrations are compiled into the binaries (sqlx::migrate!), so the image
# only carries the curated data for the seed.

ARG RUST_VERSION=1.98

# --- Dependency planning, so a code change does not rebuild every crate ------
FROM rust:${RUST_VERSION}-slim-bookworm AS chef
RUN cargo install cargo-chef --locked --version ^0.1
WORKDIR /src

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- Build -------------------------------------------------------------------
FROM chef AS builder
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json \
    -p api -p worker -p seed
COPY . .
RUN cargo build --release --locked -p api -p worker -p seed --bins \
 && mkdir -p /out \
 && cp target/release/api target/release/worker target/release/seed /out/

# --- Runtime -----------------------------------------------------------------
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates tini \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /app --shell /usr/sbin/nologin app

WORKDIR /app
COPY --from=builder /out/ /app/bin/
COPY data/ /app/data/
COPY docker/entrypoint.sh /app/entrypoint.sh
RUN chmod 0755 /app/entrypoint.sh /app/bin/* \
 && chown -R root:root /app

ENV IH_SERVICE=api \
    API_HOST=0.0.0.0 \
    API_PORT=4000 \
    WORKER_METRICS_PORT=9090 \
    RUN_MIGRATIONS=true \
    LOG_FORMAT=json \
    RUST_LOG=info

USER app
EXPOSE 4000 9090
ENTRYPOINT ["/usr/bin/tini", "--", "/app/entrypoint.sh"]
