# THE TEST'S PARTS, as plain derivations the host and the VM evaluate alike.
#
# `tests/parts-vm.nix` evaluates this on the host, for the plan the system
# carries, and the VM evaluates the SAME file for the same `.drv` paths: a
# `.drv` cannot be handed to a VM's store without every output of its graph
# (AGENTS.md, the `exportReferencesGraph` trap), and these must arrive only
# by building, inside the VM, against the test's server.
#
# The programs come in as STORE PATHS (bash, curl, coreutils, the outputs
# both stores hold) and become opaque inputs here: the derivations name no
# derivation but their own download, so their closure is small and the same
# on both sides, and the VM builds them in the sandbox, as a machine would.
{
  system,
  bash,
  curl,
  coreutils,
}:

let
  opaque =
    path:
    builtins.appendContext path {
      ${path} = {
        path = true;
      };
    };
  sh = "${opaque bash}/bin/bash";
  get = "${opaque curl}/bin/curl";
  bin = "${opaque coreutils}/bin";

  # What the server serves for a part at a version: plain bytes, with no
  # store path in them (a download may not refer to the store).
  content = name: version: "${name} ${version}\n";
  file = name: version: "${name}-${version}.txt";

  download =
    name: version:
    derivation {
      name = "test-part-${name}-${version}-download";
      inherit system;
      builder = sh;
      # THE ADDRESS AS `fetchurl` SAYS IT (`urls`), which is what the
      # realizer reads to offer a file to be handed in. `&& exit 0`: the
      # script ends in bash itself (see below).
      urls = "http://server/${file name version}";
      args = [
        "-c"
        "${get} -fsS -o $out $urls && exit 0"
      ];
      outputHashMode = "flat";
      outputHashAlgo = "sha256";
      outputHash = builtins.hashString "sha256" (content name version);
    };

  # A part around the file `bytes`: its download, or a file of the store.
  partWith =
    name: version: bytes:
    derivation {
      name = "test-part-${name}-${version}";
      inherit system;
      builder = sh;
      args = [
        "-c"
        # THE SCRIPT ENDS IN BASH, never in a coreutils program: `bash -c`
        # execs its last command, coreutils closes standard error before it
        # exits, and Nix kills a builder whose log reached its end — a race
        # measured here as "builder failed due to signal 9", one build in
        # three. A stdenv builder ends in bash and never meets it.
        ''
          set -e
          ${bin}/mkdir -p $out/bin $out/lib/qt-6/plugins
          printf '#!${sh}\n${bin}/cat ${bytes}\n' > $out/bin/${name}
          ${bin}/chmod +x $out/bin/${name}
          # A plugin folder the view must never reach.
          ${bin}/touch $out/lib/qt-6/plugins/never.so
          exit 0
        ''
      ];
    };
  partOf = name: version: partWith name version (download name version);
in
{
  inherit content file;
  a1 = partOf "a" "1";
  a2 = partOf "a" "2";
  b1 = partOf "b" "1";
  c1 = partOf "c" "1";
  # Handed in by the person, and carried by a stick.
  d1 = partOf "d" "1";
  e1 = partOf "e" "1";
  # Never at its origin: only on the person's mirror.
  f1 = partOf "f" "1";
  # Carried by the machine already (an ISO's, a test's): seeded, never built.
  g1 = partWith "g" "1" (builtins.toFile "test-part-g-1" (content "g" "1"));
  # A PART OF THE PERSON'S, which the machine flake hands its realizer
  # (`personParts`): built in the VM, from nothing it downloads.
  person1 = partWith "person-good" "1" (
    builtins.toFile "test-person-good-1" (content "person-good" "1")
  );
  downloadOf = download;
}
