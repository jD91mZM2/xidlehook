{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [ xorg.libxcb xorg.libX11 xorg.libXScrnSaver libpulseaudio ];
  nativeBuildInputs = with pkgs; [ pkg-config ];
  LD_LIBRARY_PATH = "${pkgs.libpulseaudio}/lib";
}
