# THE STAND-IN FOR AN APPLY'S BUILD, for the VM tests of the boot trial
# (`tests/boot-trial-vm.nix`, `tests/rescue-rollback-vm.nix`,
# `tests/secureboot-trial-vm.nix`): building a NixOS system inside the VM,
# offline, is not what they are about.
#
# `nix build … --out-link OUT` links OUT to the system the test names in
# `RONOCTUA_TEST_BUILDS` (the apply hands its environment down); `nix-env`
# beside it is the machine's own. Everything after the build — `nix-env`,
# `switch-to-configuration`, the boot loader's own builder, `bootctl`, the
# firmware, the bless — is the real one.
{ pkgs }:

nix:
pkgs.runCommand "nix-that-builds-what-the-test-says" { } ''
  mkdir -p $out/bin
  ln -s ${nix}/bin/nix-env $out/bin/nix-env
  cat > $out/bin/nix <<'EOF'
  #!${pkgs.runtimeShell}
  out="" prev=""
  for a in "$@"; do
    if [ "$prev" = --out-link ]; then out=$a; fi
    prev=$a
  done
  exec ${pkgs.coreutils}/bin/ln -sfn "$RONOCTUA_TEST_BUILDS" "$out"
  EOF
  chmod +x $out/bin/nix
''
