# syntax=docker/dockerfile:1

FROM alpine:3.19

# Install glibc compatibility for Alpine
RUN apk add --no-cache \
    ca-certificates \
    gcompat \
    libgcc \
    libstdc++

# Copy the CLI binary (without GUI dependencies)
ARG TARGETARCH
COPY bin/linux_${TARGETARCH}/khm /usr/local/bin/khm
RUN chmod +x /usr/local/bin/khm

# Create non-root user
RUN adduser -D -u 1000 khm
USER khm

ENTRYPOINT ["/usr/local/bin/khm"]