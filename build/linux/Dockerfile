# syntax=docker/dockerfile:1
ARG GO_VERSION=1.26.5
ARG NODE_VERSION=24.15.0
FROM golang:${GO_VERSION}-bookworm AS go-toolchain
FROM node:${NODE_VERSION}-bookworm-slim AS node-toolchain

# Match the Ubuntu 24.04 release-verification host and its WebKitGTK ABI.
FROM ubuntu:24.04 AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential ca-certificates file git pkg-config \
    libgtk-3-dev libwebkit2gtk-4.1-dev \
    && rm -rf /var/lib/apt/lists/*
COPY --from=go-toolchain /usr/local/go /usr/local/go
COPY --from=node-toolchain /usr/local/bin/node /usr/local/bin/node
COPY --from=node-toolchain /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx
ENV PATH="/usr/local/go/bin:/root/go/bin:${PATH}" CGO_ENABLED=1
ARG WAILS_VERSION=v2.16.0
RUN go install github.com/wailsapp/wails/v2/cmd/wails@${WAILS_VERSION}

WORKDIR /workspace
COPY go.mod go.sum ./
RUN go mod download
COPY frontend/package.json frontend/package-lock.json ./frontend/
RUN npm --prefix frontend ci
COPY . .
RUN npm --prefix frontend run build \
    && wails build -clean -trimpath -m -nocolour -nopackage \
        -platform linux/amd64 -tags webkit2_41 -o Librairii \
    && file build/bin/Librairii | grep 'ELF 64-bit.*x86-64' \
    && ldd build/bin/Librairii > /tmp/librairii-ldd.txt \
    && ! grep 'not found' /tmp/librairii-ldd.txt
RUN set -eu; \
    version="$(tr -d '[:space:]' < VERSION)"; \
    mkdir /out; \
    cp build/bin/Librairii /out/Librairii; \
    cd /out; \
    artifact="Librairii-${version}-linux-amd64.tar.gz"; \
    tar --sort=name --mtime='@0' --owner=0 --group=0 --numeric-owner \
        -czf "$artifact" Librairii; \
    sha256sum "$artifact" > "$artifact.sha256"; \
    sha256sum -c "$artifact.sha256"

FROM scratch AS artifacts
COPY --from=build /out/ /
