# syntax=docker/dockerfile:1.7

FROM node:25-bookworm-slim AS frontend
WORKDIR /src/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run check && npm test && npm run build

FROM rust:1.98-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:trixie-slim AS runtime
ENV DEBIAN_FRONTEND=noninteractive \
    AUTOSUBS_HOST=0.0.0.0 \
    AUTOSUBS_PORT=3000 \
    AUTOSUBS_CONFIG_DIR=/config \
    AUTOSUBS_DATA_DIR=/data \
    AUTOSUBS_FONTS_DIR=/fonts \
    AUTOSUBS_DIST_DIR=/app/frontend \
    AUTOSUBS_ALLOWED_ROOTS=/data:/media
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl ffmpeg fontconfig fonts-dejavu-core \
 && rm -rf /var/lib/apt/lists/* \
 && groupadd --gid 1000 autosubs \
 && useradd --uid 1000 --gid 1000 --home-dir /nonexistent --shell /usr/sbin/nologin autosubs \
 && mkdir -p /app/frontend /config /data /fonts /media \
 && chown -R 1000:1000 /config /data /fonts /media
WORKDIR /app
COPY --from=builder /src/target/release/autosubs /app/autosubs
COPY --from=frontend /src/frontend/build /app/frontend
USER 1000:1000
EXPOSE 3000
STOPSIGNAL SIGTERM
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 CMD ["curl","--fail","--silent","--show-error","http://127.0.0.1:3000/api/v1/health"]
ENTRYPOINT ["/app/autosubs"]
