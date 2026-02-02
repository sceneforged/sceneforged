# Build stage
FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /build

# Copy manifests first for layer caching
COPY Cargo.toml Cargo.lock* ./

# Create dummy src to cache dependencies
RUN mkdir -p src/bin && echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > src/bin/dv-convert.rs && \
    echo "pub fn probe() {}" > src/lib.rs
RUN cargo build --release && rm -rf src

# Copy actual source
COPY src ./src
COPY examples ./examples

# Build binaries
RUN touch src/lib.rs && cargo build --release --example probe --bin dv-convert

# Runtime stage
FROM alpine:3.21

RUN apk add --no-cache ffmpeg mkvtoolnix

COPY --from=builder /build/target/release/examples/probe /usr/local/bin/video-probe
COPY --from=builder /build/target/release/dv-convert /usr/local/bin/dv-convert

ENTRYPOINT ["/bin/sh"]
