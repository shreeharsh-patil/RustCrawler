# -----------------------------------------------------------------------------
# Stage 1: Build binary
# -----------------------------------------------------------------------------
FROM rust:1.80-slim-bookworm AS builder

WORKDIR /usr/src/rustcrawl

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Copy actual source code and build release binary
COPY src ./src
RUN cargo build --release

# -----------------------------------------------------------------------------
# Stage 2: Minimal runtime image
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies (OpenSSL, CA-certs, Chromium, fonts)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    chromium \
    chromium-sandbox \
    fonts-liberation \
    fonts-noto-color-emoji \
    dumb-init \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN groupadd -g 10001 rustcrawl && \
    useradd -u 10001 -g rustcrawl -s /bin/bash -m rustcrawl

WORKDIR /app

# Copy binary from builder
COPY --from=builder /usr/src/rustcrawl/target/release/rustcrawl /usr/local/bin/rustcrawl

# Ensure permissions
RUN chown -R rustcrawl:rustcrawl /app

USER rustcrawl

ENV SERVER_HOST=0.0.0.0
ENV SERVER_PORT=3000
ENV BROWSER_EXECUTABLE_PATH=/usr/bin/chromium
ENV RUST_LOG=info

EXPOSE 3000

ENTRYPOINT ["/usr/bin/dumb-init", "--"]
CMD ["rustcrawl", "serve"]
