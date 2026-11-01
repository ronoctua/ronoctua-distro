# THE PARTS OF `parts-absent-vm`, as plain derivations the host and the VM
# evaluate alike — the `tests/parts-vm-parts.nix` way, for the same reason:
# a `.drv` cannot be handed to a VM's store without every output of its
# graph, and these must never be built anywhere but in the VM.
#
# None of them arrives. `broken` fails in its own builder, after a step it
# needs built; `offline` downloads from an address no network of the VM
# reaches.
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

  step =
    version:
    derivation {
      name = "test-absent-step-${version}";
      inherit system;
      builder = sh;
      # `exit 0`: the script ends in bash (`tests/parts-vm-parts.nix` says
      # why).
      args = [
        "-c"
        "${bin}/mkdir -p $out && exit 0"
      ];
    };

  brokenAt =
    version:
    derivation {
      name = "test-absent-broken-${version}";
      inherit system;
      builder = sh;
      args = [
        "-c"
        ''
          echo "building on ${step version}"
          echo "the planted failure: this part does not build" >&2
          exit 1
        ''
      ];
    };
in
{
  broken1 = brokenAt "1";
  broken2 = brokenAt "2";
  offline = derivation {
    name = "test-absent-offline";
    inherit system;
    builder = sh;
    urls = "http://192.0.2.1/offline.txt";
    args = [
      "-c"
      "${get} -fsS --connect-timeout 5 -o $out $urls && exit 0"
    ];
    outputHashMode = "flat";
    outputHashAlgo = "sha256";
    outputHash = builtins.hashString "sha256" "never served\n";
  };
}
