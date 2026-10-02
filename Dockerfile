# Official multi-stage image for Sylphx Platform (dockerfile strategy).
#
# Deploy identity contract: the platform injects SYLPHX_GIT_COMMIT_SHA /
# SYLPHX_GIT_SHA build args (fleet contract). SOURCE_COMMIT / GIT_SHA are
# CI/local overrides. build.rs bakes the first non-empty value; the final
# image gate fails the build when no revision is embedded — an image without
# deploy identity is not shippable.
FROM rust:1.97-bookworm AS builder
ARG GIT_SHA=unknown
ARG SOURCE_COMMIT
ARG SYLPHX_GIT_COMMIT_SHA=
ARG SYLPHX_GIT_SHA=
ENV GIT_SHA=${GIT_SHA}
ENV SOURCE_COMMIT=${SOURCE_COMMIT}
ENV SYLPHX_GIT_COMMIT_SHA=${SYLPHX_GIT_COMMIT_SHA}
ENV SYLPHX_GIT_SHA=${SYLPHX_GIT_SHA}
WORKDIR /app
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY data ./data
COPY static ./static
COPY templates ./templates
COPY tests ./tests
RUN set -eu; \
    TIP="${SYLPHX_GIT_COMMIT_SHA:-}"; \
    [ -z "$TIP" ] && TIP="${SYLPHX_GIT_SHA:-}"; \
    [ -z "$TIP" ] && TIP="${SOURCE_COMMIT:-}"; \
    [ -z "$TIP" ] && TIP="${GIT_SHA:-}"; \
    [ "$TIP" = "unknown" ] && TIP=""; \
    TIP="$(printf '%s' "$TIP" | tr -d '[:space:]')"; \
    if [ -n "$TIP" ]; then \
      echo "Baking tip identity: $TIP"; \
      export GIT_SHA="$TIP" SOURCE_COMMIT="$TIP"; \
    else \
      echo "WARN: no tip build-arg provided" >&2; \
    fi; \
    cargo build --release --locked

FROM debian:bookworm-slim@sha256:3783cc01769c7b2b1b83a5c5ad96c815348e28ed7da68e2e3687004faa906251
ARG GIT_SHA=unknown
ARG SOURCE_COMMIT
ARG SYLPHX_GIT_COMMIT_SHA=
ARG SYLPHX_GIT_SHA=
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl \
  && update-ca-certificates \
  && rm -rf /var/lib/apt/lists/* \
  && test -s /etc/ssl/certs/ca-certificates.crt \
  && useradd --system --uid 65532 --create-home --home-dir /app nonroot
WORKDIR /app
ENV PORT=8787 \
    HOST=0.0.0.0 \
    RUST_LOG=mark=info \
    DEFAULT_CREDIT=0 \
    GIT_SHA=${GIT_SHA} \
    SOURCE_COMMIT=${SOURCE_COMMIT} \
    SYLPHX_GIT_COMMIT_SHA=${SYLPHX_GIT_COMMIT_SHA} \
    SYLPHX_GIT_SHA=${SYLPHX_GIT_SHA} \
    SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt \
    SSL_CERT_DIR=/etc/ssl/certs
COPY --from=builder /app/target/release/mark /usr/local/bin/mark
COPY static ./static
COPY templates ./templates
# Prove CA bundle + binary are real. `mark --help` must exit (not start the server).
RUN test -s /etc/ssl/certs/ca-certificates.crt \
  && test -x /usr/local/bin/mark \
  && mark --help | grep -q "Sylphx Mark" \
  && mark --version | grep -Eq "rev [0-9a-f]{7,}"
EXPOSE 8787
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8787/health >/dev/null || exit 1
USER nonroot
CMD ["mark"]
