{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    flake-utils.url = "github:numtide/flake-utils";

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      flake-utils,
      naersk,
      nixpkgs,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        naersk-lib = pkgs.callPackage naersk { };

        clap-bash = naersk-lib.buildPackage {
          src = ./.;
          buildInputs = with pkgs; [
            pkg-config
            fuse3
          ];
        };
      in
      {
        packages = {
          inherit clap-bash;
        };

        defaultPackage = self.packages.${system}.clap-bash;

        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo
            rustc

          ];

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          packages = with pkgs; [
            rust-analyzer
          ];
        };
      }
    );
}
