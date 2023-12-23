{
  description = "mysql-binlog-repro dev environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        meta = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).package;
        inherit (meta) name version;

        # Filter inputs to only those necessary for the build
        inputTexts = path: _type: builtins.match ".*[txt|md]$" path != null;
        inputTextsOrCargo = path: type:
          (inputTexts path type) || (craneLib.filterCargoSources path type);

        craneLib = crane.lib.${system};

        # Common derivation arguments used for all builds
        commonArgs = {
          src = nixpkgs.lib.cleanSourceWith {
            src = craneLib.path ./.; # The original, unfiltered source
            filter = inputTextsOrCargo;
          };
          strictDeps = true;

          buildInputs = [
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];

          nativeBuildInputs = with pkgs; [
            pkg-config
            openssl
          ];

          # Additional environment variables can be set directly
          # MY_CUSTOM_VAR = "some value";
        };

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
          # Additional arguments specific to this derivation can be added here.
          # Be warned that using `//` will not do a deep copy of nested
          # structures
          pname = "binlog-deps";
        });

        mysql-binlog-repro-clippy = craneLib.cargoClippy (commonArgs // {
          # Again we apply some extra arguments only to this derivation
          # and not every where else. In this case we add some clippy flags
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-targets -- --deny warnings";
        });

        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        mysql-binlog-repro = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
      in
      {
        checks = {
          inherit mysql-binlog-repro mysql-binlog-repro-clippy;
        };

        packages.default = mysql-binlog-repro;

        packages.mysql-binlog-repro = mysql-binlog-repro;

        apps.default = flake-utils.lib.mkApp {
          drv = mysql-binlog-repro;
        };

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};

          # Additional dev-shell environment variables can be set directly
          # MY_CUSTOM_DEVELOPMENT_VAR = "something else";
          packages = [
            pkgs.mysql80
          ];
        };
        
        packages.docker =
          let
            bin = "${mysql-binlog-repro}/bin/${name}";
          in
          pkgs.dockerTools.buildLayeredImage {
            inherit name;
            tag = "v${version}";

            config = {
              Entrypoint = [ bin ];
              ExposedPorts."8080/tcp" = { };
            };
          };
      });
}
