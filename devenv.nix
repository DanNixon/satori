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

    # Container image management
    skopeo
  ];

  env.LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
}
