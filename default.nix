{ pkgs ? import <nixpkgs> {} }:

pkgs.stdenv.mkDerivation rec {
  name = "dev-shell";

  buildInputs = with pkgs; [
    protobuf
  ];
}