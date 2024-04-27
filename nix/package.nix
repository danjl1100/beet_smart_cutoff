{
  pkgs,
  crane,
  system,
  advisory-db,
  mkApp,
}: let
  # crate
  crate-name = "beet_smart_cutoff";

  rustChannel = "beta";
  rustVersion = "latest";
  rustToolchain = pkgs.rust-bin.${rustChannel}.${rustVersion}.default;
  rustToolchainForDevshell = rustToolchain.override {
    extensions = ["rust-analyzer" "rust-src"];
  };
  craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
  craneLibForDevShell = (crane.mkLib pkgs).overrideToolchain rustToolchainForDevshell;

  installOnlyBin = bin-name: "mkdir -p $out/bin; cp target/release/${bin-name} $out/bin/";

  crate = pkgs.callPackage ./crate.nix {
    inherit system advisory-db craneLib;
    src = let
      extFilter = path: _type:
        builtins.any (ext: builtins.match ".*${ext}$" path != null) [
          "html"
          "snap"
          "txt"
        ];
      extsOrCargo = path: type:
        (extFilter path type) || (craneLib.filterCargoSources path type);
    in
      pkgs.lib.cleanSourceWith {
        src = craneLib.path ./..;
        filter = extsOrCargo;
      };
    extraBuildArgs = {
      installPhaseCommand = installOnlyBin crate-name;
    };
  };

  drv-open-doc = let
    open-cmd =
      if pkgs.stdenv.isDarwin
      then "open"
      else "${pkgs.xdg-utils}/bin/xdg-open";
    dash-to-underscores = input: builtins.replaceStrings ["-"] ["_"] input;
  in {
    for-crate = crate-name:
      pkgs.writeShellApplication {
        name = "open-doc-${crate-name}";
        text = ''
          echo "Opening docs for crate \"${crate-name}\""
          ${open-cmd} "file://${crate.doc}/share/doc/${dash-to-underscores crate-name}/index.html"
        '';
      };
    for-crate-deps = crate-name:
      pkgs.writeShellApplication {
        name = "open-doc-${crate-name}";
        text = ''
          echo "Opening docs for crate \"${crate-name}\""
          ${open-cmd} "file://${crate.doc-deps}/share/doc/${dash-to-underscores crate-name}/index.html"
        '';
      };
    for-std = toolchainWithRustDoc:
      pkgs.writeShellApplication {
        name = "open-doc-std";
        text = ''
          echo "Opening docs for rust std..."
          ${open-cmd} file://${toolchainWithRustDoc}/share/doc/rust/html/std/index.html
        '';
      };
    inherit open-cmd;
  };

  app-default = mkApp {
    drv = crate.package;
  };
  apps-doc = {
    rust-doc = mkApp {
      drv = drv-open-doc.for-crate crate-name;
    };
    rust-doc-deps = mkApp {
      drv = drv-open-doc.for-crate-deps crate-name;
    };
    rust-doc-std = mkApp {
      drv = drv-open-doc.for-std rustToolchainForDevshell;
    };
  };
  apps =
    apps-doc
    // {
      default = app-default;
    };
in {
  inherit crate-name;
  ${crate-name} = crate.package;

  inherit
    (crate)
    checks
    tests-ignored
    ;

  inherit
    apps
    app-default
    apps-doc
    ;

  devShellFn = inputs:
    craneLibForDevShell.devShell (inputs
      // {
        inherit (crate) checks;
      });
}
