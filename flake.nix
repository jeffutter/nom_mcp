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

        # Single source of truth for the crate version — read from the workspace
        # Cargo.toml so the flake never drifts out of sync with `cargo` itself.
        version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;

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
          commonArgs // { pname = "nom-mcp-workspace"; inherit version; }
        );

        # doCheck is off for both packages: CI's `test` job already runs the full
        # suite via `cargo nextest` on every push. Re-running it inside crane's
        # checkPhase is redundant, and for nom-mcp-remote it's actively broken —
        # reqwest's rustls backend inits a TLS connector (via rustls-platform-verifier,
        # OS trust store) eagerly on Client::builder().build(), even for plain-HTTP
        # requests, and Nix's build sandbox doesn't reliably expose a cert store.
        nom-mcp = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "nom-mcp";
            inherit version;
            cargoExtraArgs = "--bin nom-mcp";
            doCheck = false;
          }
        );

        nom-mcp-remote = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            pname = "nom-mcp-remote";
            inherit version;
            cargoExtraArgs = "--bin nom-mcp-remote";
            doCheck = false;
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
                cargo-release
                cargo-watch
                clang
                lefthook
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
                cargo-nextest
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
