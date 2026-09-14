# Every Tor setting this system offers, declared once.
#
# WHY A TABLE AND NOT ROWS OF RUST
# ---------------------------------
# The same argument `assets/syncthing-options.nix` makes, and the same
# generator reads both: written by hand on the screen side these would be
# decisions spread over a file nobody could audit, and the day a torrc option
# was renamed there would be no way to notice. Declared here, they produce the
# Rust table the view walks and the list of text keys both languages owe.
#
# THE FIELDS are `nix/scripts/gen-options.py`'s, and are documented in
# `assets/syncthing-options.nix` — `id`, `key`, `kind`, `least`/`most`, `unit`,
# `ring`, `fixed`, `advanced`.
#
# `key` IS THE TORRC OPTION NAME, spelled exactly as Tor spells it. It is
# written straight into the fragment, which is why
# `ronoctua_tunnel::tor::fragment::a_name` refuses anything that is not
# letters and digits: a key that could hold a space is a way to write a
# directive nobody asked for.
#
# `fixed = true` IS INVARIANT 6 ON THE SCREEN
# --------------------------------------------
# `nix/modules/tor.nix` declares these, and the generated torrc reads
# `%include` FIRST — so Tor takes the module's value whatever the fragment
# says. Offering them as editable would be offering a key whose effect is
# already undone before it is pressed. They are drawn, reachable, and they
# refuse, saying where the value comes from. It is the shape the format
# buffer's disabled XFS row already has.
#
# WHAT IS DELIBERATELY NOT HERE
# ------------------------------
# `DisableNetwork`, which is a switch that makes a running Tor reach nothing
# while still looking started — the view has a real on/off for the unit, and
# two switches meaning almost the same thing is how a person turns the wrong
# one. Relay and bridge-server settings: this machine is a CLIENT, and an
# accidental relay is bandwidth and legal exposure nobody asked a footer for.
# `TransPort` and transparent proxying, for the reason `nix/modules/tor.nix`
# gives. And `MapAddress`, which is a table and not a setting.
{
  # ---------------------------------------------------------------------
  # THE SERVICE — what the flake owns. Drawn, reachable, and refusing.
  # ---------------------------------------------------------------------
  service = [
    {
      id = "socks-port";
      key = "SOCKSPort";
      kind = "text";
      fixed = true;
    }
    {
      id = "cookie-authentication";
      key = "CookieAuthentication";
      kind = "switch";
      fixed = true;
    }
    {
      id = "cookie-file";
      key = "CookieAuthFile";
      kind = "text";
      fixed = true;
    }
    {
      id = "cookie-group-readable";
      key = "CookieAuthFileGroupReadable";
      kind = "switch";
      fixed = true;
    }
  ];

  # ---------------------------------------------------------------------
  # WHERE IT COMES OUT — the setting people actually come here for.
  # ---------------------------------------------------------------------
  path = [
    # COUNTRY CODES OR FINGERPRINTS, comma-separated, in Tor's own notation
    # (`{de},{nl}`). Not a ring: there are two hundred countries and the list
    # is a sentence, not a choice.
    {
      id = "exit-nodes";
      key = "ExitNodes";
      kind = "text";
    }
    {
      id = "entry-nodes";
      key = "EntryNodes";
      kind = "text";
    }
    {
      id = "exclude-nodes";
      key = "ExcludeNodes";
      kind = "text";
    }
    {
      id = "exclude-exit-nodes";
      key = "ExcludeExitNodes";
      kind = "text";
    }
    # THE ONE THAT CHANGES WHAT THE TWO ABOVE MEAN. Without it Tor treats the
    # lists as a preference and leaves anyway when it has to; with it they are
    # a rule, and a country list that cannot be satisfied means no circuit at
    # all. The screen says so, because "Tor stopped working" is what it looks
    # like otherwise.
    {
      id = "strict-nodes";
      key = "StrictNodes";
      kind = "switch";
    }
    # HOW LONG A CIRCUIT CARRIES NEW STREAMS. Ten minutes is Tor's default;
    # shorter is more unlinkable and more handshakes.
    {
      id = "circuit-lifetime";
      key = "MaxCircuitDirtiness";
      kind = "number";
      least = 10;
      most = 86400;
      unit = "seconds";
    }
  ];

  # ---------------------------------------------------------------------
  # BRIDGES — the only reason Tor works at all on a censored network.
  # ---------------------------------------------------------------------
  bridges = [
    {
      id = "use-bridges";
      key = "UseBridges";
      kind = "switch";
    }
    # THE PASTED LINES. A `list` and not `text`: bridges arrive two or three at
    # a time from the browser or from the bridges address at torproject.org,
    # and torrc takes one `Bridge` line each. Every line is checked for a line
    # break before it is written — see `fragment::a_value` for what a pasted
    # newline could do.
    {
      id = "bridge-lines";
      key = "Bridge";
      kind = "list";
    }
    # WHICH PLUGGABLE TRANSPORT'S PROGRAM IS DECLARED. `lyrebird` speaks obfs4,
    # meek_lite and scramblesuit from one binary; snowflake is its own. The
    # ring's words become the `ClientTransportPlugin` line, whose `exec` comes
    # from the store paths the flake bakes in.
    {
      id = "bridge-transport";
      key = "ClientTransportPlugin";
      kind = "ring";
      ring = [
        "none"
        "obfs4"
        "snowflake"
        "meek_lite"
      ];
    }
    # A MACHINE BEHIND A FIREWALL THAT ONLY LETS 80 AND 443 OUT. It is the
    # other half of "Tor will not connect here", and it is not a bridge.
    {
      id = "firewall-ports";
      key = "ReachableAddresses";
      kind = "text";
      advanced = true;
    }
    {
      id = "fascist-firewall";
      key = "FascistFirewall";
      kind = "switch";
      advanced = true;
    }
  ];

  # ---------------------------------------------------------------------
  # ONION SERVICES — what this machine offers back.
  # ---------------------------------------------------------------------
  onion = [
    # THE DIRECTORY TOR KEEPS THE KEY IN. It is created by Tor, at 0700, under
    # the fragment directory — the parent is group-writable by `tor`, which is
    # what makes that possible without a second privileged step.
    {
      id = "onion-directory";
      key = "HiddenServiceDir";
      kind = "text";
    }
    # `<virtual port> <where it goes>`, e.g. `80 127.0.0.1:8080`. One line per
    # port, which is why it is a list.
    {
      id = "onion-ports";
      key = "HiddenServicePort";
      kind = "list";
    }
    # `HiddenServiceVersion` IS NOT HERE, and the generator is why: version 3
    # is the only one Tor still builds, so the ring had one word, and a ring
    # that cannot turn is a row that teaches a key which does nothing. A
    # setting with one possible value is a fact, and this one is not even worth
    # drawing.
    {
      id = "onion-single-hop";
      key = "HiddenServiceNonAnonymousMode";
      kind = "switch";
      advanced = true;
    }
  ];

  # ---------------------------------------------------------------------
  # THE DRAWER — real settings, rarely touched.
  # ---------------------------------------------------------------------
  advanced = [
    {
      id = "safe-socks";
      key = "SafeSocks";
      kind = "switch";
      advanced = true;
    }
    {
      id = "new-circuit-period";
      key = "NewCircuitPeriod";
      kind = "number";
      least = 1;
      most = 3600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "circuit-build-timeout";
      key = "CircuitBuildTimeout";
      kind = "number";
      least = 1;
      most = 600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "entry-guards";
      key = "NumEntryGuards";
      kind = "number";
      least = 1;
      most = 10;
      unit = "guards";
      advanced = true;
    }
    {
      id = "use-entry-guards";
      key = "UseEntryGuards";
      kind = "switch";
      advanced = true;
    }
    {
      id = "distinct-subnets";
      key = "EnforceDistinctSubnets";
      kind = "switch";
      advanced = true;
    }
    {
      id = "ipv6";
      key = "ClientUseIPv6";
      kind = "switch";
      advanced = true;
    }
    {
      id = "prefer-ipv6";
      key = "ClientPreferIPv6ORPort";
      kind = "switch";
      advanced = true;
    }
    # PADDING IS BATTERY. Tor sends cover traffic by default; the reduced form
    # is what a laptop on battery wants, and the screen says what it costs.
    {
      id = "connection-padding";
      key = "ConnectionPadding";
      kind = "ring";
      ring = [
        "auto"
        "1"
        "0"
      ];
      advanced = true;
    }
    {
      id = "reduced-padding";
      key = "ReducedConnectionPadding";
      kind = "switch";
      advanced = true;
    }
    {
      id = "avoid-disk-writes";
      key = "AvoidDiskWrites";
      kind = "switch";
      advanced = true;
    }
    # HOW LONG AN IDLE TOR WAITS BEFORE IT STOPS BUILDING CIRCUITS. Zero turns
    # it off, which is what a machine that must answer an onion service wants.
    {
      id = "dormant-timeout";
      key = "DormantClientTimeout";
      kind = "number";
      least = 0;
      most = 604800;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "dormant-on-start";
      key = "DormantOnFirstStartup";
      kind = "switch";
      advanced = true;
    }
    # AN UPSTREAM PROXY, for a network where even reaching a bridge needs one.
    {
      id = "socks-proxy";
      key = "Socks5Proxy";
      kind = "text";
      advanced = true;
    }
    {
      id = "https-proxy";
      key = "HTTPSProxy";
      kind = "text";
      advanced = true;
    }
    {
      id = "log-level";
      key = "Log";
      kind = "ring";
      ring = [
        "notice syslog"
        "info syslog"
        "debug syslog"
      ];
      advanced = true;
    }
  ];
}
