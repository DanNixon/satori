{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.05";

    flake-utils.url = "github:numtide/flake-utils";

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
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

        nativeBuildInputs = with pkgs; [pkg-config];
        buildInputs = with pkgs; [openssl];

        lintingRustFlags = "-D unused-crate-dependencies";
      in {
        devShell = pkgs.mkShell {
          nativeBuildInputs = nativeBuildInputs;
          buildInputs = buildInputs;

          packages = with pkgs; [
            # Rust toolchain
            cargo
            rustc

            # Code analysis tools
            clippy
            rust-analyzer

            # Code formatting tools
            treefmt
            alejandra
            mdl
            rustfmt

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
