with import <nixpkgs> {};

pkgs.xidlehook.overrideAttrs (old: {
  buildInputs = lib.remove pkgs.rustc old.buildInputs;

  LD_LIBRARY_PATH = "${pkgs.libpulseaudio}/lib";
})
