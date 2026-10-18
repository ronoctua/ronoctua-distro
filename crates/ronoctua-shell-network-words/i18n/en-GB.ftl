# The texts `ronoctua-shell-network-words` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Power — the battery, the charger, the profile and sleep (LEADER s p)

echo-network-refused = The network refused: { $why }
# THE TABLE, from `assets/tor-options.nix` through the generator. A key nobody
# wrote draws as `[tor-opt-…]` on screen, which is why
# `tor-options-have-text` counts them.
tor-opt-socks-port = SOCKS address
tor-opt-cookie-authentication = cookie authentication
tor-opt-cookie-file = where the cookie is kept
tor-opt-cookie-group-readable = the tor group may read the cookie
tor-opt-exit-nodes = come out through
tor-opt-entry-nodes = go in through
tor-opt-exclude-nodes = never use
tor-opt-exclude-exit-nodes = never come out through
tor-opt-strict-nodes = treat those lists as a rule, not a preference
tor-opt-circuit-lifetime = keep using a circuit for
tor-opt-use-bridges = use bridges
tor-opt-bridge-lines = bridges
tor-opt-bridge-transport = transport
tor-opt-firewall-ports = ports this network lets out
tor-opt-fascist-firewall = only go out on 80 and 443
tor-opt-onion-directory = where the onion key is kept
tor-opt-onion-ports = ports this onion service answers
tor-opt-onion-single-hop = one hop only — fast, and not anonymous
tor-opt-safe-socks = refuse SOCKS requests that would leak the name
tor-opt-new-circuit-period = consider a new circuit every
tor-opt-circuit-build-timeout = give up building a circuit after
tor-opt-entry-guards = how many entry guards
tor-opt-use-entry-guards = keep going in through the same relays
tor-opt-distinct-subnets = keep a circuit's relays on different networks
tor-opt-ipv6 = use IPv6
tor-opt-prefer-ipv6 = prefer IPv6
tor-opt-connection-padding = cover traffic
tor-opt-reduced-padding = less cover traffic
tor-opt-avoid-disk-writes = write to disk as little as possible
tor-opt-dormant-timeout = go idle after
tor-opt-dormant-on-start = start idle
tor-opt-socks-proxy = reach the network through a SOCKS proxy
tor-opt-https-proxy = reach the network through an HTTPS proxy
tor-opt-log-level = what Tor writes to the log
tor-unit-seconds = seconds
tor-unit-guards = guards
tor-word-none = none
tor-word-obfs4 = obfs4
tor-word-snowflake = snowflake
tor-word-meek-lite = meek
tor-word-auto = as Tor decides
tor-word-0 = off
tor-word-1 = on
tor-word-notice-syslog = the usual
tor-word-info-syslog = more
tor-word-debug-syslog = everything
echo-network-login = open { $url } to finish logging in

## file info editing — everything the info panel can change

tunnel-refused-no-daemon = its daemon is not running
tunnel-refused-authority = its socket is not this session's to use
tunnel-refused-unit = its service would not start or stop
tunnel-refused-not-written = the file could not be written
tunnel-refused-config = Tor would not read its configuration
tunnel-refused-daemon = the daemon said no
tunnel-refused-other = it did not work
echo-tunnel-refused-line = Tor would not read this line: { $line }
