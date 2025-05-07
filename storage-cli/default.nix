{
  pkgs,
  rustPlatform,
  version,
  gitRevision,
  buildInputs,
  nativeBuildInputs,
}: rec {
  satori-storage-cli = rustPlatform.buildRustPackage {
    pname = "satori-storage-cli";
    version = version;

    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;

    nativeBuildInputs = nativeBuildInputs;
    buildInputs = buildInputs;

    cargoBuildFlags = ["--package satori-storage-cli"];

    GIT_REVISION = gitRevision;

    # No need to do tests here, testing should have already been done earlier in CI pipeline
    doCheck = false;
  };

  satori-storage-cli-container-image = pkgs.dockerTools.buildImage {
    name = "satori-storage-cli";
    tag = "latest";
    created = "now";

    copyToRoot = pkgs.buildEnv {
      name = "image-root";
      paths = [pkgs.bashInteractive pkgs.coreutils];
      pathsToLink = ["/bin"];
    };

    config = {
      Entrypoint = ["${pkgs.tini}/bin/tini" "--" "${satori-storage-cli}/bin/satori-storage-cli"];
      Env = [
        "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      ];
    };
  };
}
