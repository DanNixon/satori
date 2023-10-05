{
  pkgs,
  naersk',
  version,
  git_revision,
  buildInputs,
  nativeBuildInputs,
}: rec {
  satori-event-processor = naersk'.buildPackage {
    name = "satori-event-processor";
    version = version;

    src = ./..;
    cargoBuildOptions = x: x ++ ["--package" "satori-event-processor"];

    nativeBuildInputs = nativeBuildInputs;
    buildInputs = buildInputs;

    overrideMain = p: {
      GIT_REVISION = git_revision;
    };
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
