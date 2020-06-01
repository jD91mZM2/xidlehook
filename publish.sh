#!/usr/bin/env nix-shell
#!nix-shell -p pandoc -i sh

set -e

pandoc -s --toc -t gfm README.org -o README.md
eval "$VISUAL" README.md

echo "Looks good? Press enter to continue."
read -r

git reset

do_package() {
    echo "Running tests..."
    cargo check --manifest-path "$1/Cargo.toml"
    cargo test --manifest-path "$1/Cargo.toml"
    cargo check --all-features --manifest-path "$1/Cargo.toml"
    cargo test --all-features --manifest-path "$1/Cargo.toml"

    # If the lock file is changed, update that
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

echo "Now updating root lock file"
rm Cargo.lock
cargo check
git add Cargo.lock
git commit --amend --no-edit

echo "Trying nix build"
cargo2nix generate
nix-build .
git add Cargo.nix
git commit --amend --no-edit

echo "Now make a tag! Yay!"
cleanup
