{
  description = "geosuggest development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, rust-overlay, ... }:
    let
      source = ./.;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
          rust = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "clippy" "rust-analyzer" "rust-src" "rustfmt" ];
          };
        in
        f pkgs rust);
    in
    {
      devShells = forAllSystems (pkgs: rust: {
        default = pkgs.mkShell {
          packages = [
            rust
            pkgs.cargo-nextest
            pkgs.git-cliff
            pkgs.gnused # GNU sed used by `just bump`
            pkgs.just
          ];
        };
      });

      packages = forAllSystems (pkgs: rust:
        let
          # Build with a minimal toolchain (rustc, cargo, rust-std only).
          # The extended `rust` (clippy, rust-analyzer, rust-src, rustfmt)
          # stays in the dev shell. Binaries compiled with the extended
          # toolchain embed stdlib source paths that reference the whole
          # toolchain closure (~2GB extra in the container image).
          rustForBuild = pkgs.rust-bin.stable.latest.minimal;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustForBuild;
            rustc = rustForBuild;
          };

          topcoatCliSource = pkgs.fetchCrate {
            pname = "topcoat-cli";
            version = "0.10.0";
            hash = "sha256-lk13D/RqrskzhXwLNz+KEbWowllQqGxoGU47T4ZdBvk=";
          };

          topcoatCli = rustPlatform.buildRustPackage {
            pname = "topcoat-cli";
            version = "0.10.0";
            src = topcoatCliSource;
            cargoLock.lockFile = "${topcoatCliSource}/Cargo.lock";
            cargoBuildFlags = [ "--bin" "cargo-topcoat" ];
            doCheck = false;
          };

          geosuggest = rustPlatform.buildRustPackage {
            pname = "geosuggest";
            version = "0.9.0";
            src = source;
            cargoLock.lockFile = "${source}/Cargo.lock";
            cargoBuildFlags = [
              "--package" "geosuggest"
              "--package" "geosuggest-utils"
              "--features"
              "geosuggest/demo,geosuggest-utils/cli,geosuggest-utils/tracing"
            ];
            nativeBuildInputs = [ topcoatCli ];
            doCheck = false;
            postBuild = ''
              cargo-topcoat asset bundle --release \
                --package geosuggest-demo --bin geosuggest-demo-assets \
                --out "$TMPDIR/geosuggest-assets"
            '';
            postInstall = ''
              mkdir -p "$out/bin/assets"
              cp -R "$TMPDIR/geosuggest-assets/." "$out/bin/assets/"
            '';
          };

          cities5000 = builtins.fetchurl {
            url = "https://download.geonames.org/export/dump/cities5000.zip";
          };
          alternateNames = builtins.fetchurl {
            url = "https://download.geonames.org/export/dump/alternateNamesV2.zip";
          };
          admin1Codes = builtins.fetchurl {
            url = "https://download.geonames.org/export/dump/admin1CodesASCII.txt";
          };
          admin2Codes = builtins.fetchurl {
            url = "https://download.geonames.org/export/dump/admin2Codes.txt";
          };
          countryInfo = builtins.fetchurl {
            url = "https://download.geonames.org/export/dump/countryInfo.txt";
          };

          geosuggestIndex = pkgs.runCommand "geosuggest-index" {
            nativeBuildInputs = [ pkgs.unzip pkgs.gnused geosuggest ];
          } ''
            data_dir="$TMPDIR/geonames"
            mkdir -p "$data_dir" "$out"
            unzip -q ${cities5000} -d "$data_dir"
            unzip -q ${alternateNames} -d "$data_dir"
            cp ${admin1Codes} "$data_dir/admin1CodesASCII.txt"
            cp ${admin2Codes} "$data_dir/admin2Codes.txt"
            cp ${countryInfo} "$data_dir/countryInfo.txt"

            sed -i -e 's/UA\t\t11/RU\t\t192/' "$data_dir/cities5000.txt"
            sed -i -e 's/UA\t\t20/RU\t\t193/' "$data_dir/cities5000.txt"
            sed -i -e 's/UA\t\t05/RU\t\t194/' "$data_dir/cities5000.txt"
            sed -i -e 's/UA\t\t14/RU\t\t195/' "$data_dir/cities5000.txt"
            sed -i -e 's/UA\t\t08/RU\t\t196/' "$data_dir/cities5000.txt"
            sed -i -e 's/UA\t\t26/RU\t\t197/' "$data_dir/cities5000.txt"

            ${geosuggest}/bin/geosuggest-build-index from-files \
              --cities="$data_dir/cities5000.txt" \
              --names="$data_dir/alternateNamesV2.txt" \
              --admin-codes="$data_dir/admin1CodesASCII.txt" \
              --admin2-codes="$data_dir/admin2Codes.txt" \
              --countries="$data_dir/countryInfo.txt" \
              --languages=ru,uk,be,zh,ja \
              --output="$out/geosuggest-index.rkyv"
          '';

          settings = pkgs.writeTextDir "etc/geosuggest-settings.toml" ''
            host = "0.0.0.0"
            port = 8000
            index_file = "${geosuggestIndex}/geosuggest-index.rkyv"
            url_path_prefix = "/"
          '';

          demoImage = pkgs.dockerTools.buildLayeredImage {
            name = "geosuggest-demo";
            tag = "latest";
            contents = [ geosuggest geosuggestIndex settings ];
            config = {
              Entrypoint = [ "${geosuggest}/bin/geosuggest" ];
              Env = [
                "GEOSUGGEST_CONFIG_FILE=${settings}/etc/geosuggest-settings.toml"
                "RUST_LOG=geosuggest=trace"
              ];
              ExposedPorts = { "8000/tcp" = { }; };
            };
          };
        in
        {
          demo-image = demoImage;
          default = demoImage;
        });
    };
}
