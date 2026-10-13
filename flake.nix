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
    };
}
