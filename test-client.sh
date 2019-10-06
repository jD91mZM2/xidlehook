#!/bin/sh

set -e

alias woot='cargo +beta run --features unstable --bin xidlehook-client -- --socket "/tmp/xidlehook-test.sock"'

woot add \
      --duration 10 \
      --index 0 \
      --activation "echo" "Timer:" "Activated" \; \
      --abortion "echo" "Timer:" "Aborted" \; \
      --deactivation "echo" "Timer:" "Deactivated" \;
woot add \
      --duration 10 \
      --activation "sh" "-c" "hello" \;
woot control --timer 1 2 --action disable
woot query
woot control --action enable
woot query --timer 0 1 2
