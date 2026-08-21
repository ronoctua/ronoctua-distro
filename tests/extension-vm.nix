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
#   3. the two files Chrome reads agree with each other AND with the `.crx`.
#      The id appears in three places and they are written by one script for
#      exactly this reason; nothing checks that at eval time;
#   4. the signature verifies the way Chrome verifies it — over
#      `"CRX3 SignedData\0" | len | signed_header_data | zip`, which is
#      documented in no convenient place and which a refactor can silently
#      shift by four bytes.
#
# WHAT IT DELIBERATELY DOES NOT PROVE
# ------------------------------------
# That Chrome accepts the policy. That needs Chrome, a display and a login, and
# a VM that started a browser to find out would be testing Google's release
# schedule. `docs/PENDING.md` records it as the owner's to confirm.
{
  pkgs,
  lib,
  ronoctua-shell,
  ronoctua-transfers,
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
        sys.exit("ERRO: o .crx nao comeca com Cr24")
    version, header_len = struct.unpack("<II", crx[4:12])
    if version != 3:
        sys.exit(f"ERRO: CRX versao {version}, e o Chrome atual so le a 3")
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
                sys.exit("ERRO: campo protobuf que nao e delimitado por tamanho")
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
        sys.exit(f"ERRO: o id gravado ({written}) nao e o da chave ({derived})")

    policy = json.loads(
        pathlib.Path("/etc/opt/chrome/policies/managed/ronoctua.json").read_text()
    )
    settings = policy["ExtensionSettings"]
    if list(settings) != [derived]:
        sys.exit(f"ERRO: a politica nomeia {list(settings)} e nao {derived}")
    if settings[derived]["installation_mode"] != "force_installed":
        sys.exit("ERRO: a politica nao forca a instalacao")

    host = json.loads(
        pathlib.Path(
            "/etc/opt/chrome/native-messaging-hosts/br.ronoctua.transfers.json"
        ).read_text()
    )
    origins = host["allowed_origins"]
    if origins != [f"chrome-extension://{derived}/"]:
        sys.exit(f"ERRO: o host aceita {origins}, e nao so a nossa extensao")
    if not pathlib.Path(host["path"]).is_file():
        sys.exit(f"ERRO: o host aponta para {host['path']}, que nao existe")

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
        sys.exit("ERRO: a assinatura do .crx NAO confere do jeito que o Chrome confere")

    print(f"OK: {derived}, assinatura confere, e os tres lugares concordam")
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
            description = "O navegador — aqui so a chave que o modulo le.";
          };
        }
      )
    ];

    virtualisation.memorySize = 1024;
    virtualisation.cores = 2;

    _module.args.ronoctua = {
      transfers = ronoctua-transfers;
      inherit extension;
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
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("ronoctua-browser-extension.service")

    # 1. THE KEY, and its permissions.
    machine.succeed("test -f /var/lib/ronoctua-browser/key.pem")
    perms = machine.succeed("stat -c '%a %U' /var/lib/ronoctua-browser/key.pem").strip()
    assert perms == "600 root", f"a chave esta {perms}, e devia ser 600 root"
    home = machine.succeed("stat -c '%a' /var/lib/ronoctua-browser").strip()
    assert home == "700", f"o diretorio esta {home}, e devia ser 700"

    # 2. EVERYTHING AGREES, and the signature is the one Chrome checks.
    print(machine.succeed("python3 ${verify}"))

    # 3. A SECOND RUN KEEPS THE ID. Regenerating the key would leave the old
    #    extension force-installed by a policy nothing writes any more, and take
    #    the person's switch and history with it.
    before = machine.succeed("cat /var/lib/ronoctua-browser/id").strip()
    machine.succeed("systemctl restart ronoctua-browser-extension.service")
    machine.wait_for_unit("ronoctua-browser-extension.service")
    after = machine.succeed("cat /var/lib/ronoctua-browser/id").strip()
    assert before == after, f"o id mudou entre execucoes: {before} -> {after}"
    print(machine.succeed("python3 ${verify}"))

    # 4. AND THE HOST BINARY ANSWERS Chrome's framing.
    #
    #    Four bytes of length and then JSON, both ways. A `ping` needs no queue
    #    behind it, which is the point: what is under test is the framing and
    #    the binary being where the manifest says it is.
    machine.succeed(
        "python3 - <<'PY'\n"
        "import json, struct, subprocess, pathlib\n"
        "host = json.loads(pathlib.Path('/etc/opt/chrome/native-messaging-hosts/br.ronoctua.transfers.json').read_text())\n"
        "body = json.dumps({'kind': 'ping'}).encode()\n"
        "done = subprocess.run([host['path'], 'browser'],\n"
        "    input=struct.pack('<I', len(body)) + body, capture_output=True, timeout=30)\n"
        "out = done.stdout\n"
        "assert len(out) > 4, f'sem resposta: {done.stderr!r}'\n"
        "size = struct.unpack('<I', out[:4])[0]\n"
        "answer = json.loads(out[4:4 + size])\n"
        "assert answer.get('ok') is True, answer\n"
        "print('OK: o host responde ao enquadramento do Chrome')\n"
        "PY"
    )
  '';
}
