{pkgs, ...}: {
  packages = with pkgs; [
    # Rust toolchain
    rustup

    # Code formatting
    treefmt
    alejandra
    mdl

    # Rust dependency linting
    cargo-deny
  ];
}
