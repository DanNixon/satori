{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";

    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    fenix,
    naersk,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        toolchain = fenix.packages.${system}.toolchainOf {
          channel = "1.75";
          date = "2023-12-28";
          sha256 = "SXRtAuO4IqNOQq+nLbrsDFbVk+3aVA8NNpSZsKlVH/8=";
        };

        naersk' = pkgs.callPackage naersk {
          cargo = toolchain.rust;
          rustc = toolchain.rust;
        };

        cargo = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        version = cargo.workspace.package.version;
        gitRevision = self.shortRev or self.dirtyShortRev;

        nativeBuildInputs = with pkgs; [cmake pkg-config];
        buildInputs = with pkgs; [openssl];

        lintingRustFlags = "-D unused-crate-dependencies";
      in rec {
        devShell = pkgs.mkShell {
          nativeBuildInputs = nativeBuildInputs ++ [toolchain.toolchain];
          buildInputs = buildInputs;

          packages = with pkgs; [
            # A newer version of Nix is required to use `dirtyShortRev`
            nix

            # Code formatting tools
            treefmt
            alejandra
            mdl

            # Rust dependency linting
            cargo-deny

            # Container image management tool
            skopeo
          ];

          RUSTFLAGS = lintingRustFlags;
        };

        packages =
          import ./agent {inherit pkgs naersk' version gitRevision nativeBuildInputs buildInputs;}
          // import ./archiver {inherit pkgs naersk' version gitRevision nativeBuildInputs buildInputs;}
          // import ./ctl {inherit pkgs naersk' version gitRevision nativeBuildInputs buildInputs;}
          // import ./event-processor {inherit pkgs naersk' version gitRevision nativeBuildInputs buildInputs;};
      }
    );
}
