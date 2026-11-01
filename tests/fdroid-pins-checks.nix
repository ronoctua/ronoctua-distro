# THE DISTRO'S APK PINS ARE WHAT F-DROID'S SIGNED INDEX SIGNS (2026-11-01).
#
#   - `distro-apk-pins-are-the-signed-index`: `ronoctua-fdroid-pins`, which
#     `nix run .#distro-pins` asks (`ronoctua_software::pins::signed`), over
#     PLANTED fixtures — an `entry.jar` signed here by a key of the test's
#     own, the way F-Droid signs its own (a PKCS#7 block over the `.SF`,
#     RSA and SHA-256), naming the cut index of the fixtures: a pin is the
#     version that index offers, by the hash it signs, and the pin the distro
#     holds is told against it (the same, another version, none, a package
#     the index does not list); a pin that claims the signed version code
#     with another hash or address DISAGREES; one byte of the index changed,
#     or an `entry.json` changed after it was signed, is refused; a jar
#     signed by another key — with its own certificate, or with the pinned
#     one — is refused; and the program says which file broke the chain,
#     and answers a wrong command line with its usage. The tool's side of it
#     (what it pins, keeps and refuses for each answer) is
#     `distro-pins-move-only-what-follows`.
#
# Run out of the harness the test units already built (`rust.testsOf`):
# exactly these five tests, or nothing was proved.
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{ rust }:
let
  tests = [
    "a_pin_is_what_the_signed_index_offers"
    "a_pin_that_claims_the_signed_version_with_other_bytes_disagrees"
    "a_tampered_index_or_entry_is_refused"
    "a_jar_signed_by_another_certificate_is_refused"
    "the_program_answers_from_files_and_says_its_refusals"
  ];
in
{
  distro-apk-pins-are-the-signed-index = rust.testsOf {
    name = "ronoctua-distro-apk-pins-are-the-signed-index";
    crate = "ronoctua-software";
    exact = map (t: "pins::tests::signed_index::${t}") tests;
    expect = "test result: ok. ${toString (builtins.length tests)} passed";
  };
}
