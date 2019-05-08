{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [ xlibsWrapper xorg.libXScrnSaver libpulseaudio ];
  nativeBuildInputs = with pkgs; [ pkg-config ];
  LD_LIBRARY_PATH = "${pkgs.libpulseaudio}/lib";
}
