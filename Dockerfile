# Sceneforged Docker Build
#
# Build from workspace root:
#   docker build -t sceneforged .

# Stage 1: Build the SvelteKit UI
FROM node:20-alpine AS ui-builder

# Enable and install pnpm
RUN corepack enable && corepack prepare pnpm@latest --activate

WORKDIR /app/ui

# Copy package files
COPY ui/package.json ui/pnpm-lock.yaml ./

# Install dependencies
RUN pnpm install --frozen-lockfile

# Copy source and build
COPY ui/ ./
RUN pnpm run build

# Stage 2: Build the Rust binary
FROM rust:1.85-bookworm AS rust-builder

WORKDIR /app

# Install build dependencies including FFmpeg dev headers for native bindings
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libavformat-dev \
    libavcodec-dev \
    libavdevice-dev \
    libavfilter-dev \
    libavutil-dev \
    libswscale-dev \
    libswresample-dev \
    clang \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace Cargo files first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates/sceneforged-av/Cargo.toml crates/sceneforged-av/
COPY crates/sceneforged-common/Cargo.toml crates/sceneforged-common/
COPY crates/sceneforged-db/Cargo.toml crates/sceneforged-db/
COPY crates/sceneforged-media/Cargo.toml crates/sceneforged-media/
COPY crates/sceneforged-parser/Cargo.toml crates/sceneforged-parser/
COPY crates/sceneforged-probe/Cargo.toml crates/sceneforged-probe/

# Create dummy sources to build dependencies
RUN mkdir -p src \
    crates/sceneforged-av/src \
    crates/sceneforged-common/src \
    crates/sceneforged-db/src \
    crates/sceneforged-media/src \
    crates/sceneforged-parser/src \
    crates/sceneforged-probe/src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn dummy() {}" > src/lib.rs && \
    echo "pub fn dummy() {}" > crates/sceneforged-av/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/sceneforged-common/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/sceneforged-db/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/sceneforged-media/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/sceneforged-parser/src/lib.rs && \
    echo "pub fn dummy() {}" > crates/sceneforged-probe/src/lib.rs
RUN cargo build --release -p sceneforged || true
RUN rm -rf src crates/*/src

# Copy actual source
COPY src/ ./src/
COPY crates/ ./crates/
COPY benches/ ./benches/
COPY tests/ ./tests/

# Build the binary
RUN cargo build --release -p sceneforged

# Stage 3: Runtime image
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies and media tools
RUN apt-get update && apt-get install -y \
    ca-certificates \
    ffmpeg \
    mediainfo \
    mkvtoolnix \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Install dovi_tool from GitHub releases
ARG DOVI_TOOL_VERSION=2.3.1
RUN ARCH=$(dpkg --print-architecture) && \
    case "$ARCH" in \
        amd64) DOVI_ARCH="x86_64-unknown-linux-musl" ;; \
        arm64) DOVI_ARCH="aarch64-unknown-linux-musl" ;; \
        *) echo "Unsupported architecture: $ARCH" && exit 1 ;; \
    esac && \
    wget -q "https://github.com/quietvoid/dovi_tool/releases/download/${DOVI_TOOL_VERSION}/dovi_tool-${DOVI_TOOL_VERSION}-${DOVI_ARCH}.tar.gz" -O /tmp/dovi_tool.tar.gz && \
    tar -xzf /tmp/dovi_tool.tar.gz -C /usr/local/bin && \
    chmod +x /usr/local/bin/dovi_tool && \
    rm /tmp/dovi_tool.tar.gz

# Create app user
RUN useradd -ms /bin/bash sceneforged

# Create directories
RUN mkdir -p /app/static /config /data && \
    chown -R sceneforged:sceneforged /app /config /data

WORKDIR /app

# Copy the binary from rust builder
COPY --from=rust-builder /app/target/release/sceneforged /app/sceneforged

# Copy the UI build from ui builder
COPY --from=ui-builder /app/ui/build /app/static

# Copy example config
COPY config.example.toml /config/config.example.toml

# Set permissions
RUN chmod +x /app/sceneforged && \
    chown -R sceneforged:sceneforged /app /config /data

# Switch to app user
USER sceneforged

# Environment variables
ENV RUST_LOG=info
ENV SCENEFORGED_CONFIG=/config/config.toml

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD wget -q --spider http://localhost:8080/health || exit 1

# Default command
CMD ["/app/sceneforged", "start", "--config", "/config/config.toml"]
