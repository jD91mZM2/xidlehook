with import <nixpkgs> {};

pkgs.xidlehook.overrideAttrs (orig: {
  LD_LIBRARY_PATH = "${pkgs.libpulseaudio}/lib";
})
