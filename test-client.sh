#!/bin/sh
# -*- eval: (aggressive-indent-mode -1); -*-

set -e

alias woot='cargo run --bin xidlehook-client -- --socket "/tmp/xidlehook-test.sock"'

woot add \
    --time 10 \
    --index 0 \
    --activation "echo" "Timer:" "Activated" \; \
    --abortion "echo" "Timer:" "Aborted" \; \
    --deactivation "echo" "Timer:" "Deactivated" \;
woot add \
    --time 10 \
    --activation "sh" "-c" "echo hello" \;
woot control --timer 1 2 --action disable
woot query
woot control --action enable
woot query --timer 0 1 2
