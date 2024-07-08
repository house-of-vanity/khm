# syntax=docker/dockerfile:1

FROM alpine:latest
COPY khm_linux-amd64/khm /usr/local/bin/khm
ENTRYPOINT ["/usr/local/bin/khm"]
