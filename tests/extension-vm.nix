# The extension's installation, proved on a machine that booted.
#
# WHAT ONLY THE VM PROVES
# -----------------------
# The packer is exercised by hand and the manifest by `extension-has-no-fingerprint`,
# but neither of those is a machine. Four things need one, and every one of them
# is the kind that fails in silence:
#
#   1. the unit RUNS. A `systemd.services` entry that does not evaluate, or a
#      script that fails on a machine with no `/var/lib` yet, is a policy nobody
#      writes and an extension nobody installs;
#   2. the key is generated ONCE and stays `0600` root. A second run must not
#      change the id — a changed id is a different extension, and the old one
#      stays force-installed by a policy nothing writes any more;
#   3. the files Chrome reads agree with each other AND with the `.crx`. The
#      id appears in four places — the policy and the two hosts' manifests —
#      and they are written by one script for exactly this reason; nothing
#      checks that at eval time;
#   4. the signature verifies the way Chrome verifies it — over
#      `"CRX3 SignedData\0" | len | signed_header_data | zip`, which is
#      documented in no convenient place and which a refactor can silently
#      shift by four bytes.
#
#   5. THE VERSION GOES UP, and only when the extension changes. Chrome
#      replaces a force-installed extension only with a higher version: the
#      packed manifest and `update.xml` carry the same `<base>.<n>`, a restart
#      with the same build keeps it, and another build takes `n + 1`.
#
# And the two hosts are called THE WAY CHROME CALLS THEM: the extension's
# origin as the first argument, read from the manifest — never a word of our
# own, which is how the queue's host died under a real browser while this
# test, passing `browser` by hand, stayed green.
#
# WHAT IT DELIBERATELY DOES NOT PROVE
# ------------------------------------
# That Chrome accepts the policy. That needs Chrome, a display and a login:
# `tests/kiosk-vm.nix` runs the real one and proves it there.
{
  pkgs,
  lib,
  ronoctua-shell,
  ronoctua-transfers,
  ronoctua-browser,
  extension,
}:

let
  # The check's own reading of the crx, in the same terms `pack-crx.py` writes
  # it — and deliberately NOT by importing that script: a verifier that shared
  # code with the writer would agree with it about a mistake.
  verify = pkgs.writeText "verify-crx.py" ''
    import hashlib, json, pathlib, struct, subprocess, sys

    home = pathlib.Path("/var/lib/ronoctua-browser")
    crx = (home / "ronoctua.crx").read_bytes()
    if crx[:4] != b"Cr24":
        sys.exit("ERROR: the .crx does not start with Cr24")
    version, header_len = struct.unpack("<II", crx[4:12])
    if version != 3:
        sys.exit(f"ERROR: CRX version {version}, and the current Chrome only reads 3")
    header, payload = crx[12 : 12 + header_len], crx[12 + header_len :]

    def fields(buf):
        out, i = [], 0
        while i < len(buf):
            key = shift = 0
            while True:
                b = buf[i]; i += 1
                key |= (b & 0x7F) << shift; shift += 7
                if not b & 0x80: break
            if key & 7 != 2:
                sys.exit("ERROR: a protobuf field that is not length-delimited")
            size = shift = 0
            while True:
                b = buf[i]; i += 1
                size |= (b & 0x7F) << shift; shift += 7
                if not b & 0x80: break
            out.append((key >> 3, buf[i : i + size])); i += size
        return dict(out)

    top = fields(header)
    proof = fields(top[2])
    public, signature, signed_header = proof[1], proof[2], top[10000]

    # THE ID, DERIVED HERE and compared with all three places that carry it.
    letters = "abcdefghijklmnop"
    raw = hashlib.sha256(public).digest()[:16]
    derived = "".join(letters[b >> 4] + letters[b & 0x0F] for b in raw)

    written = (home / "id").read_text().strip()
    if written != derived:
        sys.exit(f"ERROR: the id written ({written}) is not the key's ({derived})")

    policy = json.loads(
        pathlib.Path("/etc/opt/chrome/policies/managed/ronoctua.json").read_text()
    )
    settings = policy["ExtensionSettings"]
    if list(settings) != [derived]:
        sys.exit(f"ERROR: the policy names {list(settings)} and not {derived}")
    if settings[derived]["installation_mode"] != "force_installed":
        sys.exit("ERROR: the policy does not force the installation")

    # BOTH HOSTS, each allowing our extension alone, each at a binary that
    # exists — and named by the one table of names.
    for name in ${hostNames}:
        host = json.loads(
            pathlib.Path(f"/etc/opt/chrome/native-messaging-hosts/{name}.json").read_text()
        )
        if host["name"] != name:
            sys.exit(f"ERROR: {name}.json calls itself {host['name']}")
        origins = host["allowed_origins"]
        if origins != [f"chrome-extension://{derived}/"]:
            sys.exit(f"ERROR: {name} accepts {origins}, and not our extension alone")
        if not pathlib.Path(host["path"]).is_file():
            sys.exit(f"ERROR: {name} points at {host['path']}, which does not exist")

    # THE VERSION: what the crx says and what `update.xml` advertises are one
    # number, the built base plus the machine's counter.
    import io, re, zipfile
    packed = json.loads(zipfile.ZipFile(io.BytesIO(payload)).read("manifest.json"))
    update = (home / "update.xml").read_text()
    advertised = re.search(r'<updatecheck [^>]*version="([0-9.]+)"', update).group(1)
    if packed["version"] != advertised:
        sys.exit(f"ERROR: the crx is {packed['version']} and update.xml says {advertised}")
    if not packed["version"].startswith("${extension.version}."):
        sys.exit(f"ERROR: {packed['version']} is not the base ${extension.version} and a counter")

    # AND THE SIGNATURE, over exactly the bytes Chrome covers.
    blob = (
        b"CRX3 SignedData\0"
        + struct.pack("<I", len(signed_header))
        + signed_header
        + payload
    )
    pathlib.Path("/tmp/blob").write_bytes(blob)
    pathlib.Path("/tmp/sig").write_bytes(signature)
    pathlib.Path("/tmp/pub.der").write_bytes(public)
    subprocess.run(
        ["openssl", "rsa", "-pubin", "-inform", "DER", "-in", "/tmp/pub.der",
         "-outform", "PEM", "-out", "/tmp/pub.pem"],
        check=True, capture_output=True,
    )
    done = subprocess.run(
        ["openssl", "dgst", "-sha256", "-verify", "/tmp/pub.pem",
         "-signature", "/tmp/sig", "/tmp/blob"],
        capture_output=True,
    )
    if done.returncode != 0:
        sys.exit("ERROR: the .crx's signature does NOT check out the way Chrome checks it")

    print(f"OK: {derived} {packed['version']}, the signature checks out, and every place agrees")
  '';

  hosts = import ../nix/browser-hosts.nix;
  hostNames = builtins.toJSON [
    hosts.transfers
    hosts.browser
  ];

  # A SECOND BUILD of the same extension, for the counter: the same files and
  # one more, so its store path differs.
  rebuilt = pkgs.runCommand "ronoctua-extension-rebuilt" { inherit (extension) version; } ''
    cp -r ${extension} $out
    chmod -R u+w $out
    echo >> $out/share/ronoctua-extension/popup.css
  '';
