#!/usr/bin/env nix-shell
#!nix-shell -p pandoc -i sh

set -e

if ! which cargo-release 2>&1 > /dev/null; then
    echo "You need to install cargo-release"
    echo "$ cargo install cargo-release"
    exit 1
fi

pandoc -s --toc -t gfm README.org -o README.md
eval "$VISUAL" README.md

echo "Looks good? Press enter to continue."
read -r

echo "Running tests..."
cargo check
cargo test

echo "Making sure packaging works..."
cargo publish --dry-run --manifest-path xidlehook-core/Cargo.toml
cargo publish --dry-run --manifest-path xidlehook-daemon/Cargo.toml

version="$(sed -e 's/^version\s*=\s*"\([0-9]\+\.[0-9]\+\.[0-9]\+\)"/\1/' -e t -e d xidlehook-daemon/Cargo.toml)"

git status

echo "Publishing version $version!!! Press enter to continue."
read -r

git tag "$version" --annotate --sign -m "xidlehook $version"

cargo publish --manifest-path xidlehook-core/Cargo.toml &
cargo publish --manifest-path xidlehook-daemon/Cargo.toml &
wait
