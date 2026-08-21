#!/usr/bin/env python3
"""Packs an unpacked extension into a signed CRX3, and prints its id.

# Why this exists at all

Chrome will not install an extension from a directory unless somebody turns on
developer mode and clicks — which is imperative, and invariant 6 says that if it
is not in the flake it does not exist. What Chrome WILL do declaratively is
honour an enterprise policy that force-installs an extension from an update
manifest, and that needs a signed `.crx` and a stable id.

# Why the key is not in the repository

The id is derived from the signing key, so a stable id needs a stable key — and
a private key in a public repository is exactly what `nix/modules/secrets.nix`
refuses, in the sentence that lists "private SSH key, wifi PSK, token". So the
key is generated ON THE MACHINE by the unit that calls this, and the id is
computed here, at that moment, and written into the policy at that moment. The
unit is declared in the flake; what it produces is machine-local state. That is
the same arrangement the Wi-Fi secret already uses.

# The format, which is documented nowhere convenient

    "Cr24" | u32le version=3 | u32le header_len | CrxFileHeader | zip

`CrxFileHeader` is protobuf:

    message AsymmetricKeyProof { bytes public_key = 1; bytes signature = 2; }
    message CrxFileHeader {
      repeated AsymmetricKeyProof sha256_with_rsa = 2;
      bytes signed_header_data = 10000;
    }
    message SignedData { bytes crx_id = 1; }

and the signature covers

    b"CRX3 SignedData\\x00" | u32le len(signed_header_data) | signed_header_data | zip

The id is the first sixteen bytes of the SHA-256 of the DER public key, with
each nibble written as a letter from `a` to `p` — Chrome's own alphabet, chosen
so an id is never mistaken for hex.
"""

import argparse
import hashlib
import pathlib
import struct
import subprocess
import sys
import tempfile
import zipfile


def varint(value: int) -> bytes:
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        out.append(byte | (0x80 if value else 0))
        if not value:
            return bytes(out)


def field(number: int, payload: bytes) -> bytes:
    """One length-delimited protobuf field (wire type 2)."""
    return varint((number << 3) | 2) + varint(len(payload)) + payload


def crx_id(public_der: bytes) -> bytes:
    return hashlib.sha256(public_der).digest()[:16]


def readable_id(raw: bytes) -> str:
    """The id as Chrome writes it: sixteen bytes, thirty-two letters a–p."""
    return "".join("abcdefghijklmnop"[b >> 4] + "abcdefghijklmnop"[b & 0x0F] for b in raw)


def archive(source: pathlib.Path) -> bytes:
    """The extension as a zip, byte-for-byte reproducible.

    A fixed timestamp and a sorted walk: without them the same directory packs
    into two different files, the signature changes, and Chrome treats every
    rebuild as a different extension.
    """
    with tempfile.TemporaryDirectory() as work:
        path = pathlib.Path(work) / "e.zip"
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as zip_file:
            for entry in sorted(source.rglob("*")):
                if not entry.is_file():
                    continue
                info = zipfile.ZipInfo(
                    str(entry.relative_to(source)), date_time=(1980, 1, 1, 0, 0, 0)
                )
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o644 << 16
                zip_file.writestr(info, entry.read_bytes())
        return path.read_bytes()


def sign(key: pathlib.Path, payload: bytes) -> bytes:
    """RSA-SHA256, through openssl.

    The command line and not a Python library: adding `cryptography` to this
    build would be a dependency the whole closure carries so that one file can
    be signed once at first boot, and openssl is already there for the token.
    """
    with tempfile.TemporaryDirectory() as work:
        blob = pathlib.Path(work) / "to-sign"
        blob.write_bytes(payload)
        done = subprocess.run(
            ["openssl", "dgst", "-sha256", "-sign", str(key), str(blob)],
            capture_output=True,
            check=False,
        )
        if done.returncode != 0:
            sys.exit(f"ERRO: openssl recusou assinar: {done.stderr.decode(errors='replace')}")
        return done.stdout


def public_der(key: pathlib.Path) -> bytes:
    done = subprocess.run(
        ["openssl", "rsa", "-in", str(key), "-pubout", "-outform", "DER"],
        capture_output=True,
        check=False,
    )
    if done.returncode != 0:
        sys.exit(f"ERRO: openssl nao leu a chave: {done.stderr.decode(errors='replace')}")
    return done.stdout


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=pathlib.Path)
    parser.add_argument("--key", required=True, type=pathlib.Path)
    parser.add_argument("--out", required=True, type=pathlib.Path)
    parser.add_argument(
        "--id-file",
        type=pathlib.Path,
        help="onde escrever o id, para quem precisa dele sem reler o .crx",
    )
    args = parser.parse_args()

    if not args.source.is_dir():
        sys.exit(f"ERRO: {args.source} nao e um diretorio")
    if not (args.source / "manifest.json").is_file():
        sys.exit(f"ERRO: {args.source} nao tem manifest.json")

    der = public_der(args.key)
    identifier = crx_id(der)
    signed_header = field(1, identifier)
    zip_bytes = archive(args.source)

    to_sign = (
        b"CRX3 SignedData\x00"
        + struct.pack("<I", len(signed_header))
        + signed_header
        + zip_bytes
    )
    signature = sign(args.key, to_sign)

    proof = field(1, der) + field(2, signature)
    header = field(2, proof) + field(10000, signed_header)

    args.out.write_bytes(
        b"Cr24" + struct.pack("<II", 3, len(header)) + header + zip_bytes
    )
    name = readable_id(identifier)
    if args.id_file:
        args.id_file.write_text(name + "\n", encoding="utf-8")
    print(name)


if __name__ == "__main__":
    main()