in
pkgs.testers.runNixOSTest {
  name = "ronoctua-extension";

  nodes.machine = {
    imports = [
      ../nix/modules/browser-extension.nix
      # The module hangs off the browser's own switch, and the browser is NOT
      # installed here: a VM that pulled Chrome in would spend four hundred
      # megabytes to check a JSON file. So the option is declared on its own,
      # in its own module — mixing `options` and bare configuration in one
      # attribute set is what NixOS refuses with a message about `_module`.
      (
        { lib, ... }:
        {
          options.ronoctua.browser.enable = lib.mkOption {
            type = lib.types.bool;
            default = true;
            description = "The browser — here only the key the module reads.";
          };
        }
      )
    ];

    virtualisation.memorySize = 1024;
    virtualisation.cores = 2;

    _module.args.ronoctua = {
      transfers = ronoctua-transfers;
      browser = ronoctua-browser;
      inherit extension;
    };

    # The same machine with another build of the extension: `switch-to-
    # configuration` restarts the unit (its script cites the build), which
    # is the path a rebuild takes.
    specialisation.rebuilt.configuration = {
      _module.args.ronoctua = lib.mkForce {
        transfers = ronoctua-transfers;
        browser = ronoctua-browser;
        extension = rebuilt;
      };
    };

    environment.systemPackages = [
      pkgs.python3
      pkgs.openssl
    ];

    # `display-manager.service` is what the module orders itself before, and
    # there is none here. Without this the unit is ordered before a unit that
    # does not exist, which systemd accepts — but saying so out loud is what
    # keeps the test honest about what it is running.
    systemd.services.ronoctua-browser-extension.before = lib.mkForce [ ];
  };

  testScript = ''
    import re

    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("ronoctua-browser-extension.service")

    # 1. THE KEY, and its permissions.
    machine.succeed("test -f /var/lib/ronoctua-browser/key.pem")
    perms = machine.succeed("stat -c '%a %U' /var/lib/ronoctua-browser/key.pem").strip()
    assert perms == "600 root", f"the key is {perms}, and should be 600 root"
    # AND THE REST IS THE PERSON'S TO READ. Chrome runs as the person and
    # fetches `update.xml` and the crx through `file://`: a directory it
    # cannot enter is an extension that never installs, silently — which is
    # what `0700` here was.
    home = machine.succeed("stat -c '%a' /var/lib/ronoctua-browser").strip()
    assert home == "755", f"the directory is {home}, and Chrome (the person) must enter it"
    for public in ("update.xml", "ronoctua.crx"):
        machine.succeed(f"runuser -u nobody -- test -r /var/lib/ronoctua-browser/{public}")
    machine.fail("runuser -u nobody -- cat /var/lib/ronoctua-browser/key.pem")

    # 2. EVERYTHING AGREES, and the signature is the one Chrome checks.
    print(machine.succeed("python3 ${verify}"))

    # 3. A SECOND RUN KEEPS THE ID. Regenerating the key would leave the old
    #    extension force-installed by a policy nothing writes any more, and take
    #    the person's switch and history with it.
    def advertised():
        xml = machine.succeed("cat /var/lib/ronoctua-browser/update.xml")
        found = re.search(r'<updatecheck [^>]*version="([0-9.]+)"', xml)
        assert found, f"update.xml advertises no version:\n{xml}"
        return found.group(1)

    before = machine.succeed("cat /var/lib/ronoctua-browser/id").strip()
    first = advertised()
    machine.succeed("systemctl restart ronoctua-browser-extension.service")
    machine.wait_for_unit("ronoctua-browser-extension.service")
    after = machine.succeed("cat /var/lib/ronoctua-browser/id").strip()
    assert before == after, f"the id changed between runs: {before} -> {after}"
    assert advertised() == first, f"the same build moved the version: {first} -> {advertised()}"
    print(machine.succeed("python3 ${verify}"))

    # 3b. ANOTHER BUILD GOES UP. Chrome takes nothing that is not higher.
    machine.succeed(
        "/run/current-system/specialisation/rebuilt/bin/switch-to-configuration test"
    )
    machine.wait_for_unit("ronoctua-browser-extension.service")
    second = advertised()
    older = [int(n) for n in first.split(".")]
    newer = [int(n) for n in second.split(".")]
    assert newer > older, f"another build did not go up: {first} -> {second}"
    print(f"OK: the version went {first} -> {second}")
    print(machine.succeed("python3 ${verify}"))

    # 4. AND THE HOST BINARY ANSWERS Chrome's framing, called as Chrome calls
    #    it: the manifest's `path`, and the caller's origin — read from the
    #    manifest's own allow-list — as the only argument.
    #
    #    Four bytes of length and then JSON, both ways. A `ping` needs no queue
    #    behind it, which is the point: what is under test is the framing, the
    #    argument, and the binary being where the manifest says it is.
    machine.succeed(
        "python3 - <<'PY'\n"
        "import json, struct, subprocess, pathlib\n"
        "host = json.loads(pathlib.Path('/etc/opt/chrome/native-messaging-hosts/${hosts.transfers}.json').read_text())\n"
        "body = json.dumps({'kind': 'ping'}).encode()\n"
        "done = subprocess.run([host['path'], host['allowed_origins'][0]],\n"
        "    input=struct.pack('<I', len(body)) + body, capture_output=True, timeout=30)\n"
        "out = done.stdout\n"
        "assert len(out) > 4, f'no answer: {done.stderr!r}'\n"
        "size = struct.unpack('<I', out[:4])[0]\n"
        "answer = json.loads(out[4:4 + size])\n"
        "assert answer.get('ok') is True, answer\n"
        "print('OK: the host answers the Chrome framing')\n"
        "PY"
    )

    # 5. THE KIOSK'S HOST, called the same way: with no footer to reach it
    #    says so in a frame, and the end of its input ends it. By hand, it
    #    refuses to run at all.
    machine.succeed(
        "python3 - <<'PY'\n"
        "import json, struct, subprocess, pathlib\n"
        "host = json.loads(pathlib.Path('/etc/opt/chrome/native-messaging-hosts/${hosts.browser}.json').read_text())\n"
        "done = subprocess.run([host['path'], host['allowed_origins'][0]],\n"
        "    input=b\"\", capture_output=True, timeout=30,\n"
        "    env={'XDG_RUNTIME_DIR': '/tmp'})\n"
        "assert done.returncode == 0, done.stderr\n"
        "out = done.stdout\n"
        "size = struct.unpack('<I', out[:4])[0]\n"
        "said = json.loads(out[4:4 + size])\n"
        "assert said == {'said': 'footer', 'up': False}, said\n"
        "hand = subprocess.run([host['path'], 'browser'], capture_output=True, timeout=30)\n"
        "assert hand.returncode == 2 and hand.stdout == b\"\", hand\n"
        "print('OK: the kiosk host speaks the framing and takes only the origin')\n"
        "PY"
    )
  '';
}
