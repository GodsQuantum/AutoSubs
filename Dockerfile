# syntax=docker/dockerfile:1

FROM node:26.11.1-trixie-slim@sha256:193fe51b64e77981119c98c2002c9e32a70e2f006fb4d25068ce0558998917f0 AS frontend
WORKDIR /src/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
COPY Cargo.toml CHANGELOG.md README.md /src/
COPY .github/workflows/appimage.yml .github/workflows/release.yml /src/.github/workflows/
COPY packaging/appimage/build.sh /src/packaging/appimage/
RUN npm run check && npm test && npm run build

FROM rust:1.99.0-trixie@sha256:81015672d6939b0265f09adf96d5de125482bb1330106076956b0518e74edc90 AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:sid-slim@sha256:1f11c32b03195c1b28c55e4579755659edae5c76dd76a69b4e4fb34c58fa8744 AS runtime
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
# Debian slim starts without a CA bundle. Signed, immutable snapshot indices
# authorize *every* .deb hash; native APT mirror transport retries failed files
# on ordinary Debian package pools before archived snapshot mirrors. All mirrors
# are constrained by the same Debian-signed 2026-09-29 Packages index.
RUN set -eu; \
    rm -f /etc/apt/sources.list /etc/apt/sources.list.d/*; \
    printf '%s\tpriority:%s type:%s\n' \
      'http://snapshot-cloudflare.debian.org/archive/debian/20260929T000000Z/' 1 index \
      'http://snapshot-fastly.debian.org/archive/debian/20260929T000000Z/' 2 index \
      'http://snapshot.debian.org/archive/debian/20260929T000000Z/' 3 index \
      'http://deb.debian.org/debian/' 1 deb \
      'http://ftp.debian.org/debian/' 2 deb \
      'http://snapshot-cloudflare.debian.org/archive/debian/20260929T000000Z/' 3 deb \
      'http://snapshot-fastly.debian.org/archive/debian/20260929T000000Z/' 4 deb \
      'http://snapshot.debian.org/archive/debian/20260929T000000Z/' 5 deb \
      > /etc/apt/snapshot-mirrors.txt; \
    printf '%s\n' 'deb [check-valid-until=no] mirror+file:/etc/apt/snapshot-mirrors.txt sid main' > /etc/apt/sources.list; \
    apt-get -o Acquire::Check-Valid-Until=false -o Acquire::Retries=1 -o Acquire::http::Timeout=25 -o APT::Update::Error-Mode=any update; \
    set -- ca-certificates curl fontconfig fonts-dejavu-core \
      ffmpeg=7:9.0.2-1 \
      mesa-va-drivers=26.2.3-2 \
      mesa-vulkan-drivers=26.2.3-2; \
    if [ "$(dpkg --print-architecture)" = "amd64" ]; then \
      set -- "$@" intel-media-va-driver=26.2.4+dfsg1-1; \
    fi; \
    # Snapshot CDN may return transient 503/504 for a few immutable packages.
    # Preserve already downloaded verified .debs between bounded retries.
    install_ok=0; \
    for attempt in 1 2 3 4 5 6; do \
      if apt-get -o Binary::apt::APT::Keep-Downloaded-Packages=true -o Acquire::Retries=2 -o Acquire::http::Timeout=30 -y install --no-install-recommends "$@"; then \
        install_ok=1; \
        break; \
      fi; \
      echo "APT transient fetch failure (attempt ${attempt}/6); retrying with cached packages" >&2; \
      sleep 12; \
    done; \
    test "$install_ok" -eq 1; \
    rm -rf /var/lib/apt/lists/*; \
    groupadd --gid 1000 autosubs; \
    useradd --uid 1000 --gid 1000 --home-dir /nonexistent --shell /usr/sbin/nologin autosubs; \
    mkdir -p /app/frontend /config /data /fonts /media; \
    chown -R 1000:1000 /config /data /fonts /media
WORKDIR /app
COPY --from=builder /src/target/release/autosubs /app/autosubs
COPY --from=frontend /src/frontend/build /app/frontend
USER 1000:1000
EXPOSE 3000
STOPSIGNAL SIGTERM
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 CMD ["curl","--fail","--silent","--show-error","http://127.0.0.1:3000/api/v1/health"]
ENTRYPOINT ["/app/autosubs"]
