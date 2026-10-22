#!/bin/sh
set -eu
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
python3 deployment/preflight.py
exec python3 tools/dashboard.py "$@"
