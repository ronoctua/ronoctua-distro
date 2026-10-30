# ── anchor: L25 ──
# THE ISO INSTALLS WITH NO NETWORK AT ALL, and the machine it installs boots.
#
# WHAT ONLY THIS PROVES
# ---------------------
# The image a person writes to a stick, booted with NO network card (`-nic
# none`: `/sys/class/net` holds `lo` alone), installs a machine that answers
# unlike every reference and probe of `nix/installer/offline.nix` — another
# name, disk, language, keyboard, locale and zone, the `full` profile with
# programs left out, and the hardware report of THIS VM — and the machine
# boots from its disk:
#
#   1. `ronoctua-install --execute` ends well: every `nix` it runs is offline
#      with no substituter, so anything the ISO did not carry would have
#      stopped it (the build before the disk names what);
#   2. the installed store holds no part (no Claude Code, no LibreOffice),
#      the programs left out are not in its parts plan and one kept is,
#      and the first boot's record and sealed passphrase are on `/persist`;
#   3. the second boot asks the disk's passphrase (typed on the serial line),
#      reaches the person's session, has no failed system unit, and the
#      belongings' first boot waits for the network instead of failing.
#
# The machines are the image and the disk, started by hand: the image has
# no test backdoor, so everything is said and typed on the serial console.
# A TPM (swtpm, its state kept across both boots) seals the belongings'
# passphrase.
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
  memory = 6144;
  cores = 4;
  kernel = "${cfg.system.build.kernel}/${cfg.system.boot.loader.kernelFile}";
  initrd = "${cfg.system.build.initialRamdisk}/${cfg.system.boot.loader.initrdFile}";
  params = lib.concatStringsSep " " (
    [ "init=${cfg.system.build.toplevel}/init" ] ++ cfg.boot.kernelParams ++ [ "console=ttyS0,115200" ]
  );
  ovmf = pkgs.OVMF.fd;

  # ANSWERS NO FIXTURE GIVES (tests/fixtures/machine*). The access is a
  # deploy key made on the ISO; the repository is never reached.
  passphrase = "offline-disk-passphrase";
  answers = {
    RONOCTUA_INST_LANGUAGE = "pt-BR";
    RONOCTUA_INST_DISK = "/dev/disk/by-id/virtio-target";
    RONOCTUA_INST_PROFILE = "full";
    RONOCTUA_INST_LEFT_OUT = "libreoffice,vlc,solidcraft";
    RONOCTUA_INST_KEYBOARD = "pt";
    RONOCTUA_INST_KEYBOARD_VARIANT = "";
    RONOCTUA_INST_COMPOSE_KEY = "compose:menu";
    RONOCTUA_INST_SYSTEM_LOCALE = "es_ES.UTF-8";
    RONOCTUA_INST_TIMEZONE = "America/Sao_Paulo";
    RONOCTUA_INST_HOSTNAME = "noctua-offline";
    RONOCTUA_INST_PASSWORD = "offline-password";
    RONOCTUA_INST_PASSPHRASE = passphrase;
    RONOCTUA_INST_WIFI = "";
    RONOCTUA_INST_BELONGINGS = "git@belongings.invalid:someone/belongings.git";
    RONOCTUA_INST_BELONGINGS_ACCESS = "/root/keys/deploy";
    RONOCTUA_INST_BELONGINGS_PASSPHRASE = "offline-belongings-passphrase";
  };

  base = lib.concatStringsSep " " [
    (qemuCommon.qemuBinary pkgs.qemu_test)
    "-m ${toString memory}"
    "-smp ${toString cores}"
    # NO NETWORK CARD: QEMU adds one unless told not to.
    "-nic none"
    "-drive if=pflash,format=raw,unit=0,readonly=on,file=${ovmf}/FV/OVMF_CODE.fd"
    "-chardev socket,id=chrtpm,path=$ROOM/tpm/sock"
    "-tpmdev emulator,id=tpm0,chardev=chrtpm"
    "-device tpm-tis,tpmdev=tpm0"
  ];

  test = pkgs.testers.runNixOSTest {
    name = "ronoctua-installer-offline";
    nodes = { };
    # An offline install of the full profile copies its whole closure from
    # the image; two boots on top.
    globalTimeout = 7200;
    testScript = ''
      import json
      import os
      import re
      import shutil
      import subprocess
      import tempfile
      import time

      ISO = "${iso}"
      ANSWERS = json.loads(r"""${builtins.toJSON answers}""")
      ROOM = tempfile.mkdtemp()
      os.makedirs(f"{ROOM}/tpm")
      shutil.copy("${ovmf}/FV/OVMF_VARS.fd", f"{ROOM}/vars.fd")
      os.chmod(f"{ROOM}/vars.fd", 0o644)
      subprocess.run(["${pkgs.qemu_test}/bin/qemu-img", "create", "-q", "-f", "qcow2",
                      f"{ROOM}/target.qcow2", "64G"], check=True)


      def tpm():
          """The TPM, over its state on disk: swtpm ends when its VM lets go,
          so each boot starts one — the same TPM, with what it sealed."""
          sock = f"{ROOM}/tpm/sock"
          if os.path.exists(sock):
              os.unlink(sock)
          process = subprocess.Popen(["${pkgs.swtpm}/bin/swtpm", "socket", "--tpm2",
                                      "--tpmstate", f"dir={ROOM}/tpm",
                                      "--ctrl", f"type=unixio,path={sock}"])
          for _ in range(100):
              if os.path.exists(sock):
                  return process
              time.sleep(0.1)
          raise Exception("swtpm made no socket")


      def command(*more: str) -> str:
          return " ".join(["${base}".replace("$ROOM", ROOM),
                           f"-drive if=pflash,format=raw,unit=1,file={ROOM}/vars.fd",
                           f"-drive file={ROOM}/target.qcow2,if=none,id=target,format=qcow2,cache=unsafe",
                           "-device virtio-blk-pci,drive=target,serial=target", *more])


      def wait_for(machine, pattern: str, seconds: int, refused: str | None = None) -> re.Match:
          deadline = time.monotonic() + seconds
          while True:
              said = machine.get_console_log()
              if refused is not None and re.search(refused, said):
                  raise Exception(f"the console said {refused!r} while waiting for {pattern!r}")
              found = re.search(pattern, said, re.S)
              if found:
                  return found
              if time.monotonic() > deadline:
                  raise Exception(f"{pattern!r} was not on the console after {seconds} s")
              time.sleep(2)


      RAN = [0]


      def run(machine, line: str, seconds: int = 120) -> str:
          """`line` typed on the serial shell; what it printed between two
          markers, after its status was 0."""
          RAN[0] += 1
          mark = f"RAN{RAN[0]}"
          machine.send_console(f"echo {mark}-START; {line}; echo {mark}-END=$?\n")
          found = wait_for(machine, rf"{mark}-START\r?\n(.*?){mark}-END=(\d+)", seconds)
          # A console's colours and carriage returns are not what was said.
          said = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", found.group(1)).replace("\r", "")
          assert found.group(2) == "0", f"`{line}` ended with {found.group(2)}:\n{said}"
          return said


      swtpm = tpm()
      with subtest("the ISO, with no network card, installs a machine no fixture describes"):
          live = create_machine(command(
              "-kernel ${kernel}", "-initrd ${initrd}", "-append '${params}'",
              "-device qemu-xhci",
              f"-drive id=stick,if=none,readonly=on,format=raw,file={ISO}",
              "-device usb-storage,drive=stick"), name="live")
          live.start()
          try:
              # THE CONSOLE'S AUTOMATIC LOGIN, in whatever language the
              # system speaks, never a prompt's look: `run` finds its way by
              # the markers it echoes.
              wait_for(live, r"login: root \(", 600, refused=r"emergency mode")
              nics = run(live, "ls /sys/class/net")
              assert nics.split() == ["lo"], f"the live system has a network: {nics!r}"
              run(live, "mkdir -p /root/keys && ssh-keygen -q -t ed25519 -N ''' -f /root/keys/deploy")
              written = "".join(f"{k}={v}\n" for k, v in ANSWERS.items())
              live.send_console(f"cat > /tmp/answers <<'ANSWERS'\n{written}ANSWERS\n")
              run(live, "ronoctua-install --execute --answers /tmp/answers </dev/null >/tmp/install.log 2>&1"
                        " || { tail -n 60 /tmp/install.log; false; }", 5400)
              parts = run(live, "ls /mnt/nix/store | grep -cE -- '-(claude-code|libreoffice)-[0-9]' || true")
              assert parts.strip() == "0", f"a part is in the installed store: {parts!r}"
              # THE PROGRAMS LEFT OUT ARE NOT WANTED, and one kept is: the
              # installed system's parts plan, read through its own profile
              # (the system was built in this live store, so its paths are here).
              plan = run(live, "cat $(readlink -f /mnt/nix/var/nix/profiles/system-1-link)/etc/ronoctua/parts-plan.json")
              for gone in ("libreoffice", "vlc", "solidcraft"):
                  assert f'"{gone}"' not in plan, f"{gone} was left out and is in the plan"
              assert '"blender"' in plan, "blender was kept and is not in the plan"
              left = run(live, "ls /mnt/persist/etc/ronoctua/first-boot")
              assert "record.json" in left and ".cred" in left, f"the first boot has nothing to finish: {left!r}"
              run(live, "grep -q '\"address\": \"git@belongings.invalid' /mnt/persist/etc/ronoctua/first-boot/record.json")
              run(live, "! grep -rqs offline-belongings-passphrase /mnt/persist /mnt/home /mnt/etc/nixos")
              live.send_console("poweroff\n")
              live.wait_for_shutdown()
          finally:
              live.crash()

      swtpm.terminate()
      swtpm.wait()
      swtpm = tpm()
      with subtest("the machine boots from its disk, offline, to the person's session"):
          booted = create_machine(command(
              "-vga std",
              # A comma inside a QEMU option's value is written twice.
              "-smbios type=11,value=io.systemd.boot.kernel-cmdline-extra=console=ttyS0,,115200",
              "-smbios type=11,value=io.systemd.stub.kernel-cmdline-extra=console=ttyS0,,115200"),
              name="booted")
          booted.start()
          try:
              # THE DISK'S PASSPHRASE, typed on the serial line. The prompt
              # ends no line, and the driver's console log holds whole lines
              # only: it is typed again until the initrd says the disk opened.
              # THE DISK'S PASSPHRASE, typed on the serial line. The prompt
              # ends no line, and the driver's console log holds whole lines
              # only: it is typed once the boot manager starts the system, and
              # again only if the session's console does not come.
              wait_for(booted, r"Linux Boot Manager", 300)
              time.sleep(20)
              for _ in range(3):
                  booted.send_console("${passphrase}\n")
                  try:
                      wait_for(booted, r"noctua-offline login: noctua \(", 300, refused=r"emergency mode")
                      break
                  except Exception as e:
                      if "emergency mode" in str(e):
                          raise
              else:
                  raise Exception("the person's console never came")
              assert "[FAILED]" not in booted.get_console_log(), "a unit failed while booting"
              failed = run(booted, "systemctl --failed --no-legend --plain | cat")
              assert failed.strip() == "", f"system units failed:\n{failed}"
              run(booted, "systemctl is-active ronoctua-first-boot.timer")
              time.sleep(60)
              # TRIED, AND WAITING FOR THE NETWORK: no failure, the record kept.
              print(run(booted, "journalctl -b -u ronoctua-first-boot -o cat --no-pager | tail -n 20 || true"))
              state = run(booted, "systemctl show -p Result -p ExecMainStatus ronoctua-first-boot.service")
              assert "Result=success" in state and "ExecMainStatus=10" in state, state
              failed = run(booted, "systemctl --failed --no-legend --plain | cat")
              assert failed.strip() == "", f"system units failed once the first boot tried:\n{failed}"
              run(booted, "systemctl --user show -p ActiveState ronoctua-footer.service")
          finally:
              booted.crash()
      swtpm.terminate()
    '';
  };
in
# `nodes` is empty — the machines are the image and the disk, started by
# hand — so what the gate reads of a test's nodes is said here.
test
// {
  nodes.installer.virtualisation = {
    memorySize = memory;
    inherit cores;
  };
}
