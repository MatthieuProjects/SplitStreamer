#!/bin/bash -e

install -m 664 signaling.service "${ROOTFS_DIR}/etc/systemd/system/"
mkdir -p "${ROOTFS_DIR}/opt/signaling"
cp -r "./dist/*" "${ROOTFS_DIR}/opt/signaling"
on_chroot << EOF
systemctl enable signaling.service
EOF
