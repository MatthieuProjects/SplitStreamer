#!/bin/sh

# build our binaries
#docker buildx bake --set "*.platform=linux/arm/v7" --set "*.target=bare" --set "*.output=type=local,dest=out"

# Copy our config folder
cp out/linux_arm64/server pigen-splitstreamer/01-copy-binaries/
cp -r signaling/build pigen-splitstreamer/02-copy-signaling-server/dist
cp -r web/dist pigen-splitstreamer/03-copy-web-interface/dist

cp pigen.config pi-gen/config
cp -r pigen-splitstreamer pi-gen/splitstreamer
cd pi-gen
./build-docker.sh
