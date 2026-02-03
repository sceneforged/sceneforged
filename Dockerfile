# syntax=docker/dockerfile:1.4
# Sceneforged Docker Build
#
# Build from workspace root with BuildKit:
#   DOCKER_BUILDKIT=1 docker build -t sceneforged .

# Stage 1: Build the SvelteKit UI
FROM node:20-alpine AS ui-builder

# Build arg for commit hash
ARG COMMIT_SHA=dev

# Enable and install pnpm
RUN corepack enable && corepack prepare pnpm@latest --activate

WORKDIR /app/ui

# Copy package files
COPY ui/package.json ui/pnpm-lock.yaml ./

# Install dependencies with cache mount
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile

# Copy source and build with commit hash
COPY ui/ ./
ENV PUBLIC_COMMIT_SHA=$COMMIT_SHA
RUN pnpm run build

# Stage 2: Build the Rust binary (Trixie for FFmpeg 7+ with coded_side_data API)
FROM rust:1.85-trixie AS rust-builder

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

# Copy all source files
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY crates/ ./crates/
COPY benches/ ./benches/
COPY tests/ ./tests/

# Build with cache mounts for cargo registry and target directory
# This provides true incremental builds across Docker builds
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release -p sceneforged && \
    cp target/release/sceneforged /tmp/sceneforged

# Stage 3: Runtime image (Trixie for FFmpeg 7+)
FROM debian:trixie-slim AS runtime

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

# Copy the binary from rust builder (copied to /tmp to survive cache mount)
COPY --from=rust-builder /tmp/sceneforged /app/sceneforged

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
