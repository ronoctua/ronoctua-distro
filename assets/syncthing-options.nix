# Every option Syncthing exposes, declared once.
#
# WHY A TABLE AND NOT A HUNDRED ROWS OF RUST
# ------------------------------------------
# The API carries 131 keys across four objects. Written by hand on the screen
# side, that would be 131 decisions spread over a file nobody could audit, and
# the day Syncthing renamed one there would be no way to notice. Declared here,
# it produces THREE things from one source: the Rust table the view walks, the
# list of text keys both languages owe, and the check that compares this file
# against a real `/rest/config` and fails in BOTH directions.
#
# THE FIELDS
# ----------
#   id        kebab-case. It is the stem of the text key (`syncthing-opt-<id>`)
#             and it is what a check names. It is NOT the JSON key: `type`
#             would collide across scopes and reads as nothing on a screen.
#   key       the JSON key, exactly as Syncthing spells it.
#   within    the nested object the key sits inside, when it does. A PATCH of
#             `{"versioning":{"type":"simple"}}` deep-merges, because Go's
#             unmarshal leaves absent fields alone — but an ARRAY is replaced
#             whole, which is why no list is ever written this way.
#   kind      switch | number | text | ring | list | amount
#   least     for `number`, the smallest Syncthing accepts.
#   most      for `number`, the largest worth offering.
#   ring      for `ring`, the words, in the order the ring turns.
#   fixed     the flake writes this one. See below.
#   advanced  it lives in the drawer, closed.
#
# `fixed = true` IS INVARIANT 6 ON THE SCREEN
# --------------------------------------------
# `nix/modules/syncthing.nix` PUTs these into the running service at every
# activation. Offering them as editable would be offering a key whose effect
# the next `nixos-rebuild` undoes, in silence — which is the exact failure this
# project spends its checks hunting. So they are drawn, reachable, and they
# refuse, saying where the value comes from. It is the shape the format
# buffer's disabled XFS row already has.
#
# WHAT IS DELIBERATELY NOT HERE
# ------------------------------
# The API key and the GUI password: they are read to sign a request and never
# reach a snapshot, let alone a row. `deviceID`, `certName`, `introducedBy`,
# `urSeen`, `urUniqueId` and `unackedNotificationIDs`: those are identity and
# bookkeeping, not settings — the view draws the first three as facts.
# A folder's `id`: Syncthing will not change one, so it is a fact too.
# `devices`: which machines a folder is shared with is a LIST OF ROWS on the
# sheet, one per machine with a yes or no, because a folder is shared with the
# machines you have and not with a string you type.
{
  # ---------------------------------------------------------------------
  # A FOLDER — 36 options of the 41 keys it carries.
  # ---------------------------------------------------------------------
  folder = [
    # WHAT IT IS CALLED AND WHERE IT IS are options like any other, and that is
    # deliberate. They were going to be three hand-made rows with three
    # hand-made editors beside a hundred generated ones; declaring them means
    # one machine edits everything, and the two languages owe their labels
    # through the same generator as the rest. The `id` is NOT here: Syncthing
    # will not change a folder's id, and a row that cannot act is a fact.
    {
      id = "folder-label";
      key = "label";
      kind = "text";
    }
    {
      id = "folder-path";
      key = "path";
      kind = "text";
    }
    {
      id = "folder-type";
      key = "type";
      kind = "ring";
      ring = [
        "sendreceive"
        "sendonly"
        "receiveonly"
        "receiveencrypted"
      ];
    }
    {
      id = "folder-paused";
      key = "paused";
      kind = "switch";
    }
    {
      id = "rescan-interval";
      key = "rescanIntervalS";
      kind = "number";
      least = 0;
      most = 31536000;
      unit = "seconds";
    }
    {
      id = "watch-for-changes";
      key = "fsWatcherEnabled";
      kind = "switch";
    }
    {
      id = "watch-delay";
      key = "fsWatcherDelayS";
      kind = "number";
      least = 1;
      most = 3600;
      unit = "seconds";
    }
    {
      id = "watch-timeout";
      key = "fsWatcherTimeoutS";
      kind = "number";
      least = 0;
      most = 3600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "ignore-permissions";
      key = "ignorePerms";
      kind = "switch";
    }
    {
      id = "keep-conflicts";
      key = "maxConflicts";
      kind = "number";
      least = -1;
      most = 100;
      unit = "copies";
    }
    {
      id = "pull-order";
      key = "order";
      kind = "ring";
      ring = [
        "random"
        "alphabetic"
        "smallestFirst"
        "largestFirst"
        "oldestFirst"
        "newestFirst"
      ];
    }
    {
      id = "block-pull-order";
      key = "blockPullOrder";
      kind = "ring";
      ring = [
        "standard"
        "random"
        "inOrder"
      ];
      advanced = true;
    }
    {
      id = "copy-range-method";
      key = "copyRangeMethod";
      kind = "ring";
      ring = [
        "standard"
        "copy_file_range"
        "ioctl"
        "sendfile"
        "duplicate_extents"
        "all"
      ];
      advanced = true;
    }
    {
      id = "filesystem-type";
      key = "filesystemType";
      kind = "ring";
      ring = [
        "basic"
        "fake"
      ];
      advanced = true;
    }
    {
      id = "marker-name";
      key = "markerName";
      kind = "text";
      advanced = true;
    }
    {
      id = "auto-normalize";
      key = "autoNormalize";
      kind = "switch";
      advanced = true;
    }
    {
      id = "case-sensitive";
      key = "caseSensitiveFS";
      kind = "switch";
      advanced = true;
    }
    {
      id = "junctions-as-dirs";
      key = "junctionsAsDirs";
      kind = "switch";
      advanced = true;
    }
    {
      id = "ignore-delete";
      key = "ignoreDelete";
      kind = "switch";
      advanced = true;
    }
    {
      id = "disable-sparse";
      key = "disableSparseFiles";
      kind = "switch";
      advanced = true;
    }
    {
      id = "disable-fsync";
      key = "disableFsync";
      kind = "switch";
      advanced = true;
    }
    {
      id = "block-indexing";
      key = "blockIndexing";
      kind = "switch";
      advanced = true;
    }
    {
      id = "mod-time-window";
      key = "modTimeWindowS";
      kind = "number";
      least = 0;
      most = 3600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "max-concurrent-writes";
      key = "maxConcurrentWrites";
      kind = "number";
      least = 1;
      most = 64;
      unit = "writes";
      advanced = true;
    }
    {
      id = "copy-ownership";
      key = "copyOwnershipFromParent";
      kind = "switch";
      advanced = true;
    }
    {
      id = "sync-ownership";
      key = "syncOwnership";
      kind = "switch";
      advanced = true;
    }
    {
      id = "send-ownership";
      key = "sendOwnership";
      kind = "switch";
      advanced = true;
    }
    {
      id = "sync-xattrs";
      key = "syncXattrs";
      kind = "switch";
      advanced = true;
    }
    {
      id = "send-xattrs";
      key = "sendXattrs";
      kind = "switch";
      advanced = true;
    }
    {
      id = "scan-progress-interval";
      key = "scanProgressIntervalS";
      kind = "number";
      least = 0;
      most = 3600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "puller-max-pending";
      key = "pullerMaxPendingKiB";
      kind = "number";
      least = 0;
      most = 1048576;
      unit = "kib";
      advanced = true;
    }
    {
      id = "puller-delay";
      key = "pullerDelayS";
      kind = "number";
      least = 0;
      most = 3600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "puller-pause";
      key = "pullerPauseS";
      kind = "number";
      least = 0;
      most = 3600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "copiers";
      key = "copiers";
      kind = "number";
      least = 0;
      most = 64;
      unit = "threads";
      advanced = true;
    }
    {
      id = "hashers";
      key = "hashers";
      kind = "number";
      least = 0;
      most = 64;
      unit = "threads";
      advanced = true;
    }
    {
      id = "folder-group";
      key = "group";
      kind = "text";
      advanced = true;
    }
    # THE SPACE TO LEAVE FREE is an object, `{ value, unit }`, and its two
    # halves are two rows that both write the whole object back.
    {
      id = "least-free-space";
      key = "minDiskFree";
      kind = "amount";
    }
    # VERSIONING is a nested object, and a PATCH of one deep-merges: Go's
    # unmarshal leaves the fields the JSON does not mention alone. An ARRAY
    # would not, which is why no list is ever written this way.
    {
      id = "versioning";
      key = "type";
      within = "versioning";
      kind = "ring";
      ring = [
        ""
        "trashcan"
        "simple"
        "staggered"
        "external"
      ];
    }
    {
      id = "versioning-cleanup";
      key = "cleanupIntervalS";
      within = "versioning";
      kind = "number";
      least = 0;
      most = 31536000;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "versioning-path";
      key = "fsPath";
      within = "versioning";
      kind = "text";
      advanced = true;
    }
    {
      id = "xattr-permitted";
      key = "entries";
      within = "xattrFilter";
      kind = "list";
      advanced = true;
    }
  ];

  # ---------------------------------------------------------------------
  # A DEVICE — 16 of the 19 keys.
  # ---------------------------------------------------------------------
  device = [
    {
      id = "device-name";
      key = "name";
      kind = "text";
    }
    {
      id = "device-paused";
      key = "paused";
      kind = "switch";
    }
    {
      id = "compression";
      key = "compression";
      kind = "ring";
      ring = [
        "metadata"
        "always"
        "never"
      ];
    }
    {
      id = "auto-accept-folders";
      key = "autoAcceptFolders";
      kind = "switch";
    }
    {
      id = "introducer";
      key = "introducer";
      kind = "switch";
    }
    {
      id = "skip-introduction-removals";
      key = "skipIntroductionRemovals";
      kind = "switch";
      advanced = true;
    }
    {
      id = "untrusted";
      key = "untrusted";
      kind = "switch";
    }
    {
      id = "device-send-limit";
      key = "maxSendKbps";
      kind = "number";
      least = 0;
      most = 1000000;
      unit = "kbps";
    }
    {
      id = "device-receive-limit";
      key = "maxRecvKbps";
      kind = "number";
      least = 0;
      most = 1000000;
      unit = "kbps";
    }
    # THE MEASUREMENT THAT PAID FOR THIS ONE is in the module: a single TCP
    # flow cannot keep enough window in flight on a wireless link whose latency
    # under load goes from 3 ms to 150. The flake tunes the DEFAULT, not this;
    # a device that already exists keeps whatever it was created with.
    {
      id = "connections";
      key = "numConnections";
      kind = "number";
      least = 0;
      most = 8;
      unit = "connections";
    }
    {
      id = "max-request";
      key = "maxRequestKiB";
      kind = "number";
      least = 0;
      most = 65536;
      unit = "kib";
      advanced = true;
    }
    {
      id = "remote-interface-port";
      key = "remoteGUIPort";
      kind = "number";
      least = 0;
      most = 65535;
      advanced = true;
    }
    {
      id = "device-group";
      key = "group";
      kind = "text";
      advanced = true;
    }
    {
      id = "device-addresses";
      key = "addresses";
      kind = "list";
    }
    {
      id = "allowed-networks";
      key = "allowedNetworks";
      kind = "list";
      advanced = true;
    }
    {
      id = "ignored-folders";
      key = "ignoredFolders";
      kind = "list";
      advanced = true;
    }
  ];

  # ---------------------------------------------------------------------
  # THE WHOLE SERVICE — 46 of the 55 keys.
  # ---------------------------------------------------------------------
  global = [
    # --- the fourteen the flake owns ----------------------------------
    {
      id = "nat-traversal";
      key = "natEnabled";
      kind = "switch";
      fixed = true;
    }
    {
      id = "relays";
      key = "relaysEnabled";
      kind = "switch";
      fixed = true;
    }
    {
      id = "announce-locally";
      key = "localAnnounceEnabled";
      kind = "switch";
      fixed = true;
    }
    {
      id = "announce-globally";
      key = "globalAnnounceEnabled";
      kind = "switch";
      fixed = true;
    }
    {
      id = "send-limit";
      key = "maxSendKbps";
      kind = "number";
      least = 0;
      most = 1000000;
      unit = "kbps";
      fixed = true;
    }
    {
      id = "receive-limit";
      key = "maxRecvKbps";
      kind = "number";
      least = 0;
      most = 1000000;
      unit = "kbps";
      fixed = true;
    }
    {
      id = "limit-inside-the-lan";
      key = "limitBandwidthInLan";
      kind = "switch";
      fixed = true;
    }
    {
      id = "low-priority";
      key = "setLowPriority";
      kind = "switch";
      fixed = true;
    }
    {
      id = "auto-upgrade";
      key = "autoUpgradeIntervalH";
      kind = "number";
      least = 0;
      most = 168;
      unit = "hours";
      fixed = true;
    }
    {
      id = "usage-report";
      key = "urAccepted";
      kind = "number";
      least = -1;
      most = 3;
      fixed = true;
    }
    {
      id = "crash-reporting";
      key = "crashReportingEnabled";
      kind = "switch";
      fixed = true;
    }
    {
      id = "open-a-browser";
      key = "startBrowser";
      kind = "switch";
      fixed = true;
    }

    # --- the thirty-two that are yours --------------------------------
    # TWO OF THE FOURTEEN COME FROM NIXPKGS AND NOT FROM OUR MODULE, and they
    # are the flake's all the same: `services.syncthing` PUTs the whole of
    # `settings.options` at every activation, and nixpkgs contributes a default
    # for these two. `syncthing-flake-owned-rows-refuse` is what found them —
    # both were declared editable, and both would have been silently undone by
    # the next `nixos-rebuild`.
    {
      id = "folder-concurrency";
      key = "maxFolderConcurrency";
      kind = "number";
      least = 0;
      most = 64;
      unit = "folders";
      fixed = true;
    }
    {
      id = "keep-temporaries";
      key = "keepTemporariesH";
      kind = "number";
      least = 0;
      most = 8760;
      unit = "hours";
    }
    {
      id = "reconnect-interval";
      key = "reconnectionIntervalS";
      kind = "number";
      least = 1;
      most = 3600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "progress-interval";
      key = "progressUpdateIntervalS";
      kind = "number";
      least = -1;
      most = 3600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "announce-lan-addresses";
      key = "announceLANAddresses";
      kind = "switch";
      advanced = true;
    }
    {
      id = "overwrite-names";
      key = "overwriteRemoteDeviceNamesOnConnect";
      kind = "switch";
      advanced = true;
    }
    {
      id = "send-full-index";
      key = "sendFullIndexOnUpgrade";
      kind = "switch";
      advanced = true;
    }
    {
      id = "temp-index-min-blocks";
      key = "tempIndexMinBlocks";
      kind = "number";
      least = 0;
      most = 65536;
      unit = "blocks";
      advanced = true;
    }
    {
      id = "incoming-request-buffer";
      key = "maxConcurrentIncomingRequestKiB";
      kind = "number";
      least = 0;
      most = 1048576;
      unit = "kib";
      advanced = true;
    }
    {
      id = "connection-limit-enough";
      key = "connectionLimitEnough";
      kind = "number";
      least = 0;
      most = 1024;
      unit = "connections";
      advanced = true;
    }
    {
      id = "connection-limit-max";
      key = "connectionLimitMax";
      kind = "number";
      least = 0;
      most = 1024;
      unit = "connections";
      advanced = true;
    }
    {
      id = "priority-tcp-lan";
      key = "connectionPriorityTcpLan";
      kind = "number";
      least = 0;
      most = 100;
      advanced = true;
    }
    {
      id = "priority-tcp-wan";
      key = "connectionPriorityTcpWan";
      kind = "number";
      least = 0;
      most = 100;
      advanced = true;
    }
    {
      id = "priority-quic-lan";
      key = "connectionPriorityQuicLan";
      kind = "number";
      least = 0;
      most = 100;
      advanced = true;
    }
    {
      id = "priority-quic-wan";
      key = "connectionPriorityQuicWan";
      kind = "number";
      least = 0;
      most = 100;
      advanced = true;
    }
    {
      id = "priority-relay";
      key = "connectionPriorityRelay";
      kind = "number";
      least = 0;
      most = 100;
      advanced = true;
    }
    {
      id = "priority-upgrade-threshold";
      key = "connectionPriorityUpgradeThreshold";
      kind = "number";
      least = 0;
      most = 100;
      advanced = true;
    }
    {
      id = "traffic-class";
      key = "trafficClass";
      kind = "number";
      least = 0;
      most = 255;
      advanced = true;
    }
    {
      id = "announce-port";
      key = "localAnnouncePort";
      kind = "number";
      least = 1;
      most = 65535;
      fixed = true;
      advanced = true;
    }
    {
      id = "announce-group";
      key = "localAnnounceMCAddr";
      kind = "text";
      advanced = true;
    }
    {
      id = "nat-lease";
      key = "natLeaseMinutes";
      kind = "number";
      least = 0;
      most = 1440;
      unit = "minutes";
      advanced = true;
    }
    {
      id = "nat-renewal";
      key = "natRenewalMinutes";
      kind = "number";
      least = 0;
      most = 1440;
      unit = "minutes";
      advanced = true;
    }
    {
      id = "nat-timeout";
      key = "natTimeoutSeconds";
      kind = "number";
      least = 0;
      most = 600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "stun-keepalive-start";
      key = "stunKeepaliveStartS";
      kind = "number";
      least = 0;
      most = 3600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "stun-keepalive-least";
      key = "stunKeepaliveMinS";
      kind = "number";
      least = 0;
      most = 3600;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "relay-reconnect";
      key = "relayReconnectIntervalM";
      kind = "number";
      least = 1;
      most = 1440;
      unit = "minutes";
      advanced = true;
    }
    {
      id = "pre-releases";
      key = "upgradeToPreReleases";
      kind = "switch";
      advanced = true;
    }
    {
      id = "audit";
      key = "auditEnabled";
      kind = "switch";
      advanced = true;
    }
    {
      id = "audit-file";
      key = "auditFile";
      kind = "text";
      advanced = true;
    }
    {
      id = "report-delay";
      key = "urInitialDelayS";
      kind = "number";
      least = 0;
      most = 86400;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "report-insecurely";
      key = "urPostInsecurely";
      kind = "switch";
      advanced = true;
    }
    {
      id = "report-address";
      key = "urURL";
      kind = "text";
      advanced = true;
    }
    {
      id = "crash-address";
      key = "crURL";
      kind = "text";
      advanced = true;
    }
    {
      id = "releases-address";
      key = "releasesURL";
      kind = "text";
      advanced = true;
    }
    {
      id = "listen-addresses";
      key = "listenAddresses";
      kind = "list";
    }
    {
      id = "discovery-servers";
      key = "globalAnnounceServers";
      kind = "list";
      advanced = true;
    }
    {
      id = "stun-servers";
      key = "stunServers";
      kind = "list";
      advanced = true;
    }
    {
      id = "always-local-networks";
      key = "alwaysLocalNets";
      kind = "list";
      advanced = true;
    }
    {
      id = "least-free-home";
      key = "minHomeDiskFree";
      kind = "amount";
    }
  ];

  # ---------------------------------------------------------------------
  # THE WEB INTERFACE — the handful that are settings rather than secrets.
  # ---------------------------------------------------------------------
  interface = [
    # THE ADDRESS IS THE FLAKE'S, and deliberately so: it stays on the
    # loopback because whoever opens it is `LEADER s s a`, on this same
    # machine. On the LAN it would be an administrative port with no password.
    {
      id = "interface-address";
      key = "address";
      kind = "text";
      fixed = true;
    }
    {
      id = "interface-enabled";
      key = "enabled";
      kind = "switch";
      advanced = true;
    }
    {
      id = "interface-tls";
      key = "useTLS";
      kind = "switch";
      advanced = true;
    }
    {
      id = "interface-theme";
      key = "theme";
      kind = "text";
      advanced = true;
    }
    {
      id = "interface-user";
      key = "user";
      kind = "text";
      advanced = true;
    }
    {
      id = "interface-auth-mode";
      key = "authMode";
      kind = "ring";
      ring = [
        "static"
        "ldap"
      ];
      advanced = true;
    }
    {
      id = "interface-basic-auth-prompt";
      key = "sendBasicAuthPrompt";
      kind = "switch";
      advanced = true;
    }
    {
      id = "interface-metrics-without-auth";
      key = "metricsWithoutAuth";
      kind = "switch";
      advanced = true;
    }
    {
      id = "interface-insecure-admin";
      key = "insecureAdminAccess";
      kind = "switch";
      advanced = true;
    }
    {
      id = "interface-insecure-host-check";
      key = "insecureSkipHostcheck";
      kind = "switch";
      advanced = true;
    }
    {
      id = "interface-insecure-frames";
      key = "insecureAllowFrameLoading";
      kind = "switch";
      advanced = true;
    }
    {
      id = "interface-session-life";
      key = "sessionCookieDurationS";
      kind = "number";
      least = 0;
      most = 31536000;
      unit = "seconds";
      advanced = true;
    }
    {
      id = "interface-socket-permissions";
      key = "unixSocketPermissions";
      kind = "text";
      advanced = true;
    }
  ];
}
