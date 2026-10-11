#!/bin/sh
set -eu

# Keep the shell entry point for existing local reproduction commands.
exec python3 "$(dirname "$0")/bounded.py"
