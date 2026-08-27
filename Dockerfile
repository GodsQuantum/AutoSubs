# Stage 1: Frontend build
FROM node:20-alpine AS frontend
WORKDIR /app
COPY frontend/package*.json ./
RUN npm install
COPY frontend/ ./
RUN npm run build

# Stage 2: Rust build
FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev pkgconfig
WORKDIR /app
COPY Cargo.toml ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release && rm -rf src
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Stage 3: runtime
FROM alpine:3.20
RUN apk add --no-cache ffmpeg ca-certificates
RUN addgroup -S autosubs && adduser -S autosubs -G autosubs
WORKDIR /app
COPY --from=builder /app/target/release/autosubs /app/autosubs
COPY --from=frontend /app/dist /app/dist
COPY --chown=autosubs:autosubs . /app
USER autosubs
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s CMD wget -qO- http://127.0.0.1:3000/api/settings || exit 1
ENTRYPOINT ["/app/autosubs"]
