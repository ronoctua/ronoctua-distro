# Secure Boot keys for TESTING — never for real use.
#
# # Why it exists
#
# lanzaboote signs the UKI during the boot manager's installation, which
# in a test VM happens inside Nix's sandbox. The keys have to
# exist there; without them there is no signed UKI, without a signed UKI there is no
# PCR 11 measured, and without PCR 11 `tpm2-measure-pcr` is inert — milestone 8's
# whole chain depends on this.
#
# # This does NOT violate invariant 13
#
# The invariant forbids personal data IN THE REPOSITORY. These keys are generated
# at build time, live only in the store, and are public by construction: whoever
# has the store has the private key. That is exactly why they are no good
# for a real machine, and the file's name says so.
#
# The real keys of an installation are created on the computer itself, live
# in `/etc/secureboot`, and section 7.2 routes them to `/persist` — otherwise the
# root wipe would erase them and the machine would no longer boot.
#
# # Why `openssl`, and not `sbctl create-keys`
#
# `sbctl` uses landlock and requires `/var/lib`, which does not exist in Nix's
# sandbox; it fails with "populating ruleset for /var/lib: no such file or
# directory", which mentions no sandbox at all. The layout it expects afterwards
# is simple enough to be created directly — and that way there is no state outside
# `$out`.
{
  lib,
  runCommand,
  openssl,
  util-linux,
}:

runCommand "ronoctua-test-sb-keys"
  {
    nativeBuildInputs = [
      openssl
      util-linux
    ];
    meta = {
      description = "Disposable Secure Boot keys, for the VM tests alone";
      platforms = lib.platforms.linux;
    };
  }
  ''
    mkdir -p $out/keys/{PK,KEK,db}

    # The owner's GUID. Random: nothing depends on the value, and fixing it
    # would give the impression that it means something.
    #
    # WITHOUT the line break: `sbctl` reads the whole file as a UUID and
    # complains about "invalid UUID length: 37" — 36 characters plus the `\n`.
    printf '%s' "$(uuidgen)" > $out/GUID

    # One authority per level, as the Secure Boot chain expects:
    # PK signs KEK, KEK signs db, db signs the UKI.
    for level in PK KEK db; do
      openssl req -newkey rsa:2048 -nodes \
        -keyout $out/keys/$level/$level.key \
        -new -x509 -sha256 -days 3650 \
        -subj "/CN=ronoctua test $level/" \
        -out $out/keys/$level/$level.pem 2>/dev/null
    done

    # No real secret here, but the right mode all the same: a world-readable
    # `.key` in a tutorial becomes a world-readable `.key` on a
    # real machine.
    chmod 600 $out/keys/*/*.key

    # A warning in the directory itself. If one day someone points the production
    # `pkiBundle` here, let them at least find this.
    cat > $out/README.txt <<'NOTICE'
    DISPOSABLE Secure Boot keys, generated at build time.

    The private key is in /nix/store and is therefore public. They serve only for
    ronoctua-distro's VM tests. Do not point `boot.lanzaboote.pkiBundle`
    from a real machine at these.
    NOTICE
  ''
