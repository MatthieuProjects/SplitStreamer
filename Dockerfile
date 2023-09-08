# syntax=docker/dockerfile:1
ARG TARGET

# Base image for cargo-chef
FROM rust:bullseye AS chef
RUN cargo install cargo-chef
WORKDIR /app

# xx scripts for cross platform packages install
FROM --platform=$BUILDPLATFORM tonistiigi/xx:master AS xx

# cargo-chef planner
FROM --platform=$BUILDPLATFORM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# builder job
FROM --platform=$BUILDPLATFORM chef as builder

# Install build deps
RUN apt-get update \
	&& apt-get -y --no-install-recommends install clang lld build-essential pkg-config

ENV TARGET=${TARGET}
# Copy the xx scripts
COPY --from=xx / /

ARG TARGETPLATFORM

# Install the libraries dependent on architecture.
RUN xx-apt install -y libglib2.0-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libgstreamer-plugins-bad1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-tools gstreamer1.0-x gstreamer1.0-alsa linux-libc-dev gstreamer1.0-nice

COPY --from=planner /app/recipe.json recipe.json
# Build deps
RUN PKG_CONFIG_PATH=/usr/lib/$(xx-info triple)/pkgconfig PKG_CONFIG_SYSROOT_DIR=/usr/$(xx-info triple) cargo chef cook --target=$(xx-cargo --print-target-triple) --release --recipe-path recipe.json

# Copy source code
COPY . .
RUN cd $TARGET
RUN RUSTFLAGS="-L /usr/$(xx-info triple)" PKG_CONFIG_PATH=/usr/lib/$(xx-info triple)/pkgconfig PKG_CONFIG_SYSROOT_DIR=/usr/$(xx-info triple) xx-cargo build --release --target-dir ./build 

#Copy from the build/<target triple>/release folder to the out folder
RUN mkdir -p ./out && cp ./build/*/release/* ./out || true

FROM debian:bullseye AS runtime
ARG TARGET
ENV TARGET=${TARGET}

RUN apt-get update \
  && apt-get install -y \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    linux-libc-dev libc6 libc-bin gstreamer1.0-nice

COPY --from=builder /app/out/$TARGET /usr/local/bin/
ENTRYPOINT /usr/local/bin/$TARGET
