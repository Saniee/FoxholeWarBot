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

COPY src ./src
# Migrations are embedded by sqlx::migrate! at compile time — no live database
# is needed to build the image.
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release

# --- runtime ----------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/FoxholeWarBot /usr/local/bin/foxholewarbot
COPY assets ./assets

# The API response cache lives here; mount a volume over it to survive restarts.
RUN mkdir -p /app/cache

CMD ["foxholewarbot"]
