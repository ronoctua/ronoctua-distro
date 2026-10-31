# A Wi-Fi network inside a VM, for the checks that join one.
#
# `mac80211_hwsim` gives the machine two radios on one simulated air: the
# first (`wlan0`) is the access point — `hostapd` with three networks, and DHCP
# from dnsmasq — and the second (`wlan1`) is NetworkManager's, as a laptop's
# radio is. Nothing leaves the VM.
#
# THE NETWORKS are the two a home router offers — WPA2, and WPA3 in
# transition mode (both keys; its `wpa-psk` answered too) — and a third WPA2
# one. Not WPA3 alone: on this air the 26.05 stack did not complete SAE even
# from `nmcli` (the unstable one does, below).
#
# AND THE TRANSITION NETWORK ON A SECOND BAND, taking WPA3 alone — what a
# router on three bands does on 6 GHz, where the standard requires it. A
# third radio (`wlan2`) on 5 GHz: the supplicant lists 5 GHz first at the
# same signal, as it listed the 6 GHz one first on a laptop, by its wider
# channel. A join that named the first access point of the name named this
# one, and NetworkManager refused the `wpa-psk` profile there before the
# radio tried a key ("Access point does not support PSK but setting
# requires it"): the installer said "check its password" of a right one
# (2026-10-25). The unstable stack completes SAE on this air, and
# NetworkManager takes the twin for its wider band when a profile fits it —
# so the twin hands out addresses as the router's other radio does
# (2026-10-31: with none, a right join waited for an address that never came).
{ pkgs }:
rec {
  # WPA2 and WPA3 in transition.
  transition = "ronoctua-air";
  # WPA2 alone.
  wpa2 = "ronoctua-air-wpa2";
  # WPA2 too: a third network, for a join nobody asks to keep.
  guest = "ronoctua-air-guest";
  password = "a-test-passphrase";

  # The transition network's WPA3-only twin, on 5 GHz: the address of the
  # radio it is on (`mac80211_hwsim` gives radio N `02:00:00:00:0N:00`).
  # NEVER NetworkManager's radio's (`…:01:00`): the access point then owns
  # the address of the station that asks it, and the unstable `hostapd`
  # refuses that station before any key ("not allowed to authenticate",
  # status 1) — a wrong password and a right one alike.
  twin = "02:00:00:00:02:00";

  module = {
    boot.kernelModules = [ "mac80211_hwsim" ];
    # THREE RADIOS: the access point's, NetworkManager's, and the twin's.
    boot.extraModprobeConfig = ''
      options mac80211_hwsim radios=3
    '';
    services.hostapd = {
      enable = true;
      radios.wlan2 = {
        band = "5g";
        # Not ACS, which could land on a DFS channel and wait a minute.
        channel = 36;
        countryCode = "US";
        networks.wlan2 = {
          ssid = transition;
          authentication = {
            mode = "wpa3-sae";
            saePasswords = [ { inherit password; } ];
          };
          bssid = twin;
          # NO TRANSITION DISABLE: the module's WPA3 mode tells a client to
          # stop using the network's other keys, and a router's 6 GHz radio
          # does not say that of the bands it shares a name with.
          settings.transition_disable = pkgs.lib.mkForce "0x00";
        };
      };
      radios.wlan0 = {
        band = "2g";
        countryCode = "US";
        networks = {
          wlan0 = {
            ssid = transition;
            authentication = {
              mode = "wpa3-sae-transition";
              saePasswords = [ { inherit password; } ];
              wpaPassword = password;
            };
            bssid = "02:00:00:00:00:00";
          };
          wlan0-1 = {
            ssid = wpa2;
            authentication = {
              mode = "wpa2-sha1";
              wpaPassword = password;
            };
            bssid = "02:00:00:00:00:01";
          };
          wlan0-2 = {
            ssid = guest;
            authentication = {
              mode = "wpa2-sha1";
              wpaPassword = password;
            };
            bssid = "02:00:00:00:00:02";
          };
        };
      };
    };
    # The access points' side is not NetworkManager's.
    networking.networkmanager.unmanaged = [
      "interface-name:wlan0*"
      "interface-name:wlan2*"
    ];
    # ITS DHCP: the firewall drops what reaches the access point's side, and
    # a joined radio would wait for an address that never comes.
    networking.firewall.trustedInterfaces = [
      "wlan0"
      "wlan0-1"
      "wlan0-2"
      "wlan2"
    ];
    systemd.services.air-dhcp = {
      description = "DHCP on the test's access point";
      after = [ "hostapd.service" ];
      requires = [ "hostapd.service" ];
      wantedBy = [ "multi-user.target" ];
      path = [ pkgs.iproute2 ];
      script = ''
        # The other networks' interfaces are hostapd's, and come with it.
        until ip link show wlan0-2 > /dev/null 2>&1; do sleep 0.2; done
        ip addr replace 192.168.50.1/24 dev wlan0
        ip addr replace 192.168.51.1/24 dev wlan0-1
        ip addr replace 192.168.52.1/24 dev wlan0-2
        ip addr replace 192.168.53.1/24 dev wlan2
        exec ${pkgs.dnsmasq}/bin/dnsmasq --keep-in-foreground --bind-interfaces \
          --interface=wlan0 --interface=wlan0-1 --interface=wlan0-2 --interface=wlan2 \
          --port=0 --no-resolv \
          --dhcp-range=192.168.50.10,192.168.50.50 \
          --dhcp-range=192.168.51.10,192.168.51.50 \
          --dhcp-range=192.168.52.10,192.168.52.50 \
          --dhcp-range=192.168.53.10,192.168.53.50 \
          --dhcp-leasefile=/run/air-dhcp.leases
      '';
    };
    # WPA_SUPPLICANT OVER D-BUS, as NetworkManager reaches it: the machines
    # and the ISO have it from NetworkManager's module (`networking.wireless`
    # evaluates true there), and with `hostapd` on it evaluates false here.
    services.dbus.packages = [ pkgs.wpa_supplicant ];
    systemd.packages = [ pkgs.wpa_supplicant ];
  };
}
