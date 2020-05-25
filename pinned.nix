# Used by default.nix in case no nixpkgs is specified. Pinning is
# useful to ensure cachix binary cache gets used.

import (builtins.fetchGit {
  name = "nixos-19.09-2020-05-25";
  url = https://github.com/nixos/nixpkgs/;
  # Commit hash for nixos-unstable as of 2020-05-25
  # `git ls-remote https://github.com/nixos/nixpkgs-channels nixos-19.09`
  ref = "refs/heads/nixos-19.09";
  rev = "2efedf8fc74e8056f81bd18899276b085becf6dc";
})
