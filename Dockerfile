# --- build ------------------------------------------------------------------
FROM rust:1-bookworm AS builder

WORKDIR /build

# Cache the dependency build: compile a stub against Cargo.toml/lock first, so a
# source-only change doesn't re-download and rebuild the whole dependency tree.
# Glob so the build works with or without a committed Cargo.lock. Committing the
# lock is recommended for a binary crate once you've run a build locally.
COPY Cargo.* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# The label font is include_bytes!'d, so it has to exist at *compile* time, not
# just in the runtime image. Only the font — the map and icon art is read from
# disk at runtime and has no business in the builder layer.
COPY assets/Inter-Bold.ttf ./assets/Inter-Bold.ttf

COPY src ./src
# Migrations are embedded by sqlx::migrate! at compile time — no live database
# is needed to build the image.
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release

# --- runtime ----------------------------------------------------------------
FROM debian:bookworm-slim

# libssl3: reqwest's default TLS backend links OpenSSL dynamically, so the
# runtime image needs it even though nothing here compiles.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/FoxholeWarBot /usr/local/bin/foxholewarbot
COPY assets ./assets

# The API response cache lives here; mount a volume over it to survive restarts.
# The log files live in /app/logs and are worth a volume for a different reason:
# they are the record of what happened, and a container that is rebuilt on every
# deploy takes them with it otherwise. `compose.yaml` mounts both.
RUN mkdir -p /app/cache /app/logs

CMD ["foxholewarbot"]
