# An installed machine joins Wi-Fi networks, and remembers the ones it is told
# to — over a simulated air (`tests/wifi-air.nix`), with the distro's network
# module, the TPM's agent and a TPM (`swtpm`).
#
# WHAT ONLY THIS PROVES
# ---------------------
# NetworkManager asks a secret agent, at activation, for every password not
# kept in the profile itself — and the TPM's agent answers only with a blob.
# A join that hands the password over marked agent-owned or not-saved fails
# before the radio tries a key, and a "remember" that seals AFTER the profile
# asks races the agent. Only a radio shows it; `wifi-tpm-vm` has none. Here,
# with `wlan1` the machine's radio:
#
#   1. THE FIRST BOOT: the profile and the blob the installer writes for the
#      installed machine join through the agent; deleting that profile takes
#      its blob;
#   2. THE FOOTER, as the distro's user: a typed password joins in memory and
#      "keep it" seals it through the agent and writes the profile to disk
#      with no password in it, the link staying up; a wrong password is
#      refused and kept nowhere — and the remembered network's blob survives
#      it, and the network comes back through the agent;
#   3. THE BOOT'S CONSOLE, as root: joins and remembers, sealing itself;
#   4. a join nobody keeps, and then a REBOOT: the remembered networks come up
#      through the agent alone, and the one nobody kept is gone.
#
# The steps are the library's own calls, as the footer and the console make
# them: `#[ignore]`d tests of `ronoctua-network` (and one of the installer's),
# run here from their test harness with the network in the environment.
{
  pkgs,
  lib,
  networkConsole,
  networkAgent,
  # The test harnesses of `ronoctua-network` and `ronoctua-installer`
  # (`rust.test.testBins`).
  networkTests,
  installerTests,
}:

let
  air = import ./wifi-air.nix { inherit pkgs; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-wifi-joins";

  nodes.machine = {
    imports = [
      ../nix/modules/network.nix
      { options.ronoctua.user = lib.mkOption { type = lib.types.str; }; }
      { ronoctua.user = "noctua"; }
      air.module
    ];
    _module.args.ronoctua = { inherit networkConsole networkAgent; };
    # The distro's user, in the group NetworkManager answers.
    users.users.noctua = {
      isNormalUser = true;
      extraGroups = [ "networkmanager" ];
    };
    virtualisation = {
      tpm.enable = true;
      memorySize = 2048;
    };
    security.tpm2 = {
      enable = true;
      tctiEnvironment.enable = true;
    };
    # The blobs' folder; in a VM there is no `/persist` mounted.
    systemd.tmpfiles.rules = [
      "d /persist 0755 root root -"
      "d /persist/etc 0755 root root -"
    ];
  };

  testScript = ''
    TRANSITION, WPA2, GUEST, KEY = "${air.transition}", "${air.wpa2}", "${air.guest}", "${air.password}"

    def harness(tests, name):
        return machine.succeed(f"ls {tests}/bin/{name}-*").strip()

    def run(tests, name, test, ssid, user=None):
        """One `#[ignore]`d test, on `ssid`, as `user` (root when none)."""
        command = (
            f"env RONOCTUA_TEST_WIFI_SSID={ssid} RONOCTUA_TEST_WIFI_PASSWORD={KEY} "
            f"{harness(tests, name)} --ignored --exact {test} --nocapture --test-threads=1"
        )
        if user:
            command = f"runuser -u {user} -- {command}"
        print(machine.succeed(command + " 2>&1"))

    def blob(ssid):
        return f"/persist/etc/ronoctua/wifi/{ssid.encode().hex()}.cred"

    def on(ssid):
        machine.wait_until_succeeds(
            f"nmcli -t -f GENERAL.STATE,GENERAL.CONNECTION device show wlan1 | tr '\\n' ' ' | grep -q '^GENERAL.STATE:100 .*GENERAL.CONNECTION:{ssid} *$'",
            timeout=120,
        )

    def profiles():
        return machine.succeed("nmcli -t -f NAME,FILENAME connection show").splitlines()

    def boot():
        machine.wait_for_unit("air-dhcp.service")
        machine.wait_for_unit("ronoctua-network-agent.service")
        for ssid in (TRANSITION, WPA2, GUEST):
            machine.wait_until_succeeds(f"nmcli -t -f SSID dev wifi list --rescan yes | grep -qx {ssid}", timeout=90)

    machine.start()
    boot()

    with subtest("the installer's profile and blob join through the agent, as at the first boot"):
        run("${installerTests}", "ronoctua_installer", "plan::on_the_air::the_installed_wifi_is_written_here", TRANSITION)
        profile = "/etc/NetworkManager/system-connections/ronoctua-wifi.nmconnection"
        machine.succeed(f"grep -qx 'psk-flags=1' {profile}")
        machine.fail(f"grep -q '^psk=' {profile}")
        machine.succeed("nmcli connection reload")
        on(TRANSITION)
        # FORGOTTEN, it takes its blob: the profile left its password to the agent.
        machine.succeed(f"nmcli connection delete id {TRANSITION}")
        machine.wait_until_fails(f"test -e {blob(TRANSITION)}", timeout=30)

    with subtest("the footer joins in memory, and keeping it seals through the agent"):
        run("${networkTests}", "ronoctua_network", "on_the_air::the_footer_joins_then_remembers", WPA2, "noctua")
        on(WPA2)
        machine.succeed(f"test -e {blob(WPA2)}")
        kept = [p for p in profiles() if p.startswith(WPA2 + ":")]
        assert len(kept) == 1 and ":/etc/NetworkManager/" in kept[0], kept
        file = kept[0].split(":", 1)[1]
        machine.succeed(f"grep -qx 'psk-flags=1' '{file}'")
        machine.fail(f"grep -q '^psk=' '{file}'")

    with subtest("a wrong password is refused and kept nowhere, and the remembered one survives it"):
        run("${networkTests}", "ronoctua_network", "on_the_air::a_wrong_password_is_refused_and_kept_nowhere", WPA2, "noctua")
        machine.succeed(f"test -e {blob(WPA2)}")
        assert len([p for p in profiles() if p.startswith(WPA2 + ":")]) == 1, profiles()
        # AND IT COMES BACK through the agent, with nobody typing.
        on(WPA2)

    with subtest("the right password right after a wrong one comes up, however long the supplicant holds it"):
        run("${networkTests}", "ronoctua_network", "on_the_air::a_right_password_after_a_wrong_one_comes_up", TRANSITION, "noctua")
        on(TRANSITION)

    with subtest("the boot's console joins and remembers, as root, sealing itself"):
        run("${networkTests}", "ronoctua_network", "on_the_air::the_console_joins_then_remembers_as_root", TRANSITION)
        machine.succeed(f"test -e {blob(TRANSITION)}")

    with subtest("a join nobody keeps is gone after a reboot; the remembered come up through the agent"):
        run("${networkTests}", "ronoctua_network", "on_the_air::a_join_is_kept_for_this_boot_only", GUEST, "noctua")
        machine.fail(f"test -e {blob(GUEST)}")
        machine.shutdown()
        machine.start()
        boot()
        machine.wait_until_succeeds(
            "nmcli -t -f GENERAL.STATE device show wlan1 | grep -q '^GENERAL.STATE:100 '", timeout=120
        )
        after = profiles()
        print("\n".join(after))
        assert not [p for p in after if p.startswith(GUEST + ":")], after
        for ssid in (WPA2, TRANSITION):
            machine.succeed(f"nmcli --wait 90 connection up id {ssid} ifname wlan1")
            on(ssid)
  '';
}
