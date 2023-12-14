{
  pkgs,
  naersk',
  version,
  gitRevision,
  buildInputs,
  nativeBuildInputs,
}: rec {
  satori-web = naersk'.buildPackage {
    name = "satori-web";
    version = version;

    src = ./..;
    cargoBuildOptions = x: x ++ ["--package" "satori-web"];

    nativeBuildInputs = nativeBuildInputs ++ [pkgs.makeWrapper];
    buildInputs = buildInputs;

    overrideMain = p: {
      GIT_REVISION = gitRevision;
    };
  };

  satori-web-container-image =
    pkgs.dockerTools.buildImage {
      name = "satori-web";
      tag = "latest";
      created = "now";

      copyToRoot = pkgs.buildEnv {
        name = "image-root";
        paths = with pkgs; [bashInteractive coreutils];
        pathsToLink = ["/bin"];
      };

      config = {
        Entrypoint = ["${pkgs.tini}/bin/tini" "--" "${satori-web}/bin/satori-web"];
        ExposedPorts = {
          "8000/tcp" = {};
          "9090/tcp" = {};
        };
        Env = [
          "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
          "HTTP_SERVER_ADDRESS=0.0.0.0:8000"
          "OBSERVABILITY_ADDRESS=0.0.0.0:9090"
        ];
      };
    };
  }
