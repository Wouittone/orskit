FROM mcr.microsoft.com/devcontainers/rust:1-1.96-bookworm

ARG RUST_TOOLCHAIN=1.96.1
ARG CARGO_NEXTEST_VERSION=0.9.140
ARG JUST_VERSION=1.38.0

RUN rustup toolchain install "${RUST_TOOLCHAIN}" \
      --profile minimal \
      --component rustfmt,clippy \
    && rustup default "${RUST_TOOLCHAIN}" \
    && cargo "+${RUST_TOOLCHAIN}" install cargo-nextest \
      --locked \
      --version "${CARGO_NEXTEST_VERSION}" \
    && cargo "+${RUST_TOOLCHAIN}" install just \
      --locked \
      --version "${JUST_VERSION}"
