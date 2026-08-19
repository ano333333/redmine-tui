{
  description = "redmine-tui front draft";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let pkgs = nixpkgs.legacyPackages.${system};
      in {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustc
            cargo
            cargo-insta
            cargo-llvm-cov
            llvmPackages_latest.llvm
            clippy
          ];

          shellHook = ''
            export LLVM_COV=$(command -v llvm-cov)
            export LLVM_PROFDATA=$(command -v llvm-profdata)
            export REDMINE_PORT=''${REDMINE_PORT:-8080}
            export REDMINE_URL=''${REDMINE_URL:-http://127.0.0.1:$REDMINE_PORT}
            export REDMINE_API_KEY=''${REDMINE_API_KEY:-0123456789abcdef0123456789abcdef01234567}

            echo "Rust development environment loaded"
            echo "Rust version: $(rustc --version)"
            echo "Redmine URL: $REDMINE_URL"
            echo ""
          '';
        };
      });
}
