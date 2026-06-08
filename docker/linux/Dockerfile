FROM ubuntu:24.04

LABEL org.opencontainers.image.source="https://github.com/AliusTech/alius"
LABEL org.opencontainers.image.description="Alius Linux release build environment"

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    curl \
    git \
    tar \
    unzip \
    xz-utils \
    zip \
    zstd \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile default --default-toolchain stable

ENV CARGO_HOME="/root/.cargo"
ENV RUSTUP_HOME="/root/.rustup"
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /workspace
