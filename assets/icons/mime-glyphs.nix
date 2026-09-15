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
    # --- A LIST OF THINGS TO PLAY IS NOT A THING TO PLAY ----------------
    #
    # All six spellings, and they get ONE glyph on purpose. Five of them are
    # under `audio/`, so the generic rule drew them as music — which says the
    # file IS sound, and a playlist is a name for sound somebody else holds.
    # The sixth, `application/xspf+xml`, fell to the application generic and
    # came out as a plain file: two drawings for one kind of thing.
    #
    # AND THE `.m3u` PAIR IS WHY IT MATTERS. `audio/x-mpegurl` and
    # `application/vnd.apple.mpegurl` both claim `*.m3u` and `*.m3u8` at the
    # same weight, so which of them wins is decided by the bytes — see
    # `mime::of_path`. With one glyph for both, that decision stops being
    # visible, which is right: nobody looking at a listing can tell the two
    # names apart and nobody should have to.
    #
    # `list` and not `music`: it is the only token in `icons.nix` that says
    # "these are entries", and `ronoctua_inspect::playlist` makes the same
    # argument in words — what decides is not what a file is made of but what
    # it is FOR.
    {
      mime = "audio/x-mpegurl";
      token = "list";
    }
    {
      mime = "application/vnd.apple.mpegurl";
      token = "list";
    }
    {
      mime = "audio/x-scpls";
      token = "list";
    }
    {
      mime = "application/xspf+xml";
      token = "list";
    }
    {
      mime = "audio/x-ms-asx";
      token = "list";
    }
    {
      mime = "audio/x-iriver-pla";
      token = "list";
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
    # THE SHAPES THE DATABASE ALREADY KNOWS. All six are generically
    # `image-x-generic`, so without these a `.stl` wears the photograph's
    # glyph — correct by freedesktop's lights and useless in a listing, which
    # is exactly what this block is for.
    #
    # The five the database does NOT know are in `ours`, further down, and the
    # generator refuses to let a type appear in both.
    {
      mime = "model/stl";
      token = "mesh";
    }
    {
      mime = "model/3mf";
      token = "mesh";
    }
    {
      mime = "model/obj";
      token = "mesh";
    }
    {
      mime = "model/gltf+json";
      token = "mesh";
    }
    {
      mime = "model/gltf-binary";
      token = "mesh";
    }
    {
      mime = "model/iges";
      token = "mesh";
    }
  ];

  # WHEN TWO TYPES CLAIM THE SAME NAME AND UPSTREAM DOES NOT DECIDE.
  #
  # `globs2` has two rows for `*.obj` at the same weight — `application/x-tgif`
  # and `model/obj` — and `mime::by_name` keeps the FIRST of an equal pair, so
  # a `.obj` on this machine resolved to a 1990s X11 drawing program's document
  # and got no preview, no facts and the neutral glyph. Neither `exact` nor
  # `ours` can fix that: `exact` decides the icon and nothing else, and an
  # `ours` row joins the list at the END with the same weight 50 and loses the
  # same tie.
  #
  # WHAT THIS DOES IS REMOVE THE LOSER'S GLOB, and not raise the winner's
  # weight. Raising it would also beat a future claimant of HIGHER weight,
  # which is more than "TGIF does not get `.obj`" means.
  #
  # AND IT REFUSES TO GO STALE. The generator fails when either type is absent,
  # when either does not actually claim that pattern, or when their weights
  # already differ — the last because upstream would then have decided and this
  # row would be doing nothing while looking deliberate.
  prefer = [
    {
      glob = "*.obj";
      mime = "model/obj";
      over = "application/x-tgif";
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
  #
  # AND EACH ONE SAYS WHAT IT DESCENDS FROM, in `under`.
  #
  # A type invented here has no subclass entry in freedesktop's table, so
  # `mime::is_a` answers `false` for every question about it — and the first
  # thing that asked was the preview, whose "is this text?" is exactly
  # `is_a(mime, "text/plain")`. Without this field a `.nix` file would be the
  # one file on a NixOS machine whose content the preview refused to show.
  #
  # So a type is invented WHOLE here — its name, its glob, its icon and its
  # place in the hierarchy — rather than in four places that each know a
  # quarter of it.
  ours = [
    {
      mime = "text/x-nix";
      glob = "*.nix";
      under = "text/plain";
      token = "nix";
    }
    # TYPESCRIPT, and upstream's silence here is not an oversight.
    #
    # `*.ts` in shared-mime-info 2.4 is `video/mp2t` — an MPEG transport
    # stream — and the database is RIGHT: `.ts` meant that years before the
    # language existed. So there is no type to correct in `exact`; there is a
    # type that does not exist, which is what this block is for.
    #
    # It matters beyond the icon: `assets/syntax.nix` claims MIME types and is
    # refused if it invents one, so without this line a TypeScript file would
    # be offered a video's preview and no highlighting at all.
    {
      mime = "text/x-typescript";
      glob = "*.ts";
      under = "text/plain";
      token = "code";
    }
    {
      mime = "text/x-typescript";
      glob = "*.tsx";
      under = "text/plain";
      token = "code";
    }
    # ZIG, which upstream has no type for at all.
    {
      mime = "text/x-zig";
      glob = "*.zig";
      under = "text/plain";
      token = "code";
    }

    # THE FIVE 3D FORMATS THE DATABASE HAS NO TYPE FOR.
    #
    # Checked against the pinned `globs2`: not one of `*.ply`, `*.off`,
    # `*.dae`, `*.amf`, `*.step` or `*.stp` has a single claimant upstream. The
    # day one of them gains a type, the generator refuses this row by name and
    # says so — which is the behaviour wanted, because the row would then be a
    # second answer to a question the database had started answering.
    #
    # `model/vnd.collada+xml` is COLLADA's registered name, so it is used
    # rather than an `x-` one invented here: what is missing upstream is the
    # entry, not the name.
    #
    # `model/x-amf` and not `application/x-amf`, which Adobe's Action Message
    # Format has had for twenty years.
    {
      mime = "model/x-ply";
      glob = "*.ply";
      # A PLY may be words or bytes under the same extension, and the header
      # is what says which — so the honest parent is "some bytes", not text.
      under = "application/octet-stream";
      token = "mesh";
    }
    {
      mime = "model/x-off";
      glob = "*.off";
      under = "text/plain";
      token = "mesh";
    }
    {
      mime = "model/vnd.collada+xml";
      glob = "*.dae";
      under = "application/xml";
      token = "mesh";
    }
    {
      mime = "model/x-amf";
      glob = "*.amf";
      # Same argument as the PLY: the specification allows the file to be a zip
      # OR the bare XML, under one extension.
      under = "application/octet-stream";
      token = "mesh";
    }
    # STEP, under BOTH of its extensions. It is plain text, and saying so is
    # what lets the preview fall back to showing the source if the geometry
    # ever refuses.
    {
      mime = "model/step";
      glob = "*.step";
      under = "text/plain";
      token = "mesh";
    }
    {
      mime = "model/step";
      glob = "*.stp";
      under = "text/plain";
      token = "mesh";
    }
  ];
}
