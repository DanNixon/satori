{
  pkgs,
  naersk',
  version,
  git_revision,
  buildInputs,
  nativeBuildInputs,
}: rec {
  satori-agent = naersk'.buildPackage {
    name = "satori-agent";
    version = version;

    src = ./..;
    cargoBuildOptions = x: x ++ ["--package" "satori-agent"];

    nativeBuildInputs = nativeBuildInputs ++ [pkgs.makeWrapper];
    buildInputs = buildInputs;

    # Ensure ffmpeg binary is available
    postInstall = ''
      wrapProgram $out/bin/satori-agent --prefix PATH : ${pkgs.lib.makeBinPath [pkgs.ffmpeg]}
    '';
    overrideMain = p: {
      GIT_REVISION = git_revision;
    };
  };

  satori-agent-container-image = let
    entrypoint = pkgs.writeShellApplication {
      name = "entrypoint";
      text = ''
        #!${pkgs.runtimeShell}
        mkdir -m 1777 /tmp
        ${satori-agent}/bin/satori-agent "$@"
      '';
    };
  in
    pkgs.dockerTools.buildImage {
      name = "satori-agent";
      tag = "latest";
      created = "now";

      copyToRoot = pkgs.buildEnv {
        name = "image-root";
        paths = with pkgs; [bashInteractive coreutils];
        pathsToLink = ["/bin"];
      };

      config = {
        Entrypoint = ["${pkgs.tini}/bin/tini" "--" "${entrypoint}/bin/entrypoint"];
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
