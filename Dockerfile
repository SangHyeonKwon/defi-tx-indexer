# ── Stage 1: Build ──────────────────────────────────────────────
FROM rust:1.86-slim-bookworm AS builder

WORKDIR /app

# 시스템 빌드 의존성 설치
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# 의존성 캐싱을 위해 Cargo 파일만 먼저 복사
COPY Cargo.toml Cargo.lock ./
COPY crates/db/Cargo.toml crates/db/Cargo.toml
COPY crates/decoder/Cargo.toml crates/decoder/Cargo.toml
COPY crates/indexer/Cargo.toml crates/indexer/Cargo.toml
COPY crates/api/Cargo.toml crates/api/Cargo.toml

# 더미 소스로 의존성만 빌드 (캐시 레이어)
RUN mkdir -p crates/db/src crates/decoder/src crates/indexer/src crates/api/src && \
    echo "pub fn dummy() {}" > crates/db/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/decoder/src/lib.rs && \
    echo "fn main() {}" > crates/indexer/src/main.rs && \
    echo "fn main() {}" > crates/api/src/main.rs && \
    cargo build --release -p api 2>/dev/null || true

# 실제 소스 복사 및 빌드
COPY . .
RUN touch crates/db/src/lib.rs crates/decoder/src/lib.rs crates/indexer/src/main.rs crates/api/src/main.rs && \
    cargo build --release -p api

# ── Stage 2: Runtime ───────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/api /usr/local/bin/api

EXPOSE 3000

CMD ["api"]
