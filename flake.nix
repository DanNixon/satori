{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = {nixpkgs, ...}: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {inherit system;};
  in {
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = with pkgs; [
        # Rust toolchain
        rustup

        # Code formatting
        treefmt
        alejandra
        mdl
        typos

        # Rust dependency linting
        cargo-deny
      ];
    };
  };
}
