# Used by default.nix in case no nixpkgs is specified. Pinning is
# useful to ensure cachix binary cache gets used.

import (builtins.fetchGit {
  name = "nixos-19.09-2019-12-02";
  url = https://github.com/nixos/nixpkgs/;
  # Commit hash for nixos-unstable as of 2019-12-02
  # `git ls-remote https://github.com/nixos/nixpkgs-channels nixos-19.09`
  ref = "refs/heads/nixos-19.09";
  rev = "dae3575cee5b88de966d06b11861c602975cb23a";
})
