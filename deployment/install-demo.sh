#!/bin/sh
set -eu
# Explicit prerequisite installation only; no cloud resources or source builds.
sudo apt-get update
sudo apt-get install --no-install-recommends python3 qemu-system-x86
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
exec python3 deployment/preflight.py
