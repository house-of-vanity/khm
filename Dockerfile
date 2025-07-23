# syntax=docker/dockerfile:1

FROM ubuntu:22.04

# Install basic runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY khm /usr/local/bin/khm
RUN chmod +x /usr/local/bin/khm

ENTRYPOINT ["/usr/local/bin/khm"]
