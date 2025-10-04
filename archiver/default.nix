{
  pkgs,
  rustPlatform,
  version,
  gitRevision,
}: rec {
  satori-archiver = rustPlatform.buildRustPackage {
    pname = "satori-archiver";
    version = version;

    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;

    cargoBuildFlags = ["--package satori-archiver"];

    GIT_REVISION = gitRevision;

    # No need to do tests here, testing should have already been done earlier in CI pipeline
    doCheck = false;
  };

  satori-archiver-container-image = pkgs.dockerTools.buildImage {
    name = "satori-archiver";
    tag = "latest";
    created = "now";

    copyToRoot = pkgs.buildEnv {
      name = "image-root";
      paths = [pkgs.bashInteractive pkgs.coreutils];
      pathsToLink = ["/bin"];
    };

    config = {
      Entrypoint = ["${pkgs.tini}/bin/tini" "--" "${satori-archiver}/bin/satori-archiver"];
      ExposedPorts = {
        "9090/tcp" = {};
      };
      Env = [
        "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
        "OBSERVABILITY_ADDRESS=0.0.0.0:9090"
      ];
    };
  };
}
