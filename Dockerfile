# syntax=docker/dockerfile:1
ARG TARGET

FROM --platform=$BUILDPLATFORM tonistiigi/xx:master AS xx
FROM --platform=$BUILDPLATFORM rust as alpine_rbuild
ENV TARGET=${TARGET}

# We need this to handle gstreamer packages.
RUN apt-get update \
  && apt-get -y --no-install-recommends install clang lld build-essential pkg-config \
  && apt-get upgrade -y

# Copy the xx scripts
COPY --from=xx / /

ARG TARGETPLATFORM
# Install the libraries dependent on architecture.
RUN xx-apt install -y libgcc-10-dev libglib2.0-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libgstreamer-plugins-bad1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav gstreamer1.0-tools gstreamer1.0-x gstreamer1.0-alsa gstreamer1.0-gl linux-libc-dev gstreamer1.0-nice

# Copy source code
COPY . .
RUN cd $TARGET
RUN PKG_CONFIG_PATH=/usr/lib/$(xx-info triple)/pkgconfig PKG_CONFIG_SYSROOT_DIR=/usr/$(xx-info triple) cargo fetch
RUN RUSTFLAGS="-L /usr/$(xx-info triple)" PKG_CONFIG_PATH=/usr/lib/$(xx-info triple)/pkgconfig PKG_CONFIG_SYSROOT_DIR=/usr/$(xx-info triple) xx-cargo build --release --target-dir ./build 

#Copy from the build/<target triple>/release folder to the out folder
RUN mkdir -p ./out && cp ./build/*/release/* ./out || true

FROM --platform=$BUILDPLATFORM scratch as bare
COPY --from=alpine_rbuild /out/$TARGET .

FROM debian AS runtime
ARG TARGET
ENV TARGET=${TARGET}

RUN apt-get update \
  && apt-get upgrade -y \
  && apt-get install -y gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav linux-libc-dev libc6 libc-bin gstreamer1.0-nice

COPY --from=alpine_rbuild /out/$TARGET /usr/local/bin/
ENTRYPOINT /usr/local/bin/$TARGET
