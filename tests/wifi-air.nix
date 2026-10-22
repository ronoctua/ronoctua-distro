# A Wi-Fi network inside a VM, for the checks that join one.
#
# `mac80211_hwsim` gives the machine two radios on one simulated air: the
# first (`wlan0`) is the access point — `hostapd` with three networks, and DHCP
# from dnsmasq — and the second (`wlan1`) is NetworkManager's, as a laptop's
# radio is. Nothing leaves the VM.
#
# THE NETWORKS are the two a home router offers — WPA2, and WPA3 in
# transition mode (both keys; its `wpa-psk` answered too) — and a third WPA2
# one. Not WPA3 alone: on this air not even `nmcli` completes SAE.
{ pkgs }:
rec {
  # WPA2 and WPA3 in transition.
  transition = "ronoctua-air";
  # WPA2 alone.
  wpa2 = "ronoctua-air-wpa2";
  # WPA2 too: a third network, for a join nobody asks to keep.
  guest = "ronoctua-air-guest";
  password = "a-test-passphrase";

  module = {
    boot.kernelModules = [ "mac80211_hwsim" ];
    services.hostapd = {
      enable = true;
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
    # The access point's side is not NetworkManager's.
    networking.networkmanager.unmanaged = [ "interface-name:wlan0*" ];
    # ITS DHCP: the firewall drops what reaches the access point's side, and
    # a joined radio would wait for an address that never comes.
    networking.firewall.trustedInterfaces = [
      "wlan0"
      "wlan0-1"
      "wlan0-2"
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
        exec ${pkgs.dnsmasq}/bin/dnsmasq --keep-in-foreground --bind-interfaces \
          --interface=wlan0 --interface=wlan0-1 --interface=wlan0-2 --port=0 --no-resolv \
          --dhcp-range=192.168.50.10,192.168.50.50 \
          --dhcp-range=192.168.51.10,192.168.51.50 \
          --dhcp-range=192.168.52.10,192.168.52.50 \
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
