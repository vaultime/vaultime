# Build image for the Linux bundles, the same base the release workflow uses.
FROM ubuntu:22.04

COPY install-build-deps.sh /tmp/install-build-deps.sh
RUN bash /tmp/install-build-deps.sh && rm -rf /var/lib/apt/lists/*

ENV CARGO_HOME=/opt/cargo \
    RUSTUP_HOME=/opt/rustup \
    PATH=/opt/cargo/bin:/opt/node/bin:$PATH

RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal --default-toolchain none

ARG NODE_VERSION=24.21.0
RUN curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" \
      | tar -xJ -C /opt \
 && mv "/opt/node-v${NODE_VERSION}-linux-x64" /opt/node
