{
  stdenvNoCC,
  lib,
  alejandra,
}: {
  checks = {
    nix-alejandra = stdenvNoCC.mkDerivation {
      name = "nix-alejandra";
      src = lib.cleanSourceWith {
        src = ./..;
        filter = path: type: ((type == "directory") || (lib.hasSuffix ".nix" path));
      };
      phases = ["buildPhase"];
      nativeBuildInputs = [alejandra];
      buildPhase = "(alejandra -qc $src || alejandra -c $src) > $out";
    };
  };
}
