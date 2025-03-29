{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.11";

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
          channel = "1.84";
          date = "2025-01-09";
          sha256 = "lMLAupxng4Fd9F1oDw8gx+qA0RuF7ou7xhNU8wgs0PU=";
        };

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain.cargo;
          rustc = rustToolchain.rustc;
        };

        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

        version = cargoToml.workspace.package.version;
        gitRevision = self.shortRev or self.dirtyShortRev;

        nativeBuildInputs = with pkgs; [pkg-config];
        buildInputs = with pkgs; [openssl];

        lintingRustFlags = "-D unused-crate-dependencies";
      in {
        devShell = pkgs.mkShell {
          nativeBuildInputs = nativeBuildInputs;
          buildInputs = buildInputs;

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
          import ./agent {inherit pkgs rustPlatform version gitRevision nativeBuildInputs buildInputs;}
          // import ./archiver {inherit pkgs rustPlatform version gitRevision nativeBuildInputs buildInputs;}
          // import ./event-processor {inherit pkgs rustPlatform version gitRevision nativeBuildInputs buildInputs;}
          // import ./storage-cli {inherit pkgs rustPlatform version gitRevision nativeBuildInputs buildInputs;};
      }
    );
}
