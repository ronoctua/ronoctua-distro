# The installer ISO, booted as it ships, from a USB stick.
#
# WHAT ONLY THIS PROVES
# ---------------------
# Every other test boots a system the test framework makes; none had booted the
# image a person writes to a stick. A copy of it onto a Ventoy stick lost its
# last 2.6 GiB (the stick was pulled before the kernel wrote it back), kept the
# name and the length, booted — the kernel and the initrd sit near the start —
# and stopped in an emergency shell at `Failed to mount
# /sysroot/nix/.ro-store`, saying nothing of why. Here, the image itself, on
# an emulated USB stick:
#
#   1. whole, its initrd checks the store against the sha256 the image
#      carries, says so, and the boot reaches the installer's login;
#   2. with the store's tail overwritten — a qcow2 over the image, never a
#      copy of it — the initrd says the stick does not hold the ISO whole and
#      stops before the store is mounted.
#
# The kernel, the initrd and the command line are the image's own (its
# boot menu's first entry), handed to QEMU directly so the console is the
# serial line: the boot menu itself is the firmware's and GRUB's, and what
# broke is what comes after it.
{
  pkgs,
  lib,
  installer,
}:

let
  cfg = installer.config;
  iso = "${cfg.system.build.isoImage}/iso/${cfg.image.fileName}";
  qemuCommon = import "${pkgs.path}/nixos/lib/qemu-common.nix" {
    inherit lib;
    inherit (pkgs) stdenv;
  };
  # THE VM IT BOOTS, which the gate sizes by what a test's nodes declare.
  memory = 2048;
  cores = 2;
  kernel = "${cfg.system.build.kernel}/${cfg.system.boot.loader.kernelFile}";
  initrd = "${cfg.system.build.initialRamdisk}/${cfg.system.boot.loader.initrdFile}";
  params = lib.concatStringsSep " " (
    [ "init=${cfg.system.build.toplevel}/init" ] ++ cfg.boot.kernelParams ++ [ "console=ttyS0,115200" ]
  );
  qemu = lib.concatStringsSep " " [
    (qemuCommon.qemuBinary pkgs.qemu_test)
    "-m ${toString memory}"
    "-smp ${toString cores}"
    "-kernel ${kernel}"
    "-initrd ${initrd}"
    "-append '${params}'"
    "-device qemu-xhci"
  ];

  test = pkgs.testers.runNixOSTest {
    name = "ronoctua-iso-checks-its-stick";
    nodes = { };
    # Two boots of ~25 s each; whatever hangs ends here, not at the hour.
    globalTimeout = 900;
    testScript = ''
      import re
      import subprocess
      import tempfile
      import time

      ISO = "${iso}"


      def stick(drive: str) -> str:
          return f"${qemu} -drive id=stick,if=none,readonly=on,{drive} -device usb-storage,drive=stick"


      # PAST THE CHECK: any of these on a broken stick is the check letting
      # it through, and is said at once rather than waited out.
      PAST = r"The stick holds this ISO whole|Mounted .*/sysroot/nix/\.ro-store|Reached target .*Initrd Default|login:"


      def booted(drive: str, name: str):
          machine = create_machine(stick(drive), name=name)
          machine.start()
          return machine


      def wait_for(machine, pattern: str, seconds: int, refused: str | None = None) -> float:
          """Until `pattern` is on the console: refused at once if `refused`
          comes first, and after `seconds` if nothing does."""
          deadline = time.monotonic() + seconds
          while True:
              said = machine.get_console_log()
              if refused is not None and re.search(refused, said):
                  raise Exception(f"the console said {refused!r} while waiting for {pattern!r}")
              if re.search(pattern, said):
                  return time.monotonic()
              if time.monotonic() > deadline:
                  raise Exception(f"{pattern!r} was not on the console after {seconds} s")
              time.sleep(1)


      # EVERY MACHINE IS STOPPED ON THE WAY OUT, a failure included: one
      # made by `create_machine` is not the driver's to stop, and a VM left
      # in the initrd's emergency shell kept the driver alive for hours.
      with subtest("whole, the stick is checked and the installer is reached"):
          whole = booted(f"format=raw,file={ISO}", "whole")
          try:
              began = wait_for(whole, r"Checking that the stick holds this ISO whole", 120,
                               refused=r"Mounted .*/sysroot/nix/\.ro-store|emergency mode")
              ended = wait_for(whole, r"The stick holds this ISO whole \(sha256 [0-9a-f]{64}\)", 300,
                               refused=r"does not hold this ISO whole|emergency mode")
              print(f"the medium check took {ended - began:.0f} s")
              wait_for(whole, r"${cfg.networking.hostName} login:", 300, refused=r"emergency mode")
          finally:
              whole.crash()

      with subtest("the store's tail lost, the initrd says so and stops before mounting it"):
          room = tempfile.mkdtemp()
          overlay = f"{room}/stick.qcow2"
          subprocess.run(["${pkgs.qemu_test}/bin/qemu-img", "create", "-q", "-f", "qcow2",
                          "-b", ISO, "-F", "raw", overlay], check=True)
          # WHERE THE STORE ENDS, from the image's own directory: it is one
          # file in two extents (it passes the 4 GiB of one ISO 9660 extent).
          report = subprocess.run(["${pkgs.xorriso}/bin/xorriso", "-indev", ISO, "-find",
                                   "/nix-store.squashfs", "-exec", "report_lba"],
                                  capture_output=True, text=True, check=True)
          ends = [(int(m.group(1)) + int(m.group(2))) * 2048
                  for m in re.finditer(r"File data lba:\s*\d+\s*,\s*(\d+)\s*,\s*(\d+)\s*,",
                                       report.stdout + report.stderr)]
          assert ends, f"xorriso did not report where the store is:\n{report.stdout}{report.stderr}"
          subprocess.run(["${pkgs.qemu_test}/bin/qemu-io", "-f", "qcow2", "-c",
                          f"write -P 0x55 {max(ends) - 2 * 1048576} 1M", overlay],
                         check=True, stdout=subprocess.DEVNULL)
          broken = booted(f"format=qcow2,file={overlay}", "broken")
          try:
              wait_for(broken, r"The stick does not hold this ISO whole: nix-store\.squashfs is not the one", 300,
                       refused=PAST)
              wait_for(broken, r"emergency mode", 120, refused=PAST)
              said = broken.get_console_log()
              assert "--into <the stick's folder>" in said, "the console does not say how to write the ISO again"
          finally:
              broken.crash()
    '';
  };
in
# `nodes` is empty — the machines are the image, started by hand — so what the
# gate reads of a test's nodes is said here, for the one VM at a time it runs.
test
// {
  nodes.stick.virtualisation = {
    memorySize = memory;
    inherit cores;
  };
}
