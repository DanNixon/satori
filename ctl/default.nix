{
  pkgs,
  naersk',
  version,
  git_revision,
  buildInputs,
  nativeBuildInputs,
} : rec {
  package = naersk'.buildPackage {
    name = "satorictl";
    version = version;

    src = ./..;
    cargoBuildOptions = x: x ++ ["--package" "satorictl"];

    nativeBuildInputs = nativeBuildInputs;
    buildInputs = buildInputs;

    overrideMain = p: {
      GIT_REVISION = git_revision;
    };
  };

  container-image = pkgs.dockerTools.buildImage {
    name = "satorictl";
    tag = "latest";
    created = "now";

    copyToRoot = pkgs.buildEnv {
      name = "image-root";
      paths = [ pkgs.bashInteractive pkgs.coreutils ];
      pathsToLink = [ "/bin" ];
    };

    config = {
      Entrypoint = [ "${pkgs.tini}/bin/tini" "--" "${package}/bin/satorictl" ];
      Env = [
        "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      ];
    };
  };
}
