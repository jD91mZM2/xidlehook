{
  description = "A rust program";

  inputs = {
    utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nmattia/naersk";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system: let
      pkgs = nixpkgs.legacyPackages."${system}";
      naersk-lib = naersk.lib."${system}";

      nativeBuildInputs = with pkgs; [ pkgconfig python3 ];
      buildInputs = with pkgs; [ libpulseaudio xorg.libxcb xorg.libXScrnSaver x11 ];
    in rec {
      # `nix build`
      packages.xidlehook = naersk-lib.buildPackage {
        pname = "xidlehook";
        # TODO: Use workspaces
        # src = ./.;
        src = ./xidlehook-daemon;

        inherit nativeBuildInputs buildInputs;
      };
      defaultPackage = packages.xidlehook;

      # `nix run`
      apps.xidlehook = utils.lib.mkApp {
        drv = packages.xidlehook;
      };
      defaultApp = apps.xidlehook;

      # `nix develop`
      devShell = pkgs.mkShell {
        buildInputs = buildInputs;
        nativeBuildInputs = nativeBuildInputs ++ [ pkgs.rustc pkgs.cargo ];
      };
    });
}
