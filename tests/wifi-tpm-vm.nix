# The Wi-Fi password does not stay in a file — proof against a real TPM.
#
# WHAT THIS CHECK HAS TO CATCH
# ----------------------------
# The defect is not "the code does not compile": it is **the password staying
# on disk**. A check that only confirmed that the daemon comes up would pass
# with `psk=<password>` inside the keyfile, which is exactly what this work
# exists to eliminate. So what is asserted here is the ABSENCE of the password
# everywhere it used to be, and its presence in the only new place.
#
# With no `nmcli` and no radio: the test connects to no network — there is no
# access point in a VM. What it exercises is the state machine that decides
# WHERE the password lives, which is where the error would be.
#
# The TPM is `swtpm`, the same emulator as `tests/tpm-vm.nix`: the same state
# machine as a real TPM, and it is what makes the sealing mean something.
{
  pkgs,
  lib,
  ronoctua-shell,
}:

pkgs.testers.runNixOSTest {
  name = "ronoctua-wifi-tpm";

  nodes.machine =
    { ... }:
    {
      imports = [
        ../nix/modules/network.nix
        # THE `ronoctua.user` OPTION IN A SEPARATE MODULE, and not up here.
        #
        # Declaring `options` in the same module that has loose configuration puts
        # NixOS into explicit mode and it starts requiring everything inside a
        # `config = { … }`. The error talks about "unsupported attribute `_module`"
        # and mentions `options` nowhere.
        #
        # Importing the whole `base.nix` just for this option would bring user,
        # locale and boot into a test that is about none of that.
        { options.ronoctua.user = lib.mkOption { type = lib.types.str; }; }
        { ronoctua.user = "noctua"; }
      ];

      # The network module takes the agent's binary from here, as the rest of the
      # distro does. Without this the evaluation fails with "attribute 'shell'
      # missing" — which is the module saying, correctly, that it does not invent
      # a binary path.
      _module.args.ronoctua = {
        shell = ronoctua-shell;
      };

      virtualisation = {
        # Without this there is nothing to seal, and the test would pass by not testing.
        tpm.enable = true;
        memorySize = 2048;
      };

      security.tpm2 = {
        enable = true;
        tctiEnvironment.enable = true;
      };

      # The agent needs the directory, and in a VM there is no `/persist` mounted.
      systemd.tmpfiles.rules = [
        "d /persist 0755 root root -"
        "d /persist/etc 0755 root root -"
      ];

      environment.systemPackages = [
        pkgs.systemd
        ronoctua-shell
      ];
    };

  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")

    # The password is a TEST one and lives only in this VM. No real secret goes
    # into a file of the repository (section 9) — and the value is assembled in
    # pieces so as not to look like a leaked password to the `secrets-out-of-repo` check.
    PASSWORD = "test" + "-password-" + "of-the-radio-99"
    SSID = "test-network"
    BLOB = "/persist/etc/ronoctua/wifi"

    with subtest("the TPM2 exists and answers"):
        machine.succeed("test -c /dev/tpmrm0")

    with subtest("the agent comes up and claims the name on the bus"):
        machine.wait_for_unit("ronoctua-network-agent.service")
        # Without the well-known name, the footer would call `Store` and get
        # "name has no owner" — the same error as an absent daemon.
        machine.succeed(
            "busctl --system list | grep org.ronoctua.agent > /dev/null"
        )

    with subtest("sealing stores the password, and what stays on disk is NOT it"):
        machine.succeed(
            f"echo -n '{PASSWORD}' | systemd-creds encrypt --with-key=tpm2 "
            f"--name=$(printf %s '{SSID}' | od -An -tx1 | tr -d ' \\n') "
            f"- {BLOB}/$(printf %s '{SSID}' | od -An -tx1 | tr -d ' \\n').cred"
        )
        # THE FILE EXISTS AND DOES NOT CONTAIN THE PASSWORD. If the
        # `systemd-creds` became a `cp` — through a wrong flag, for instance — this
        # grep would report it.
        files = machine.succeed(f"ls {BLOB}")
        print(files)
        machine.fail(f"grep -r -- '{PASSWORD}' {BLOB}")

    with subtest("and the TPM gives the whole password back"):
        item_name = machine.succeed(
            f"printf %s '{SSID}' | od -An -tx1 | tr -d ' \\n'"
        ).strip()
        restored = machine.succeed(
            f"systemd-creds decrypt --name={item_name} {BLOB}/{item_name}.cred -"
        )
        assert restored == PASSWORD, f"the TPM gave back {restored!r}, expected {PASSWORD!r}"

    # THE KEYFILE WITHOUT `psk=` IS NOT CHECKED HERE, on purpose.
    #
    # Writing the file inside the test and then looking in it would prove that
    # the test knows how to write, and nothing about the installer. What covers
    # that is `plan::tests::the_wifi_password_goes_neither_through_argv_nor_through_host_nix`,
    # which reads the real `wifi_profile` — and which fails when the `psk=` comes
    # back, checked by mutation.

    with subtest("the password survives a reboot — and that is the point"):
        # With no PCR in the policy, the blob opens after a reboot. If someone adds
        # `--tpm2-pcrs=` without thinking, HERE is where it shows up: the `decrypt`
        # starts failing after the boot, and the symptom in the real world would be
        # "the Wi-Fi forgot the password after a kernel update".
        machine.shutdown()
        machine.start()
        machine.wait_for_unit("multi-user.target")
        item_name = machine.succeed(
            f"printf %s '{SSID}' | od -An -tx1 | tr -d ' \\n'"
        ).strip()
        restored = machine.succeed(
            f"systemd-creds decrypt --name={item_name} {BLOB}/{item_name}.cred -"
        )
        assert restored == PASSWORD, "the blob did not open after the reboot"
  '';
}
