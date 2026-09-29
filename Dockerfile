# syntax=docker/dockerfile:1

FROM node:24.21.0-trixie-slim@sha256:8ec5d7557396cfe32d21c3f9c13072355ceab22b584578ca4bb28af31120cffe AS frontend
WORKDIR /src/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
COPY Cargo.toml CHANGELOG.md README.md /src/
RUN npm run check && npm test && npm run build

FROM rust:1.98.1-trixie@sha256:a8a5f0a1e5fe7dfe1d352591e4a1c7dd2c08fd70475cae872cf3458ba0df0546 AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:sid-slim@sha256:ec3fa4e0b2987ae47be353f56854191e300261915470e40165b1c906d22a65db AS runtime
ENV DEBIAN_FRONTEND=noninteractive \
    AUTOSUBS_HOST=0.0.0.0 \
    AUTOSUBS_PORT=3000 \
    AUTOSUBS_CONFIG_DIR=/config \
    AUTOSUBS_DATA_DIR=/data \
    AUTOSUBS_FONTS_DIR=/fonts \
    AUTOSUBS_DIST_DIR=/app/frontend \
    AUTOSUBS_ALLOWED_ROOTS=/data:/media \
    HOME=/tmp \
    XDG_CACHE_HOME=/tmp/.cache
RUN rm -f /etc/apt/sources.list /etc/apt/sources.list.d/* \
 && printf '%s\n' 'deb [check-valid-until=no] http://snapshot.debian.org/archive/debian/20260929T000000Z sid main' > /etc/apt/sources.list \
 && apt-get -o Acquire::Check-Valid-Until=false update \
 && apt-get install -y --no-install-recommends \
      ca-certificates curl fontconfig fonts-dejavu-core \
      ffmpeg=7:9.0.2-1 \
      mesa-va-drivers=26.2.3-2 \
      mesa-vulkan-drivers=26.2.3-2 \
 && if [ "$(dpkg --print-architecture)" = "amd64" ]; then \
      apt-get install -y --no-install-recommends intel-media-va-driver=26.2.4+dfsg1-1; \
    fi \
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
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 CMD ["curl","--fail","--silent","--show-error","http://127.0.0.1:3000/api/v1/health"]
ENTRYPOINT ["/app/autosubs"]
