# Build stage
FROM rust:1-slim AS builder
WORKDIR /app
# build-essential: mlua vendors LuaJIT (cc + make).
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*
COPY . .
ARG COGNIGRAPH_EDITION=community
RUN case "$COGNIGRAPH_EDITION" in \
      community) cargo build --release --no-default-features -p cognigraph-server -p cognigraph-cli ;; \
      enterprise) cargo build --release --no-default-features --features enterprise -p cognigraph-server -p cognigraph-cli ;; \
      *) echo "COGNIGRAPH_EDITION must be community or enterprise" >&2; exit 1 ;; \
    esac

# Runtime stage
FROM debian:stable-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 cognigraph \
    && mkdir -p /data && chown cognigraph /data
COPY --from=builder /app/target/release/cognigraph-server /usr/local/bin/cognigraph-server
COPY --from=builder /app/target/release/cognigraph /usr/local/bin/cognigraph
USER cognigraph
ENV COGNIGRAPH_HOST=0.0.0.0 \
    COGNIGRAPH_PORT=3000 \
    COGNIGRAPH_BACKEND=native \
    COGNIGRAPH_NATIVE_PATH=/data/cognigraph.redb \
    COGNIGRAPH_LOG_FORMAT=json
VOLUME /data
EXPOSE 3000
HEALTHCHECK --interval=15s --timeout=3s --start-period=10s \
    CMD curl -fsS http://127.0.0.1:3000/health || exit 1
ENTRYPOINT ["cognigraph-server"]
