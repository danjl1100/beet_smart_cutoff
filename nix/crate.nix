{
  pkgs,
  system,
  craneLib,
  advisory-db,
  extraBuildArgs ? {},
  commonArgOverrides ? {}, # includes cargoExtraArgs, cargoTestExtraArgs
  pname ? null,
  src ? null,
  srcDir ? ./.,
  isWasm ? false,
} @ inputs: let
  src =
    if (builtins.isNull inputs.src)
    then (craneLib.cleanCargoSource srcDir)
    else inputs.src;

  # Common arguments can be set here to avoid repeating them later
  commonArgs =
    {
      inherit src;

      buildInputs =
        [
          # Add additional build inputs here
        ]
        ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          # Additional darwin specific inputs can be set here
          pkgs.libiconv
          pkgs.darwin.apple_sdk.frameworks.CoreServices
        ];
    }
    // (
      if (builtins.isNull pname)
      then {}
      else {inherit pname;}
    )
    // commonArgOverrides;

  nextestArgs =
    commonArgs
    // {
      inherit cargoArtifacts;
      partitions = 1;
      partitionType = "count";
      # TODO: enable code coverage, only if it's worth it
      # } // pkgs.lib.optionalAttrs (system == "x86_64-linux") {
      #   # NB: cargo-tarpaulin only supports x86_64 systems
      #   # Check code coverage (note: this will not upload coverage anywhere)
      #   my-crate-coverage = craneLib.cargoTarpaulin (commonArgs // {
      #     inherit cargoArtifacts;
      #   });
    };

  # Build *just* the cargo dependencies, so we can reuse
  # all of that work (e.g. via cachix) when running in CI
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  # Build the actual crate itself, reusing the dependency
  # artifacts from above.
  package = craneLib.buildPackage (commonArgs
    // {
      inherit cargoArtifacts;
    }
    // extraBuildArgs);

  doc = craneLib.cargoDoc (commonArgs
    // {
      inherit cargoArtifacts;
    }
    // (
      if isWasm
      then {}
      else {
        cargoDocExtraArgs = "--workspace --no-deps"; # override default which is "--no-deps"
      }
    ));
  doc-deps = craneLib.cargoDoc (commonArgs
    // {
      inherit cargoArtifacts;
    }
    // (
      if isWasm
      then {}
      else {
        cargoDocExtraArgs = "--workspace"; # override default which is "--no-deps"
      }
    ));
in rec {
  checks = {
    # Build the crate as part of `nix flake check` for convenience
    inherit package;

    inherit doc;

    # Run clippy (and deny all warnings) on the crate source,
    # again, resuing the dependency artifacts from above.
    #
    # Note that this is done as a separate derivation so that
    # we can block the CI if there are issues here, but not
    # prevent downstream consumers from building our crate by itself.
    clippy = craneLib.cargoClippy (commonArgs
      // {
        inherit cargoArtifacts;
        # deny warnings (kinda strict, but let's see how it goes)
        cargoClippyExtraArgs = "--all-targets -- --deny warnings";
        # cargoClippyExtraArgs = "--all-targets";
      });

    # Check formatting
    fmt = craneLib.cargoFmt {
      inherit src;
    };

    # Audit dependencies
    audit = craneLib.cargoAudit {
      inherit src advisory-db;
    };

    # Run tests with cargo-nextest
    # Consider setting `doCheck = false` on `my-crate` if you do not want
    # the tests to run twice
    nextest = craneLib.cargoNextest nextestArgs;
  };

  # longer tests, not part of normal test suite
  tests-ignored = craneLib.cargoNextest (nextestArgs
    // {
      cargoNextestExtraArgs = "--run-ignored all";
    });

  inherit
    package
    doc
    doc-deps
    ;

  buildTrunkPackage = {
    pname,
    trunkIndexPath,
    ...
  } @ inputs:
    craneLib.buildTrunkPackage (commonArgs
      // inputs
      // {
        inherit cargoArtifacts;
      });
}
