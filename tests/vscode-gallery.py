"""A stand-in for the Visual Studio Marketplace, for tests/vscode-vm.nix.

The VM has no network, and VS Code's `code --install-extension` asks the
Marketplace by its real names — the test's /etc/hosts sends them here, and
the test's own CA (trusted by the machine) signs this server's certificate
for every one of them. It answers what the pinned 1.119 asks for an install
(read in its `cliProcessMain.js`):

- GET  marketplace.visualstudio.com /_apis/public/gallery/vscode/<p>/<n>/latest
       the extension as the gallery describes it, or 404;
- POST marketplace.visualstudio.com /_apis/public/gallery/extensionquery
       the query VS Code falls back to — by name, the same descriptions;
- GET  <p>.gallery.vsassets.io and <p>.gallerycdn.vsassets.io
       …/Microsoft.VisualStudio.Services.VSIXPackage — the VSIX itself, and
       …/Microsoft.VisualStudio.Code.Manifest — its package.json;
- GET  main.vscode-cdn.net /extensions/marketplace.json — an empty control
       list.

And what the editor's window asks at start-up (the start-up measure), the
whole conversation and not its first line:

- every answer carries the CORS headers, and OPTIONS answers the preflight
  the window's `fetch` sends first (without them the window stops at the
  preflight, and a measure sees only the first question);
- whatever VSIX the folder holds is offered — the test puts a Copilot Chat
  NEWER than the one the pinned build carries beside rust-analyzer, so the
  window's update of its built-in goes all the way to the download;
- GET  main.vscode-cdn.net /core/stable.json — no emergency alert, and
       /extensions/chat.json — an empty list of chat participants;
- GET  redirector.gvt1.com /edgedl/… — a redirect to one of its mirrors, as
       the real one answers, and the mirror answers with bytes.

And Nix's two hosts, for the measure of rust-analyzer's own NixOS patch on a
machine that is online (the test sends their names here only then):

- GET  channels.nixos.org /flake-registry.json — the global flake registry,
       in the real one's shape (`nixpkgs` → GitHub, which the machine's own
       registry comes before);
- GET  cache.nixos.org /nix-cache-info — a binary cache, which has none of
       the outputs asked for (404).

Everything else is 404. Every request is written, a JSON line, to the log
the test reads: which hosts VS Code reached, against the hosts the person
let. No signature is offered: VS Code takes an unsigned gallery extension
("NotSigned") of a public publisher only where `extensions.verifySignature`
is off (the test's settings say where).

And every name a client ASKED for in its TLS greeting (SNI), a JSON line to
a second log, before the handshake is decided: the start-up measure sends
every name to this machine, so a host whose name the certificate does not
carry is still written down — the handshake fails, the attempt does not.

usage: vscode-gallery.py <vsix folder> <cert> <key> <log> <sni log>
"""

import http.server
import json
import os
import ssl
import sys
import uuid
import zipfile
from xml.etree import ElementTree

VSIX_DIR, CERT, KEY, LOG, SNI_LOG = sys.argv[1:6]
# THE MIRROR redirector.gvt1.com sends a download to, shaped as its own.
MIRROR = "r1---sn-ronoctua.gvt1.com"
VSIX = "Microsoft.VisualStudio.Services.VSIXPackage"
MANIFEST = "Microsoft.VisualStudio.Code.Manifest"


def read_vsix(path):
    with zipfile.ZipFile(path) as z:
        package = json.loads(z.read("extension/package.json"))
        manifest = ElementTree.fromstring(z.read("extension.vsixmanifest"))
    identity = manifest.find(".//{*}Identity")
    return {
        "publisher": package["publisher"].lower(),
        "name": package["name"].lower(),
        "version": package["version"],
        "engine": package["engines"]["vscode"],
        "target": identity.get("TargetPlatform") if identity is not None else None,
        "package": json.dumps(package).encode(),
        "path": path,
    }


# THE GLOBAL FLAKE REGISTRY, shaped as the one at channels.nixos.org.
GLOBAL_REGISTRY = {"version": 2, "flakes": [{
    "from": {"type": "indirect", "id": "nixpkgs"},
    "to": {"type": "github", "owner": "NixOS", "repo": "nixpkgs", "ref": "nixpkgs-unstable"},
}]}

EXTENSIONS = {}
for entry in sorted(os.listdir(VSIX_DIR)):
    if entry.endswith(".vsix"):
        e = read_vsix(os.path.join(VSIX_DIR, entry))
        EXTENSIONS[f"{e['publisher']}.{e['name']}"] = e


