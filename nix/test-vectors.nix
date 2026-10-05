# FILES OTHER PEOPLE PUBLISHED FOR TESTING, pinned — what a check feeds a
# test that is `#[ignore]`d in a plain `cargo test` because the file is not
# in this tree.
#
# Each is a CAPTURE of the published address (`downloads-are-pinned`): the
# address itself may serve other bytes tomorrow. And each is held by the
# garbage-collector root of `nix/kept-sources.nix`, so a collection does not
# make the next check download it again.
{ fetchurl }:
{
  # OpenSubtitles' `breakdance.avi`, the published vector of its moviehash
  # (`8e245d9679d31e12`, 12 909 756 bytes): every capture of it since 2013
  # has the same digest, and these are those bytes (measured 2026-10-04).
  breakdance = fetchurl {
    url = "https://web.archive.org/web/20250818074934id_/http://www.opensubtitles.org/addons/avi/breakdance.avi";
    hash = "sha256-LS91kC1l5wZULZre24F0cnQpoeKXGdKIcL4NXRvwJX0=";
  };
}
