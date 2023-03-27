#!/bin/bash -e

# Add nodesource repos
on_chroot << EOF
curl -fsSL https://deb.nodesource.com/setup_17.x | bash -
EOF
