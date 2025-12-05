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

        lib = nixpkgs.lib;

        naersk-lib = pkgs.callPackage naersk { };

        clap-bash = naersk-lib.buildPackage {
          src = ./.;
          buildInputs = with pkgs; [
            pkg-config
            fuse3
          ];
        };

        writeClapBash =
          writer: filename: config:
          writer filename ''
            ${clap-bash}/bin/clap-bash \
                --add-self-to-env \
                --json ${lib.escapeShellArg (builtins.toJSON config)} \
                -- "$@"
          '';

        writeClapScript = writeClapBash pkgs.writeShellScript;
        writeClapScriptBin = writeClapBash pkgs.writeShellScriptBin;
      in
      {
        packages = {
          inherit clap-bash;
          default = clap-bash;

          clap-bash-test = writeClapScriptBin "clap-bash-test" {
            name = "clap-bash-test";
            about = "A small test script for clap-bash";
            executable = pkgs.writeShellScript "clap-bash-test-script" ''
              echo $ARG1
              echo $ARG2
              echo $ARG3
            '';
            args = [
              {
                arg1 = {
                  long = "arg1";
                  value_name = "ARG1";
                  arg_action = "append";
                  number_of_values = 2;
                };
              }
              {
                arg2 = {
                  long = "arg2";
                  arg_action = "append";
                };
              }
              {
                arg3 = {
                  required = true;
                };
              }
            ];
          };
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
      // {
        util = {
          inherit
            writeClapBash
            writeClapScript
            writeClapScriptBin
            ;
        };
      }
    );
}
