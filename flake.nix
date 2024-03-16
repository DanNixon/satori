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

        naersk' = pkgs.callPackage naersk {
          cargo = pkgs.cargo;
          rustc = pkgs.rustc;
        };

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        version = cargoToml.workspace.package.version;
        gitRevision = self.shortRev or self.dirtyShortRev;

        nativeBuildInputs = with pkgs; [cmake pkg-config];
        buildInputs = with pkgs; [openssl];

        lintingRustFlags = "-D unused-crate-dependencies";
      in rec {
        devShell = pkgs.mkShell {
          nativeBuildInputs = nativeBuildInputs;
          buildInputs = buildInputs;

          packages = with pkgs; [
            # Rust toolchain
            cargo
            clippy
            rustc
            rustfmt

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
