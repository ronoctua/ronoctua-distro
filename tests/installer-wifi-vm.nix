# The installer joins a Wi-Fi network — the ISO's TUI, on the console, over a
# simulated air (`tests/wifi-air.nix`).
#
# WHAT ONLY THIS PROVES
# ---------------------
# The ISO runs no secret agent, and NetworkManager does not take a password
# marked agent-owned or not-saved from the call that adds a profile: it asks an
# agent at activation, finds none, and fails before the radio tries a key. For
# as long as the installer joined that way, every password read as wrong on a
# real machine — and no test saw it, because no test had a radio. Here:
#
#   1. a wrong password is refused, by NetworkManager itself, and its profile
#      does not stay behind;
#   2. the right one joins a WPA2 network and a WPA3-in-transition one, and
#      the installer goes on to the next question; the password lives in
#      NetworkManager's memory (`/run`), never on disk;
#   3. a network joined before the installer started (with `nmcli`, as a
#      person does with `nmtui`) is said on its row, and the question opens
#      on it;
#   4. the transition network has a twin on another band that takes WPA3
#      alone, listed first (`tests/wifi-air.nix`) — a router's 6 GHz radio —
#      and the right password still joins it: the join names no access
#      point, and NetworkManager takes one the profile fits. Naming the
#      first one of the name, the installer said "check its password" of a
#      right one on a router on three bands (2026-10-25).
#
# The screens are read off the console in English: the VM's console font has
# no accents. Each sentence is the `.ftl`'s, read by its key — a change of
# words does not break the check, a lost key does.
{
  pkgs,
  lib,
  installer,
}:

