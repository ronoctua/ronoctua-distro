# The texts `ronoctua-shell-network` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Network — Wi-Fi and Ethernet (LEADER s n)

cmd-network-open = Networks
cmd-network-scan = Scan networks
cmd-network-connect = Connect to network
cmd-network-disconnect = Disconnect
cmd-network-forget = Forget network
cmd-network-wifi-toggle = Toggle Wi-Fi radio
cmd-network-link = Show the link
group-network = network
# THE VIEW. States the list shows in place of rows when there is no row to
# show — each says something different, and folding them into one generic
# "no networks" would hide the difference between not finding and not looking.
network-scanning = scanning…
network-empty = no networks in range
network-no-adapter = this machine has no Wi-Fi radio
network-radio-off = radio off
network-wired = wired
network-saved = saved
network-password = Password for { $network }
network-password-help = Kept by NetworkManager, outside the repository.
# ECHOES — what the minibuffer says after the action.
echo-network-connected = Connected to { $network }
echo-network-failed = Could not connect to { $network }: { $reason }
echo-network-disconnected = Disconnected
echo-network-forgotten = { $network } forgotten
echo-network-radio-on = Wi-Fi radio on
echo-network-radio-off = Wi-Fi radio off
echo-network-link = { $network } · { $band } · { $rate } · signal { $signal }
echo-network-link-wired = { $network } · { $rate }
echo-network-no-link = Not connected
echo-network-unavailable = NetworkManager is not responding

## Network — the password sealed in the TPM (psk-flags)

# The one-key question, after the connection succeeds.
network-remember = Keep the password of { $network } on this machine?
network-remember-no = no, store nothing
network-remember-yes = yes, seal it in the TPM
# The list marker: tells "saved in NetworkManager" apart from "the password exists".
network-no-tpm = in TPM
echo-network-stored = Password for { $network } sealed in the TPM
echo-network-no-stored = Connected. The password was not stored.
echo-network-store-failed = Connected, but I could not store the password: { $reason }

## Power — the battery, the charger, the profile and sleep (LEADER s p)

# THE WAY OUT — the three marks, and what the daemon believes.
network-way = the internet
network-airplane = aeroplane mode
network-tunnel-drop = RET takes it down
cmd-network-airplane = Aeroplane mode (toggle)
network-way-unknown = not decided yet
network-way-none = no way out
network-way-portal = a login page is in the way
network-way-limited = connected, but it is not answering
network-way-full = it answers
# THE VIEW — one list, four headings, and the same rows in both doors.
network-heading-overview = at a glance
network-heading-links = cable and Wi-Fi
network-heading-wifi = Wi-Fi
network-heading-remembered = remembered networks
network-no-remembered = this machine remembers no network it cannot see
network-out-of-range = not in range
network-heading-tunnels = tunnels
network-radio = Wi-Fi radio
network-on = on
network-off = off
network-no-link = this machine has no cable and no radio
network-no-traffic = nothing has gone in or out yet
network-no-tunnel = no tunnel here
network-tunnel-raise = RET brings it up
network-state-connected = connected
network-state-connecting = connecting…
network-state-disconnected = not connected
network-fact-network = Wi-Fi network
network-fact-band = Wi-Fi band
network-fact-signal = signal strength
network-fact-rate = link speed
network-fact-address = this machine's address
network-import-address = your address on it
network-fact-address-tailnet = IPv4 on the tailnet
network-fact-address-tailnet6 = IPv6 on the tailnet
network-fact-gateway = router
network-fact-dns = DNS server
network-fact-received = received
network-fact-received-boot = received since boot
network-fact-sent = sent
network-fact-sent-boot = sent since boot
network-heading-ports = open ports
network-heading-monitor = traffic
network-no-ports = nothing is listening
network-port-not-ours = not ours to see
network-ports-more = { $count ->
    [one] and one more of ours
   *[other] and { $count } more of ours
  }
network-per-second = { $bytes }/s
# THE UNITS ARE THE SAME IN EVERY LANGUAGE; the DECIMAL SEPARATOR is not, and
# that is why these are keys with a number in them rather than a string built
# in the crate.
network-rate-kilobits = { $rate } kb/s
network-rate-megabits = { $rate } Mb/s
network-rate-gigabits = { $rate } Gb/s
network-no-rate = not up
network-band = { $ghz } GHz
network-band-unknown = an unusual frequency
network-heading-lan = machines on this network
network-lan-nobody = nobody has been spoken to yet
network-lan-gateway = this network's router
network-lan-this-machine = this machine
network-lan-looking = looking for the others…
network-lan-answered = answered
network-lan-refused = this machine may not send a ping: the system does not let it open the socket
network-lan-swept = { $answered ->
    [one] one machine answered out of { $asked } asked, { $range }
   *[other] { $answered } machines answered out of { $asked } asked, { $range }
  }
