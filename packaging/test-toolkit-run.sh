#!/usr/bin/env bash
# Keep the owner separate from the caller's traps and shell state.
exec python3 "$(dirname -- "${BASH_SOURCE[0]}")/test-toolkit-run.py" "$@"
