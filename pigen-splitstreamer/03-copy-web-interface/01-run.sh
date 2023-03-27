#!/bin/bash -e

# Remove apache index.html
rm "${ROOTFS_DIR}/var/www/html/*"
# Copy assets
cp "./dist" "${ROOTFS_DIR}/var/www/html/"
