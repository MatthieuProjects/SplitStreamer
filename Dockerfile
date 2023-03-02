# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM tonistiigi/xx:master AS xx
FROM --platform=$BUILDPLATFORM rust as alpine_rbuild

# We need this to handle gstreamer packages.
RUN apt-get update && \
  apt-get -y --no-install-recommends install software-properties-common clang lld build-essential  && \
  add-apt-repository "deb http://httpredir.debian.org/debian sid main"

# Copy the xx scripts
COPY --from=xx / /

ARG TARGETPLATFORM
# Install the libraries dependent on architecture.
RUN xx-apt install -y libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libgstreamer-plugins-bad1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav gstreamer1.0-tools gstreamer1.0-x gstreamer1.0-alsa gstreamer1.0-gl

# Copy source code
COPY . .

RUN --mount=type=cache,target=/root/.cargo/git/db \
    --mount=type=cache,target=/root/.cargo/registry/cache \
    --mount=type=cache,target=/root/.cargo/registry/index \
    cargo fetch
RUN --mount=type=cache,target=/root/.cargo/git/db \
    --mount=type=cache,target=/root/.cargo/registry/cache \
    --mount=type=cache,target=/root/.cargo/registry/index \
    xx-cargo build --release --target-dir ./build

#Copy from the build/<target triple>/release folder to the out folder
RUN mkdir ./out && cp ./build/*/release/* ./out || true

FROM debian AS runtime

RUN apt-get update && \
  apt-get -y --no-install-recommends install software-properties-common && \
  add-apt-repository "deb http://httpredir.debian.org/debian sid main"

COPY --from=xx / /

ARG TARGETPLATFORM
RUN xx-apt install -y gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav

COPY --from=alpine_rbuild /out/splitstreamer /usr/local/bin/
ENTRYPOINT /usr/local/bin/splitstreamer