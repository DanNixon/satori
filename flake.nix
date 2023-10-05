{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix, naersk }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        toolchain = fenix.packages.${system}.toolchainOf {
          channel = "1.72";
          date = "2023-09-19";
          sha256 = "dxE7lmCFWlq0nl/wKcmYvpP9zqQbBitAQgZ1zx9Ooik=";
        };

        naersk' = pkgs.callPackage naersk {
          cargo = toolchain.rust;
          rustc = toolchain.rust;
        };

        ws_cargo_toml = builtins.readFile ./Cargo.toml;
        ws_cargo = builtins.fromTOML ws_cargo_toml;

        version = ws_cargo.workspace.package.version;
        git_revision = self.shortRev or self.dirtyShortRev;

        nativeBuildInputs = with pkgs; [ cmake pkg-config ];
        buildInputs = with pkgs; [ openssl ];

      in rec {
        devShell = pkgs.mkShell {
          nativeBuildInputs = nativeBuildInputs ++ [ toolchain.toolchain ];
          buildInputs = buildInputs;
          packages = with pkgs; [ alejandra treefmt nix skopeo ];
        };

        packages = {
          fmt = naersk'.buildPackage {
            src = ./.;
            nativeBuildInputs = nativeBuildInputs;
            buildInputs = buildInputs;
            mode = "fmt";
          };

          clippy = naersk'.buildPackage {
            src = ./.;
            nativeBuildInputs = nativeBuildInputs;
            buildInputs = buildInputs;
            mode = "clippy";
          };

          test = naersk'.buildPackage {
            src = ./.;
            nativeBuildInputs = nativeBuildInputs;
            buildInputs = buildInputs;
            mode = "test";
            # Ensure detailed test output appears in nix build log
            cargoTestOptions = x: x ++ ["1>&2"];

            AWS_ACCESS_KEY_ID = "minioadmin";
            AWS_SECRET_ACCESS_KEY = "minioadmin";
          };
        } //
        import ./agent { inherit pkgs naersk' version git_revision nativeBuildInputs buildInputs; } //
        import ./archiver { inherit pkgs naersk' version git_revision nativeBuildInputs buildInputs; } //
        import ./ctl { inherit pkgs naersk' version git_revision nativeBuildInputs buildInputs; } //
        import ./event-processor { inherit pkgs naersk' version git_revision nativeBuildInputs buildInputs; };
      }
    );
}
