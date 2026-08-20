# Type of file → icon token, for the file manager's listing.
#
# THE OTHER END OF THIS TABLE IS FREEDESKTOP'S.
# ---------------------------------------------
# `shared-mime-info` knows 1,232 types and answers, for each, which GENERIC icon
# it belongs to: `x-office-spreadsheet`, `package-x-generic`, `text-x-generic`.
# There are seventeen of those names in the whole database. So this table has
# seventeen rows plus one per media type, and it covers every type there is —
# by construction, not by diligence.
#
# A hand-written extension→icon table was the obvious alternative and it is the
# wrong one: it would cover the extensions somebody remembered, look complete,
# and answer `generic` for everything else. `generic` is indistinguishable from
# "I did not resolve it" (see `assets/icons/category-glyphs.nix`), so the gap
# would not even show.
#
# The values are TOKENS from `assets/tokens/icons.nix`, never glyph names and
# much less codepoints (invariant 9). `nix/scripts/gen-mimedb.py` fails if a
# token is not there, and fails again if freedesktop names a generic icon this
# table does not answer for — which is how a new upstream release announces
# itself instead of quietly drawing a square.
#
# A LIST and not an attrset, for the reason the category table gives: an
# attrset is ordered by its key, and the specific has to come before the
# general.
{
  # The seventeen names `generic-icons` actually uses, plus the `<media>-x-generic`
  # fallbacks the spec derives for everything it does not list.
  generic = [
    # --- documents, and the things people call documents ---------------
    {
      name = "x-office-document";
      token = "document";
    }
    {
      name = "x-office-spreadsheet";
      token = "spreadsheet";
    }
    {
      name = "x-office-presentation";
      token = "presentation";
    }
    {
      name = "text-html";
      token = "browser";
    }
    {
      name = "text-x-script";
      token = "code";
    }
    {
      name = "text-x-generic-template";
      token = "file";
    }
    {
      name = "text-x-generic";
      token = "file";
    }

    # --- things with content of their own ------------------------------
    {
      name = "image-x-generic";
      token = "image";
    }
    {
      name = "audio-x-generic";
      token = "music";
    }
    {
      name = "video-x-generic";
      token = "video";
    }
    {
      name = "font-x-generic";
      token = "font";
    }
    {
      name = "package-x-generic";
      token = "archive";
    }
    {
      name = "application-x-executable";
      token = "executable";
    }

    # --- media one used to hold in a hand -------------------------------
    {
      name = "media-optical";
      token = "disc";
    }
    {
      name = "media-floppy";
      token = "save";
    }

    # --- the two that describe the entry rather than its content --------
    {
      name = "folder";
      token = "folder";
    }
    {
      name = "emblem-symbolic-link";
      token = "link";
    }

    # --- the `<media>-x-generic` fallbacks ------------------------------
    #
    # The spec derives these for a type `generic-icons` does not list, and
    # without them a whole media type would fall through to `generic`.
    {
      name = "application-x-generic";
      token = "file";
    }
    {
      name = "message-x-generic";
      token = "mail";
    }
    {
      name = "model-x-generic";
      token = "graphics";
    }
    {
      name = "multipart-x-generic";
      token = "package";
    }
    {
      name = "chemical-x-generic";
      token = "science";
    }
    {
      name = "inode-x-generic";
      token = "folder";
    }
    {
      name = "x-content-x-generic";
      token = "disk";
    }
    {
      name = "x-epoc-x-generic";
      token = "file";
    }
  ];

  # WHERE THE SPECIFIC BEATS THE GENERIC.
  #
  # A short list, and each row earns its place by being a type somebody
  # RECOGNISES on sight. `application/pdf` is `x-office-document` upstream,
  # which is correct and makes a PDF indistinguishable from a `.odt` in a
  # column of thirty rows.
  #
  # EVERY TYPE HERE HAS TO EXIST UPSTREAM, and the generator fails otherwise.
  # Five of the first rows written here did not: `application/x-iso9660-image`,
  # `application/x-sqlite3` and `text/x-rust` are all names one would swear are
  # right, and the database spells them `application/vnd.efi.iso`,
  # `application/vnd.sqlite3` and `text/rust`. A row naming a type that does
  # not exist is a row that matches nothing, looks deliberate, and nobody ever
  # audits.
  exact = [
    {
      mime = "application/pdf";
      token = "pdf";
    }
    {
      mime = "application/x-executable";
      token = "executable";
    }
    {
      mime = "application/x-sharedlib";
      token = "executable";
    }
    {
      mime = "inode/directory";
      token = "folder";
    }
    {
      mime = "inode/symlink";
      token = "link";
    }
    {
      mime = "application/vnd.efi.iso";
      token = "disc";
    }
    {
      mime = "text/rust";
      token = "rust";
    }
    {
      mime = "application/x-shellscript";
      token = "terminal";
    }
    {
      mime = "application/x-desktop";
      token = "window";
    }
    {
      mime = "application/vnd.sqlite3";
      token = "disk";
    }
    {
      mime = "application/x-x509-ca-cert";
      token = "security";
    }
    {
      mime = "application/pgp-keys";
      token = "key";
    }
  ];

  # WHAT FREEDESKTOP DOES NOT KNOW, AND THIS MACHINE DOES.
  #
  # `shared-mime-info` has no type for a Nix expression, so every `.nix` file on
  # a NixOS distribution — which is to say, the whole of this repository and the
  # whole of `/etc/nixos` — comes back as plain text. These rows are added to
  # the glob table with the database's own default weight, and they are OURS:
  # the `x-` prefix says nobody registered them, which is the truth.
  #
  # The bar for adding one is that upstream has no type at all. A type that
  # exists and is merely inconvenient belongs in `exact`.
  ours = [
    {
      mime = "text/x-nix";
      glob = "*.nix";
      token = "nix";
    }
  ];
}