cmd-network-lan = Look for the other machines here
network-lan = look
network-section-next = section
network-section-previous = section back
network-hotspot-start = share this connection
network-hotspot-stop = stop sharing
network-hotspot-cost = the radio leaves the network it is on: what it shares is the cable
network-hotspot-on = this machine is the network now
network-hotspot-password = password for this hotspot
echo-network-hotspot = sharing as { $network }, password { $password }
echo-network-hotspot-failed = could not share: { $reason }
cmd-network-import = Bring a VPN in from a file
cmd-network-section-next = Next section of the network view
cmd-network-section-previous = Previous section of the network view
cmd-network-tor-open = Tor
cmd-network-tor-more = Turn this Tor setting forwards
cmd-network-tor-less = Turn this Tor setting back
cmd-network-tor-back = Leave Tor's settings
network-import-file = file to bring in
network-import-kind = kind of VPN
network-import-wireguard = WireGuard
network-import-openvpn = OpenVPN
network-import-endpoint = goes to
network-import-dns = it would move your DNS to
network-import-everything = everything this machine sends would go through it
network-import-asks = it asks for a user and a password as soon as it is brought in, and keeps them
network-import-bring = bring it in
network-import-leave = leave it where it is
network-import-where = Path of the .conf or .ovpn file
network-import-help = Read before importing: the file decides where your traffic goes.
echo-network-imported = { $network } brought in
echo-network-not-a-tunnel = { $file } is neither a WireGuard nor an OpenVPN file
echo-network-import-failed = could not bring { $file } in: { $reason }
# THE TWO TUNNELS. Neither is on at rest: Tor does not start at boot and
# Tailscale needs a login, so a machine that uses neither draws neither
# heading.
network-heading-tailscale = Tailscale
network-heading-tor = Tor
network-tailscale-state = the tailnet connection
network-tailscale-running = up
network-tailscale-stopped = down
network-tailscale-needslogin = waiting for a login
network-tailscale-nostate = starting
network-tailscale-starting = starting
network-tailscale-needsmachineauth = waiting to be approved on the tailnet
network-tailscale-inuseotheruser = another person on this machine is using it
network-tailscale-login = log in
network-tailscale-logout = log out
network-tailscale-routes = accept tailnet routes
network-tailscale-shields = refuse what comes in
network-tailscale-online = online
network-tailscale-offline = offline
network-tailscale-exit = everything goes through here
network-tor-state = the Tor service
network-tor-ready = a circuit is up
network-tor-not-ready = no circuit
network-tor-bootstrap = starting, { $percent }%
network-tor-circuits = { $count ->
    [one] one circuit open
   *[other] { $count } circuits open
  }
network-tor-identity = ask for a new identity
# TOR, WHOLE. The section is drawn whether Tor is running or not, and that is
# the fix rather than a feature: it used to live behind "the control socket
# answered", which is only true once Tor is already up — so a stopped Tor had
# no heading, no row and no key, and the one switch that existed was written to
# STOP. There was no way in the whole shell to turn Tor on.
network-tor-running = running
network-tor-stopped = stopped
network-tor-moving = changing
network-tor-absent = not installed on this machine
network-tor-unknown = cannot be asked
network-tor-start = start it
network-tor-stop = stop it
network-tor-autostart = start it with the session
network-tor-settings = settings
network-tor-sheet = Tor
network-tor-browser = open a browser through Tor
# WHAT THE FLAKE OWNS. The row is drawn and reachable and it refuses, saying
# where the value comes from — the shape the format buffer's disabled XFS row
# already has. Editing it would be a key whose effect the generated torrc
# undoes before it is pressed.
network-tor-from-flake = set by the system's configuration
network-tor-heading-service = The service
network-tor-heading-path = Where it comes out
network-tor-heading-bridges = Bridges
network-tor-heading-onion = Onion services
network-tor-heading-advanced = Rarely touched
network-tor-advanced-about = These change how Tor builds circuits. The defaults are what most people want.
ask-tor-setting = The new value — empty clears it
ask-tor-lines = One line to add — empty clears them all
network-tor-back = back
network-tor-edit = change
network-tor-more = more
network-tor-less = less
network-tor-drawer = the drawer
echo-tor-written = Tor's settings are written
echo-tor-refused = Tor would not take that: { $reason }
echo-tor-not-running = written; Tor will read it when it starts
network-fact-name = name on the tailnet
network-fact-tailnet = your tailnet's name
network-fact-magic-dns = MagicDNS
network-fact-version = Tor's version
network-fact-socks = SOCKS address
# THE HINT LINE'S WORDS — the id read as a text key, short because the line
# is one line. The long form is `cmd-<id>`, which the manual reads.
network-up = up
network-down = down
network-scan = scan
network-wifi-toggle = radio
network-disconnect = disconnect
network-forget = forget
network-link = link
network-title = network
cmd-network-window = Network in a window
cmd-network-up = Up one row
cmd-network-down = Down one row
echo-network-tunnel-up = { $network } is coming up
echo-network-tunnel-down = { $network } is down
echo-network-deleted = { $network } deleted

## file picker — the one picker of a file, a folder or a name to save under

file-picker-filter-tunnels = Tunnels (.conf, .ovpn)
network-pick-tunnel = A tunnel to bring in

## file info editing — everything the info panel can change

# The network view: the gateway's round trip, measured on demand.
network-latency = round trip to the router
network-latency-measure = measure
network-latency-measuring = measuring…
network-latency-ms = { $fastest } to { $slowest } ms ({ $answered } of { $asked })
network-latency-silent = no answer
network-latency-refused = could not measure: { $error }
# The names of the machines on the network, from systemd-resolved.
network-lan-names-refused = the names are unknown: { $why }
lan-resolve-why-no-bus = the system bus could not be reached
lan-resolve-why-no-resolved = systemd-resolved is not running
lan-resolve-why-failed = systemd-resolved could not answer
# A VPN whose file asks for a user and a password.
ask-network-vpn-user = User for this VPN
ask-network-vpn-password = Password for this VPN
echo-network-vpn-stored = { $network } will come up without asking
echo-network-vpn-store-failed = the VPN's password was not stored: { $reason }
# A phone's link in the network view, and the network crate's own refusals.
network-fact-phone = phone
