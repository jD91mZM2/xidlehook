#!/usr/bin/env nix-shell
#!nix-shell -p pandoc -i sh

set -e

pandoc -s --toc -t gfm README.org -o README.md
eval "$VISUAL" README.md

echo "Looks good? Press enter to continue."
read -r

echo "Running tests..."
cargo check
cargo test

do_package() {
    echo "Making sure packaging works..."
    cargo publish --dry-run --manifest-path "$1"/Cargo.toml

    version="$(sed -e 's/^version\s*=\s*"\([0-9]\+\.[0-9]\+\.[0-9]\+\)"/\1/' -e t -e d "$1"/Cargo.toml | head -n1)"

    git status

    echo "Publishing version $version of $1!!! Press enter to continue."
    read -r

    cargo publish --manifest-path "$1"/Cargo.toml
}

do_package xidlehook-core
echo "Waiting for crates.io to update"
sleep 5

do_package xidlehook-daemon # sets "$version"
git tag "$version" --annotate --sign -m "xidlehook $version"
