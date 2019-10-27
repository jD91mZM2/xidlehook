{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  # Things to be put in $PATH
  nativeBuildInputs = with pkgs; [ pkgconfig ];

  # Libraries to be installed
  buildInputs = with pkgs; [ openssl xorg.libxcb xorg.libXScrnSaver ];
}
