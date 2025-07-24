# syntax=docker/dockerfile:1

FROM debian:12-slim

# Install only essential runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the CLI binary (without GUI dependencies)
ARG TARGETARCH
COPY bin/linux_${TARGETARCH}/khm /usr/local/bin/khm
RUN chmod +x /usr/local/bin/khm

# Create non-root user
RUN useradd -m -u 1000 khm
USER khm

ENTRYPOINT ["/usr/local/bin/khm"]