#!/usr/bin/env nix-shell
#!nix-shell -p pandoc -i sh

set -e

pandoc -s --toc -t gfm README.org -o README.md
eval "$VISUAL" README.md

echo "Looks good? Press enter to continue."
read -r

do_package() {
    echo "Running tests..."
    cargo check --manifest-path "$1/Cargo.toml"
    cargo test --manifest-path "$1/Cargo.toml"

    # If the lock file is changed, update that
    git reset
    git add "$1/Cargo.lock"
    git commit --amend --no-edit

    echo "Making sure packaging works..."
    cargo publish --dry-run --manifest-path "$1"/Cargo.toml

    git status

    echo "Publishing $1!!! Press enter to continue."
    read -r

    cargo publish --manifest-path "$1"/Cargo.toml
}

mv Cargo.toml Cargo.toml.bak
cleanup() {
    mv Cargo.toml.bak Cargo.toml
}
trap cleanup SIGINT

do_package xidlehook-core
echo "Waiting for crates.io to update"
sleep 5

do_package xidlehook-daemon

echo "Now make a tag! Yay!"

cleanup
