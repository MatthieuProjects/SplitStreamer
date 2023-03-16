#!/bin/sh

# build our binaries
#docker buildx bake --set "*.platform=linux/arm/v7" --set "*.target=bare" --set "*.output=type=local,dest=out"

# Copy our config folder
cp out/linux_arm_v7/client pigen-splitstreamer/01-copy-binaries
cp pigen.config pi-gen/config
cp -r pigen-splitstreamer pi-gen/
cd pi-gen
./build-docker.sh
