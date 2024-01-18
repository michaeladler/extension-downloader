{ pkgs ? import <nixpkgs> { } }:

with pkgs;

pkgs.callPackage ./pkg.nix { }
