{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      crane,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        lib = nixpkgs.lib;
        craneLib = crane.mkLib pkgs;

        # Single source of truth for the Rust toolchain (rustc/cargo/rustfmt/clippy
        # all from the same rust-overlay release) — nixpkgs' own `cargo`/`rustc`
        # are deliberately not used alongside it, since mixing the two pulls in two
        # differently-versioned toolchains with no defined precedence between them.
        rustToolchain = pkgs.rust-bin.stable.latest.default;

        envVars =
          { }
          // (lib.attrsets.optionalAttrs pkgs.stdenv.isLinux {
            RUSTFLAGS = "-Clinker=clang -Clink-arg=--ld-path=${pkgs.mold}/bin/mold";
          });

        src = lib.cleanSourceWith { src = craneLib.path ./.; };

        commonArgs = (
          {
            inherit src;
            buildInputs =
              with pkgs;
              [
                clang
                rustToolchain
              ]
              ++ lib.optionals stdenv.isDarwin [ libiconv ];
          }
          // envVars
        );
        cargoArtifacts = craneLib.buildDepsOnly (
          commonArgs // { pname = "nom-mcp-workspace"; version = "0.1.0"; }
        );

        nom-mcp = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "nom-mcp";
            version = "0.1.0";
            cargoExtraArgs = "--bin nom-mcp";
          }
        );

        nom-mcp-remote = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "nom-mcp-remote";
            version = "0.1.0";
            cargoExtraArgs = "--bin nom-mcp-remote";
          }
        );
      in
      with pkgs;
      {
        packages = {
          default = nom-mcp;
          inherit nom-mcp nom-mcp-remote;
        };

        devShells = {
          # Full local-dev shell: compiler toolchain plus editor/workflow tooling.
          default = mkShell (
            {
              packages = [
                cargo-audit
                cargo-nextest
                cargo-watch
                clang
                rust-analyzer
                rustToolchain
              ];
            }
            // envVars
          );

          # Lean CI shell: compiler toolchain only, no editor/dev-workflow tools,
          # so CI jobs realize a smaller, unambiguous closure.
          ci = mkShell (
            {
              packages = [
                clang
                rustToolchain
              ]
              ++ lib.optionals stdenv.isDarwin [ libiconv ];
            }
            // envVars
          );
        };

        formatter = nixpkgs-fmt;
      }
    );
}
