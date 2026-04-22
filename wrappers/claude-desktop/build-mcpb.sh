#!/usr/bin/env bash
set -euo pipefail

# Build a .mcpb Desktop Extension package
# Usage: ./build-mcpb.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT="${SCRIPT_DIR}/no-tickets.mcpb"

cd "$SCRIPT_DIR"
zip -j "$OUTPUT" manifest.json
echo "Built: $OUTPUT"
