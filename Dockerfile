# syntax=docker/dockerfile:1.4
# Sceneforged Docker Build
#
# Build from workspace root with BuildKit:
#   DOCKER_BUILDKIT=1 docker build -t sceneforged .

# ============================================================================
# Stage 1: Build the SvelteKit UI
# ============================================================================
FROM node:22-alpine AS ui-builder

ARG COMMIT_SHA=dev

RUN corepack enable && corepack prepare pnpm@latest --activate

WORKDIR /app/ui

# Copy package files first for layer caching
COPY ui/package.json ui/pnpm-lock.yaml ./

# Install dependencies with pnpm store cache mount
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile

# Copy source and build with commit hash injected as env var
COPY ui/ ./
ENV PUBLIC_COMMIT_SHA=$COMMIT_SHA
RUN pnpm run build

# ============================================================================
# Stage 2: cargo-chef prepare (compute dependency recipe)
# ============================================================================
FROM rust:1-trixie AS chef

RUN cargo install cargo-chef

WORKDIR /app

# Copy everything needed for `cargo chef prepare`
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY crates/ ./crates/

RUN cargo chef prepare --recipe-path recipe.json

# ============================================================================
# Stage 3: cargo-chef cook (build only dependencies)
# ============================================================================
FROM rust:1-trixie AS cook

RUN cargo install cargo-chef

# Install FFmpeg dev headers and build tools required by sf-av / sf-probe
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    clang \
    libavformat-dev \
    libavcodec-dev \
    libavutil-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=chef /app/recipe.json recipe.json

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --recipe-path recipe.json

# ============================================================================
# Stage 4: Build the actual binary
# ============================================================================
FROM cook AS builder

# Source is already set up from cook; copy real source over the stubs
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY crates/ ./crates/

# Build the release binary
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release -p sceneforged && \
    cp target/release/sceneforged /tmp/sceneforged

# Copy UI build output into a staging area for the runtime image
COPY --from=ui-builder /app/ui/build /tmp/static

# ============================================================================
# Stage 5: Minimal runtime image
# ============================================================================
FROM debian:trixie-slim AS runtime

# Install runtime media tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    ffmpeg \
    mediainfo \
    mkvtoolnix \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Install dovi_tool from GitHub releases (multi-arch)
ARG DOVI_TOOL_VERSION=2.3.1
RUN ARCH=$(dpkg --print-architecture) && \
    case "$ARCH" in \
        amd64) DOVI_ARCH="x86_64-unknown-linux-musl" ;; \
        arm64) DOVI_ARCH="aarch64-unknown-linux-musl" ;; \
        *) echo "Unsupported architecture: $ARCH" && exit 1 ;; \
    esac && \
    wget -q "https://github.com/quietvoid/dovi_tool/releases/download/${DOVI_TOOL_VERSION}/dovi_tool-${DOVI_TOOL_VERSION}-${DOVI_ARCH}.tar.gz" \
        -O /tmp/dovi_tool.tar.gz && \
    tar -xzf /tmp/dovi_tool.tar.gz -C /usr/local/bin && \
    chmod +x /usr/local/bin/dovi_tool && \
    rm /tmp/dovi_tool.tar.gz

# Create non-root user and application directories
RUN useradd -ms /bin/bash sceneforged && \
    mkdir -p /app/static /config /data && \
    chown -R sceneforged:sceneforged /app /config /data

WORKDIR /app

# Copy binary and static UI assets from builder
COPY --from=builder /tmp/sceneforged /app/sceneforged
COPY --from=builder /tmp/static /app/static

RUN chmod +x /app/sceneforged && \
    chown -R sceneforged:sceneforged /app

# Switch to non-root user
USER sceneforged

# Environment defaults
ENV RUST_LOG=info
ENV SCENEFORGED_CONFIG=/config/config.json

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD wget -q --spider http://localhost:8080/health || exit 1

CMD ["/app/sceneforged", "start", "--config", "/config/config.json"]
