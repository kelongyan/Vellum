#!/usr/bin/env bash
# CUSTOM(kelongyan): the fork's app-icon master is a raster (vellum-logo.png), so upstream's
# resvg pipeline for its SVG master is replaced by packaging/icons.py (Pillow). The script keeps
# the same name and outputs; see assets/app-icon/README.md.
set -euo pipefail
exec python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/icons.py"
# CUSTOM-END