let
  air = import ./wifi-air.nix { inherit pkgs; };
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-installer-joins-wifi";

  nodes.machine = {
    imports = [
      # THE ISO'S NETWORK: the same module, with no agent.
      ../nix/modules/network.nix
      { options.ronoctua.user = lib.mkOption { type = lib.types.str; }; }
      { ronoctua.user = "noctua"; }
      air.module
    ];
    _module.args.ronoctua = {
      networkConsole = null;
      networkAgent = null;
    };
    ronoctua.network.nmAgent = false;
    services.getty.autologinUser = "root";
    virtualisation.memorySize = 2048;
    # A TPM and UEFI, so the installer opens on its question and not on the
    # screens that stop a machine without them.
    virtualisation.tpm.enable = true;
    virtualisation.qemu.options = [ "-bios ${pkgs.OVMF.fd}/FV/OVMF.fd" ];
    environment.systemPackages = [ installer ];
    # THE ANSWERS UP TO THE NETWORK, and the question to resume at.
    environment.etc."installer-answers".text = ''
      RONOCTUA_INST_LANGUAGE=en-GB
      RONOCTUA_INST_DISK=/dev/vda
      RONOCTUA_INST_PROFILE=minimal
      RONOCTUA_INST_KEYBOARD=gb
      RONOCTUA_INST_SYSTEM_LOCALE=en_GB.UTF-8
      RONOCTUA_INST_TIMEZONE=Europe/London
      RONOCTUA_INST_HOSTNAME=noctua-test
      RONOCTUA_INST_PASSWORD=Zq7-test-phrase
      RONOCTUA_INST_PASSPHRASE=Zq7-test-phrase
      RONOCTUA_INST_RESUME=inst-wifi
    '';
  };

  testScript = ''
    import re
    import time

    TEXTS = open("${../crates/ronoctua-installer/i18n/en-GB.ftl}").read()

    def said(key):
        """The key's sentence as the console shows it: its longest stretch
        with no placeholder and nothing past ASCII."""
        value = re.search(rf"^{re.escape(key)} = (.*)$", TEXTS, re.M)
        assert value, f"no `{key}` in the installer's texts"
        parts = re.split(r"\{[^}]*\}|[^\x20-\x7e]", value.group(1))
        return max((p.strip() for p in parts), key=len)

    NONE = said("inst-wifi-none")
    PASSWORD = said("inst-wifi-password")
    REFUSED = said("inst-gate-unconnected")[:30]
    UNJOINED = said("inst-gate-unjoined")[:30]
    NEXT = said("inst-belongings")
    CONNECTED = said("inst-wifi-connected")
    WPA2, TRANSITION, KEY, TWIN = "${air.wpa2}", "${air.transition}", "${air.password}", "${air.twin}"

    machine.start()
    machine.wait_for_unit("air-dhcp.service")
    machine.wait_for_unit("NetworkManager.service")
    for ssid in (WPA2, TRANSITION):
        machine.wait_until_succeeds(f"nmcli -t -f SSID dev wifi list --rescan yes | grep -qx {ssid}", timeout=90)
    # THE TWIN IN VIEW before any join: the case the laptop met.
    machine.wait_until_succeeds(f"nmcli -f BSSID dev wifi list --rescan yes | grep -qi {TWIN}", timeout=90)
    print(machine.succeed("nmcli -f BSSID,FREQ,SECURITY,SSID dev wifi list"))
    machine.wait_until_tty_matches("1", "root@")

    def start():
        machine.send_chars("clear; cp /etc/installer-answers /root/answers; ronoctua-install --answers /root/answers\n")
        machine.wait_until_tty_matches("1", NONE)
        machine.sleep(1)

    def leave():
        machine.send_key("ctrl-c")
        machine.wait_until_tty_matches("1", "root@")

    def choose(ssid):
        """The network's row, walked to from "no Wi-Fi", and Enter."""
        shown = [l.strip() for l in machine.get_tty_text("1").splitlines()]
        first = next(i for i, l in enumerate(shown) if NONE in l)
        rows = shown[first:]
        at = next(i for i, l in enumerate(rows) if l == ssid or l.startswith(ssid + " "))
        for _ in range(len(rows)):
            machine.send_key("up")
        for _ in range(at):
            machine.send_key("down")
        machine.send_key("ret")
        machine.wait_until_tty_matches("1", PASSWORD)

    def answer(password):
        """The password typed, and what the gate said: on to the next
        question, refused (the password), or unjoined (not the password)."""
        for _ in range(40):
            machine.send_key("backspace")
        machine.send_chars(password + "\n")
        began = time.monotonic()
        # The last refusal leaves the screen when the join is asked.
        while (REFUSED in machine.get_tty_text("1") or UNJOINED in machine.get_tty_text("1")) and time.monotonic() - began < 5:
            time.sleep(0.2)
        while time.monotonic() - began < 150:
            text = machine.get_tty_text("1")
            if NEXT in text:
                return "on", time.monotonic() - began
            if REFUSED in text:
                return "refused", time.monotonic() - began
            if UNJOINED in text:
                return "unjoined", time.monotonic() - began
            time.sleep(1)
        return "nothing", time.monotonic() - began

    def profiles():
        return machine.succeed("nmcli -t -f NAME,FILENAME connection show").splitlines()

    with subtest("a wrong password is refused, and its profile goes"):
        start()
        choose(WPA2)
        verdict, took = answer(KEY + "-not")
        print(f"a wrong password: {verdict} in {took:.0f} s")
        assert verdict == "refused", verdict
        assert not [p for p in profiles() if p.startswith(WPA2 + ":")], profiles()

    with subtest("the right password joins a WPA2 network, in memory"):
        verdict, took = answer(KEY)
        print(f"{WPA2}: {verdict} in {took:.0f} s")
        assert verdict == "on", verdict
        mine = [p for p in profiles() if p.startswith(WPA2 + ":")]
        assert len(mine) == 1 and "/run/NetworkManager/" in mine[0], mine
        machine.fail("grep -rqs 'psk=' /etc/NetworkManager/system-connections/")
        leave()

    with subtest("and a WPA3 network in transition, with its WPA3-only twin listed first"):
        start()
        choose(TRANSITION)
        verdict, took = answer(KEY)
        print(f"{TRANSITION}: {verdict} in {took:.0f} s")
        print(machine.succeed("nmcli -f ACTIVE,BSSID,FREQ,SSID dev wifi list | grep -i yes || true"))
        # WHY, when it did not join: what the installer said, what the radio
        # heard, and what NetworkManager did with the profile.
        if verdict != "on":
            print(machine.get_tty_text("1"))
            print(machine.succeed("nmcli -f BSSID,FREQ,SECURITY,SSID dev wifi list"))
            print(machine.succeed("journalctl -u NetworkManager -n 60 --no-pager -o cat"))
        assert verdict == "on", verdict
        leave()

    with subtest("a network joined before the installer is said, and the question opens on it"):
        machine.succeed("nmcli -t -f UUID connection show | xargs -r -n1 nmcli connection delete uuid")
        machine.wait_until_succeeds(f"nmcli -t -f SSID dev wifi list --rescan yes | grep -qx {WPA2}", timeout=90)
        machine.wait_until_succeeds(f"nmcli dev wifi connect {WPA2} password {KEY} ifname wlan1", timeout=180)
        start()
        machine.wait_until_tty_matches("1", CONNECTED)
        row = next(l for l in machine.get_tty_text("1").splitlines() if CONNECTED in l)
        assert row.strip().startswith(WPA2), row
        # ENTER ALONE takes it: the cursor opened on it, not on "no Wi-Fi".
        machine.send_key("ret")
        machine.wait_until_tty_matches("1", PASSWORD)
        verdict, took = answer(KEY)
        print(f"{WPA2}, joined before: {verdict} in {took:.0f} s")
        assert verdict == "on", verdict
        leave()
  '';
}
