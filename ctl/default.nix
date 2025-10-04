{
  pkgs,
  rustPlatform,
  version,
  gitRevision,
}: rec {
  satorictl = rustPlatform.buildRustPackage {
    pname = "satorictl";
    version = version;

    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;

    cargoBuildFlags = ["--package satorictl"];

    GIT_REVISION = gitRevision;

    # No need to do tests here, testing should have already been done earlier in CI pipeline
    doCheck = false;
  };

  satorictl-container-image = pkgs.dockerTools.buildImage {
    name = "satorictl";
    tag = "latest";
    created = "now";

    copyToRoot = pkgs.buildEnv {
      name = "image-root";
      paths = [pkgs.bashInteractive pkgs.coreutils];
      pathsToLink = ["/bin"];
    };

    runAsRoot = ''
      #!${pkgs.runtimeShell}
      mkdir -p /config
      mkdir -p /data
    '';

    config = {
      Entrypoint = ["${pkgs.tini}/bin/tini" "--" "${satorictl}/bin/satorictl"];
      Env = [
        "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      ];
      WorkingDir = "/data";
      Volumes = {
        "/config" = {};
        "/data" = {};
      };
    };
  };
}
