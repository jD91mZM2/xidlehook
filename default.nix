{ pkgsFn ? import ./pinned.nix }:

let
  mozOverlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  crateOverlay = self: super: {
    defaultCrateOverrides = super.defaultCrateOverrides // {
      xidlehook = _attrs: {
        buildInputs = with self; [ xorg.libxcb ];
      };
    };
  };
  pkgs = pkgsFn { overlays = [ mozOverlay crateOverlay ]; };
  buildRustCrate = pkgs.buildRustCrate.override {
    rustc = pkgs.latest.rustChannels.stable.rust;
  };
in (pkgs.callPackage ./Cargo.nix { inherit buildRustCrate; }).workspaceMembers.xidlehook.build.override {
  features = [];
}
