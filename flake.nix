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

            echo "Rust development environment loaded"
            echo "Rust version: $(rustc --version)"
            echo ""
          '';
        };
      });
}
