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

Everything else is 404. Every request is written, a JSON line, to the log
the test reads: which hosts VS Code reached, against the hosts the person
let. No signature is offered: VS Code takes an unsigned gallery extension
("NotSigned") of a public publisher.

usage: vscode-gallery.py <vsix folder> <cert> <key> <log>
"""

import http.server
import json
import os
import ssl
import sys
import uuid
import zipfile
from xml.etree import ElementTree

VSIX_DIR, CERT, KEY, LOG = sys.argv[1:5]
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

    def send(self, code, body, kind="application/json"):
        self.send_response(code)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

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
server.socket = context.wrap_socket(server.socket, server_side=True)
print(f"serving {sorted(EXTENSIONS)}", flush=True)
server.serve_forever()
