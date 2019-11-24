#!/bin/sh
# -*- eval: (aggressive-indent-mode -1); -*-

set -e

cargo run -- \
    --timer 2 "echo 2" "" \
    --timer 1 "echo 3" "" \
    --timer 1 "echo 4" "" \
    --timer 1 "echo 5" "" \
    --timer 5 "echo 10" "echo Cancelled between 10 and 15" \
    --timer 5 "echo 15" "" \
    --timer 5 "echo 20" "" \
    --socket /tmp/xidlehook-test.sock \
    --not-when-fullscreen --not-when-audio --once
