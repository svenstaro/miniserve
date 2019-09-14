# copied expressions from https://nixos.wiki/wiki/Rust
# and Mozilla's nix overlay README
let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
in
  with nixpkgs;
  stdenv.mkDerivation {
    name = "rust_nightly_shell";
    buildInputs = [
      nixpkgs.latest.rustChannels.nightly.rust
      openssl
      # needed to correctly populate the
      # nix specific paths to openssl libraries
      pkgconfig
    ];
  }
