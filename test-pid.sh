#!/usr/bin/env bash

set -euo pipefail

cargo run -- \
    --timer 2 'pipes.sh' 'kill "$XIDLEHOOK_PID"'
