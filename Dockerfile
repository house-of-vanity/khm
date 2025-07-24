# syntax=docker/dockerfile:1

FROM ubuntu:22.04

# Install runtime dependencies including GUI libraries
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libgtk-3-0 \
    libglib2.0-0 \
    libcairo2 \
    libpango-1.0-0 \
    libatk1.0-0 \
    libgdk-pixbuf2.0-0 \
    libxdo3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the appropriate binary based on the target architecture
ARG TARGETARCH
COPY bin/linux_${TARGETARCH}/khm /usr/local/bin/khm
RUN chmod +x /usr/local/bin/khm

ENTRYPOINT ["/usr/local/bin/khm"]
