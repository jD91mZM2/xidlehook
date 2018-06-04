with import <nixpkgs> {};

pkgs.xidlehook.overrideAttrs (old: {
  buildInputs = lib.filter (pkg: pkg != pkgs.rustc && pkg != pkgs.cargo) old.buildInputs;

  LD_LIBRARY_PATH = "${pkgs.libpulseaudio}/lib";
})
