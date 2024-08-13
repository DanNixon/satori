{
  pkgs,
  rustPlatform,
  version,
  gitRevision,
  buildInputs,
  nativeBuildInputs,
}: rec {
  satori-event-processor = rustPlatform.buildRustPackage {
    pname = "satori-event-processor";
    version = version;

    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;

    nativeBuildInputs = nativeBuildInputs;
    buildInputs = buildInputs;

    cargoBuildFlags = ["--package satori-event-processor"];

    GIT_REVISION = gitRevision;

    # No need to do tests here, testing should have already been done earlier in CI pipeline
    doCheck = false;
  };

  satori-event-processor-container-image = pkgs.dockerTools.buildImage {
    name = "satori-event-processor";
    tag = "latest";
    created = "now";

    copyToRoot = pkgs.buildEnv {
      name = "image-root";
      paths = [pkgs.bashInteractive pkgs.coreutils];
      pathsToLink = ["/bin"];
    };

    config = {
      Entrypoint = ["${pkgs.tini}/bin/tini" "--" "${satori-event-processor}/bin/satori-event-processor"];
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
