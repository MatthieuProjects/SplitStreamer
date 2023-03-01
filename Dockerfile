FROM rust as build

RUN apt-get update && \
  apt-get -y --no-install-recommends install software-properties-common && \
  add-apt-repository "deb http://httpredir.debian.org/debian sid main"
RUN apt update && apt install -y build-essential libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libgstreamer-plugins-bad1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav gstreamer1.0-tools gstreamer1.0-x gstreamer1.0-alsa gstreamer1.0-gl gstreamer1.0-gtk3 gstreamer1.0-qt5 gstreamer1.0-pulseaudio
# create a new empty shell project
RUN USER=root cargo new --bin splitstreamer
WORKDIR /splitstreamer

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# this build step will cache your dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy your source tree
COPY ./src ./src

# build for release
RUN rm ./target/release/deps/splitstreamer*
RUN cargo build --release

FROM debian
WORKDIR /app
RUN apt-get update && \
  apt-get -y --no-install-recommends install software-properties-common && \
  add-apt-repository "deb http://httpredir.debian.org/debian sid main"

RUN apt-get update && apt-get install -y gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav gstreamer1.0-x gstreamer1.0-alsa gstreamer1.0-gl gstreamer1.0-gtk3 gstreamer1.0-qt5 gstreamer1.0-pulseaudio

# copy the build artifact from the build stage
COPY --from=build /splitstreamer/target/release/splitstreamer .

# set the startup command to run your binary
CMD ["./splitstreamer"]

