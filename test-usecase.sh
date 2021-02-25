#!/usr/bin/env bash

set -euo pipefail

pipe="$(mktemp -d)/pipe"

mkfifo "$pipe"

read -rd '' lock <<EOF || true
while true; do
    read line
    echo "\$line"
done < "$pipe" | zenity --progress --text "Locked"
EOF

echo "Lock: $lock"

read -rd '' suspend <<EOF || true
zenity --info --text "Suspended"
EOF

echo "Suspend: $suspend"

cargo run -- \
    --timer 1 "$lock" "" \
    --timer 2 "echo 0 > $pipe" "" \
    --timer 2 "echo 20 > $pipe" "" \
    --timer 2 "echo 40 > $pipe" "" \
    --timer 2 "echo 60 > $pipe" "" \
    --timer 2 "echo 80 > $pipe" "" \
    --timer 2 "echo 100 > $pipe" "" \
    --timer 2 "$suspend" ""