def described(e):
    """The extension as the gallery describes one (`IRawGalleryExtension`)."""
    p, n, v = e["publisher"], e["name"], e["version"]
    asset = f"https://{p}.gallerycdn.vsassets.io/extensions/{p}/{n}/{v}/1790000000000"
    fallback = (f"https://{p}.gallery.vsassets.io/_apis/public/gallery/publisher/{p}"
                f"/extension/{n}/{v}/assetbyname")
    version = {
        "version": v,
        "flags": "validated",
        "lastUpdated": "2026-10-12T00:00:00.000Z",
        "files": [{"assetType": t, "source": f"{asset}/{t}"} for t in (MANIFEST, VSIX)],
        "properties": [
            {"key": "Microsoft.VisualStudio.Code.Engine", "value": e["engine"]},
            {"key": "Microsoft.VisualStudio.Code.ExtensionDependencies", "value": ""},
            {"key": "Microsoft.VisualStudio.Code.ExtensionPack", "value": ""},
        ],
        "assetUri": asset,
        "fallbackAssetUri": fallback,
    }
    if e["target"]:
        version["targetPlatform"] = e["target"]
    return {
        "publisher": {
            "publisherId": str(uuid.uuid5(uuid.NAMESPACE_DNS, p)),
            "publisherName": p,
            "displayName": p,
            "flags": "verified",
            "domain": None,
            "isDomainVerified": False,
        },
        "extensionId": str(uuid.uuid5(uuid.NAMESPACE_DNS, f"{p}.{n}")),
        "extensionName": n,
        "displayName": n,
        "shortDescription": n,
        "flags": "validated, public",
        "lastUpdated": "2026-10-12T00:00:00.000Z",
        "publishedDate": "2026-10-12T00:00:00.000Z",
        "releaseDate": "2026-10-12T00:00:00.000Z",
        "versions": [version],
        "categories": [],
        "tags": [],
        "statistics": [],
        "installationTargets": [{"target": "Microsoft.VisualStudio.Code", "targetVersion": ""}],
        "deploymentType": 0,
    }


class Gallery(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_request(self, code="-", size="-"):
        host = (self.headers.get("Host") or "").split(":")[0].lower()
        with open(LOG, "a") as out:
            out.write(json.dumps({"method": self.command, "host": host,
                                  "path": self.path, "code": code}) + "\n")

    def end_headers(self):
        """Every answer may be read by the window that asked (CORS)."""
        origin = self.headers.get("Origin")
        if origin:
            self.send_header("Access-Control-Allow-Origin", origin)
            self.send_header("Access-Control-Allow-Credentials", "true")
            self.send_header("Vary", "Origin")
        super().end_headers()

    def send(self, code, body, kind="application/json", extra=()):
        self.send_response(code)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        for name, value in extra:
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self):
        """The preflight of the window's `fetch`: anything it asks to send."""
        asked = self.headers.get("Access-Control-Request-Headers") or "*"
        self.send(204, b"", "text/plain", (
            ("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
            ("Access-Control-Allow-Headers", asked),
            ("Access-Control-Max-Age", "600"),
        ))

    def host(self):
        return (self.headers.get("Host") or "").split(":")[0].lower()

    def do_GET(self):
        host, path = self.host(), self.path.split("?")[0]
        if host == "marketplace.visualstudio.com" and path.endswith("/latest"):
            parts = path.split("/")
            e = EXTENSIONS.get(f"{parts[-3].lower()}.{parts[-2].lower()}")
            if e:
                return self.send(200, json.dumps(described(e)).encode())
            return self.send(404, b"{}")
        if host.endswith(".gallery.vsassets.io") or host.endswith(".gallerycdn.vsassets.io"):
            for e in EXTENSIONS.values():
                if f"/{e['name']}/{e['version']}/" not in path:
                    continue
                if path.endswith("/" + VSIX):
                    with open(e["path"], "rb") as f:
                        return self.send(200, f.read(), "application/octet-stream")
                if path.endswith("/" + MANIFEST):
                    return self.send(200, e["package"])
            return self.send(404, b"")
        if host == "main.vscode-cdn.net" and path == "/extensions/marketplace.json":
            return self.send(200, b"{}")
        if host == "main.vscode-cdn.net" and path == "/core/stable.json":
            return self.send(200, b'{"alerts": []}')
        if host == "main.vscode-cdn.net" and path == "/extensions/chat.json":
            return self.send(200, b"{}")
        if host == "redirector.gvt1.com" and path.startswith("/edgedl/"):
            where = f"https://{MIRROR}{path}?cms_redirect=yes"
            return self.send(302, b"", "text/plain", (("Location", where),))
        if host == MIRROR and path.startswith("/edgedl/"):
            return self.send(200, b"not a dictionary", "application/octet-stream")
        if host == "channels.nixos.org" and path == "/flake-registry.json":
            return self.send(200, json.dumps(GLOBAL_REGISTRY).encode())
        if host == "cache.nixos.org" and path == "/nix-cache-info":
            return self.send(200, b"StoreDir: /nix/store\nWantMassQuery: 1\nPriority: 40\n",
                             "text/x-nix-cache-info")
        return self.send(404, b"")

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else b"{}"
        if self.host() == "marketplace.visualstudio.com" and self.path.split("?")[0].endswith(
                "/extensionquery"):
            try:
                query = json.loads(body)
            except ValueError:
                query = {}
            names = [c.get("value", "").lower()
                     for f in query.get("filters", []) for c in f.get("criteria", [])
                     if c.get("filterType") == 7]
            found = [described(EXTENSIONS[n]) for n in names if n in EXTENSIONS]
            answer = {"results": [{
                "extensions": found,
                "pagingToken": None,
                "resultMetadata": [{"metadataType": "ResultCount",
                                    "metadataItems": [{"name": "TotalCount", "count": len(found)}]}],
            }]}
            return self.send(200, json.dumps(answer).encode())
        return self.send(404, b"")


server = http.server.ThreadingHTTPServer(("0.0.0.0", 443), Gallery)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(CERT, KEY)


def asked(_socket, name, _context):
    """Write down the name a client asked for; the handshake goes on."""
    with open(SNI_LOG, "a") as out:
        out.write(json.dumps({"sni": (name or "").lower()}) + "\n")


context.sni_callback = asked
server.socket = context.wrap_socket(server.socket, server_side=True)
print(f"serving {sorted(EXTENSIONS)}", flush=True)
server.serve_forever()
