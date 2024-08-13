{
  pkgs,
  rustPlatform,
  version,
  gitRevision,
  buildInputs,
  nativeBuildInputs,
}: rec {
  satori-agent = rustPlatform.buildRustPackage {
    pname = "satori-agent";
    version = version;

    src = ./..;
    cargoLock.lockFile = ../Cargo.lock;

    nativeBuildInputs = nativeBuildInputs ++ [pkgs.makeWrapper];
    buildInputs = buildInputs;

    cargoBuildFlags = ["--package satori-agent"];

    GIT_REVISION = gitRevision;

    # Ensure ffmpeg binary is available
    postInstall = ''
      wrapProgram $out/bin/satori-agent --prefix PATH : ${pkgs.lib.makeBinPath [pkgs.ffmpeg]}
    '';

    # No need to do tests here, testing should have already been done earlier in CI pipeline
    doCheck = false;
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
