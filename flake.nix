{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.05";

    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    fenix,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        rustToolchain = fenix.packages.${system}.toolchainOf {
          channel = "1.88";
          date = "2025-06-26";
          sha256 = "Qxt8XAuaUR2OMdKbN4u8dBJOhSHxS+uS06Wl9+flVEk=";
        };

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain.cargo;
          rustc = rustToolchain.rustc;
        };

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        version = cargoToml.workspace.package.version;
        gitRevision = self.shortRev or self.dirtyShortRev;

        lintingRustFlags = "-D unused-crate-dependencies";
      in {
        devShell = pkgs.mkShell {
          packages = with pkgs; [
            # Rust toolchain
            rustToolchain.toolchain

            # Code formatting
            treefmt
            alejandra
            mdl

            # Rust dependency linting
            cargo-deny

            # Container image management
            skopeo
          ];

          RUSTFLAGS = lintingRustFlags;
        };

        packages =
          import ./agent {inherit pkgs rustPlatform version gitRevision;}
          // import ./archiver {inherit pkgs rustPlatform version gitRevision;}
          // import ./ctl {inherit pkgs rustPlatform version gitRevision;}
          // import ./event-processor {inherit pkgs rustPlatform version gitRevision;};
      }
    );
}
