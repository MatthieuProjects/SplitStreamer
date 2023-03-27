#!/bin/bash -e

install -m 115 server "${ROOTFS_DIR}/usr/bin/"
install -m 664 splitstreamer.service "${ROOTFS_DIR}/etc/systemd/system/"

on_chroot << EOF
systemctl enable splitstreamer.service
EOF
