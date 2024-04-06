{
  # NOTE: This `flake.nix` is just an entrypoint into `package.nix`
  #       Where possible, all metadata should be defined in `package.nix` for non-flake consumers
  description = "interactive tool for selecting cutoff dates for smart_playlists in a beets configuration";

  inputs = {
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-23.11";
  };

  outputs = {
    # common
    self,
    flake-utils,
    nixpkgs,
    # rust
    rust-overlay,
    crane,
    advisory-db,
  }: let
    target_systems = [
      "x86_64-linux"
      # NOTE: `beets` in nixpkgs requires NIXPKGS_ALLOW_UNSUPPORTED_SYSTEM=1
      #       run once: NIXPKGS_ALLOW_UNSUPPORTED_SYSTEM=1 nix build nixpkgs#beets
      #       and then:  ln -s $(readlink result)/bin/beet beet
      #
      # This flake doesn't build `beets` directly, but rather accepts the path as a runtime argument
      "aarch64-darwin"
    ];
    arguments.parent_overlay = rust-overlay.overlays.default;
    arguments.for_package = {
      inherit
        advisory-db
        crane
        ;
      inherit (flake-utils.lib) mkApp;
    };
  in
    flake-utils.lib.eachSystem target_systems (
      system: let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [arguments.parent_overlay];
        };

        package = pkgs.callPackage ./nix/package.nix arguments.for_package;

        alejandra = pkgs.callPackage ./nix/alejandra.nix {};
      in {
        inherit (package) apps;

        checks =
          package.checks
          // alejandra.checks;

        packages = let
          inherit (package) crate-name;
        in {
          ${crate-name} = package.${crate-name};
          default = package.${crate-name};

          all-long-tests = pkgs.symlinkJoin {
            name = "all-long-tests";
            paths = [
              package.tests-ignored
            ];
          };
        };

        devShells = {
          default = package.devShellFn {
            packages = [
              pkgs.alejandra
              pkgs.bacon
              pkgs.cargo-expand
            ];
          };
        };
      }
    )
    // {
      overlays.default = final: prev: let
        # apply parent overlay
        parent_overlay = arguments.parent_overlay final prev;

        package = final.callPackage ./nix/package.nix arguments.for_package;
      in
        parent_overlay
        // {
          # NOTE: infinite recursion when using `${crate-name} = ...` syntax
          inherit (package) beet_smart_cutoff;
        };
    };
}
