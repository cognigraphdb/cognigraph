# Console assets are served by the Rust binary at the same origin.
FROM oven/bun:latest AS console
WORKDIR /ui
COPY ui/package.json ui/bun.lock ./
RUN bun install --frozen-lockfile
COPY ui/index.html ./
COPY ui/src/ src/
RUN bun run build

# Build stage
FROM rust:1-slim AS builder
WORKDIR /app
# build-essential: mlua vendors LuaJIT (cc + make).
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY vendor/ vendor/
COPY crates/ crates/
ARG COGNIGRAPH_EDITION=community
RUN case "$COGNIGRAPH_EDITION" in \
      community) cargo build --locked --release --no-default-features -p cognigraph-server -p cognigraph-cli ;; \
      enterprise) cargo build --locked --release --no-default-features --features enterprise -p cognigraph-server -p cognigraph-cli ;; \
      *) echo "COGNIGRAPH_EDITION must be community or enterprise" >&2; exit 1 ;; \
    esac
# The runtime has no shell, so its filesystem is assembled here: both binaries,
# the Railway start path kept as an alias of the server, and a /data mount
# point owned by the runtime user.
RUN mkdir -p /out/bin /out/data \
    && cp target/release/cognigraph-server target/release/cognigraph /out/bin/ \
    && ln -s cognigraph-server /out/bin/cognigraph-entrypoint \
    && chown 10001:10001 /out/data

# Runtime stage: distroless glibc (CG-81). No shell, package manager, curl or
# util-linux; root start-up and privilege dropping happen inside the server.
FROM gcr.io/distroless/cc-debian13:nonroot AS runtime
ARG COGNIGRAPH_VERSION=dev
ARG COGNIGRAPH_REVISION=unknown
ARG COGNIGRAPH_EDITION=community
LABEL org.opencontainers.image.source="https://github.com/cognigraphdb/cognigraph" \
    org.opencontainers.image.version="$COGNIGRAPH_VERSION" \
    org.opencontainers.image.revision="$COGNIGRAPH_REVISION" \
    io.cognigraph.edition="$COGNIGRAPH_EDITION"
COPY --from=builder /out/bin/ /usr/local/bin/
COPY --from=builder --chown=10001:10001 /out/data /data
COPY --from=console /ui/dist/ /ui/
COPY LICENSE LICENSE-COMMERCIAL /usr/share/licenses/cognigraph/
COPY vendor/tantivy-0.26.2/LICENSE /usr/share/licenses/cognigraph/tantivy-MIT
USER 10001:10001
ENV COGNIGRAPH_HOST=0.0.0.0 \
    COGNIGRAPH_PORT=3000 \
    COGNIGRAPH_NATIVE_PATH=/data/cognigraph.redb \
    COGNIGRAPH_UI_DIST=/ui \
    COGNIGRAPH_LOG_FORMAT=json \
    COGNIGRAPH_CONTAINER_INIT=1
# Attach persistent /data storage through the deployment platform or docker run.
# Railway rejects Docker's VOLUME instruction; the image must remain portable.
EXPOSE 3000
# The bundled CLI checks liveness and database readiness; no curl is shipped.
HEALTHCHECK --interval=15s --timeout=3s --start-period=10s \
    CMD ["/usr/local/bin/cognigraph", "--url", "http://127.0.0.1:3000", "health"]
ENTRYPOINT ["/usr/local/bin/cognigraph-server"]
