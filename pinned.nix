# Used by default.nix in case no nixpkgs is specified. Pinning is
# useful to ensure cachix binary cache gets used.

import (builtins.fetchGit {
  name = "nixos-19.09-2020-06-09";
  url = https://github.com/nixos/nixpkgs/;
  # Commit hash for nixos-unstable as of 2020-06-09
  # `git ls-remote https://github.com/nixos/nixpkgs-channels nixos-19.09`
  ref = "refs/heads/nixos-19.09";
  rev = "0a11634a29c1c9ffe7bfa08fc234fef2ee978dbb";
})
