# syntax=docker/dockerfile:1
# 

FROM alpine:latest
COPY khm_linux-amd64/khm .
ENTRYPOINT ./khm
