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
    # Generic icons shared-mime-info 2.5.1 added (mail, contacts, calendar).
    {
      name = "emblem-mail";
      token = "mail";
    }
    {
      name = "x-office-address-book";
      token = "book";
    }
    {
      name = "x-office-calendar";
      token = "document";
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
      mime = "text/x-common-lisp";
      token = "common-lisp";
    }
    {
      mime = "text/x-shellscript";
      token = "terminal";
    }
    {
      # Upstream since shared-mime-info 2.5.1 (invented here before).
      mime = "text/x-nix";
      token = "nix";
    }
    {
      # Upstream since 2.5.1, both *.step and *.stp (invented here before).
      mime = "model/step";
      token = "mesh";
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
    # THE SHAPES THE DATABASE ALREADY KNOWS. All of them are generically
    # `image-x-generic`, so without these a `.stl` wears the photograph's
    # glyph — correct by freedesktop's lights and useless in a listing, which
    # is exactly what this block is for. (`model/step`, above, and
    # `application/x-amf`, below, joined them with shared-mime-info 2.5.1.)
    #
    # The three the database does NOT know are in `ours`, further down, and the
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
    # Upstream since 2.5.1 as the Additive Manufacturing File, `*.amf`
    # (invented here as `model/x-amf` before, under the same glob). The name
    # is also what Adobe's Action Message Format travels as over HTTP, which
    # is why the invented type avoided it; freedesktop's database gives it
    # only to the 3D format, and a `.amf` file is that.
    {
      mime = "application/x-amf";
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
  #
  # `*.obj` HAS A THIRD CLAIMANT since shared-mime-info 2.5.1:
  # `application/x-coff`, a Windows object file, at the same weight and FIRST
  # in `globs2` — so with TGIF gone a `.obj` mesh tied with object code, and a
  # bare one (no `mtllib`, no exporter's banner) came out as object code. The
  # bytes cannot break that tie here: COFF's magic is `little16` words, which
  # `gen-mimedb.py` skips. A model is what an `.obj` is on this machine, so
  # each loser is a row, and each is refused on its own the day it goes stale.
  prefer = [
    {
      glob = "*.obj";
      mime = "model/obj";
      over = "application/x-tgif";
    }
    {
      glob = "*.obj";
      mime = "model/obj";
      over = "application/x-coff";
    }
  ];

  # A NAME A TYPE OF THE DATABASE ANSWERS HERE, AND UPSTREAM DOES NOT GIVE IT.
  #
  # Common Lisp (2026-10-20). `*.lsp` is one of its extensions, and upstream
  # gives it no type at all: a `.lsp` was plain text, with the text glyph and
  # no highlighting. A row adds the glob to a type the database HAS, at the
  # database's own weight, after upstream's rows — `ours` is for a type that
  # does not exist, and this one does.
  #
  # `*.cl` IS CLAIMED TWICE, ON PURPOSE. Upstream gives it to OpenCL
  # (`text/x-opencl-csrc`; `text/x-opencl-src` is its alias since
  # shared-mime-info 2.5.1), and Common Lisp writes `.cl` too. A row with
  # `shares` joins upstream's claimant at the same weight: a TIE, which
  # `mime::named` reports as unsettled and `mime::of_path` reads the head for
  # (the `*.ogg` case). There the magic decides first; then the first row
  # here whose glob matches and one of whose `content` markers is in the head
  # — markers only a Lisp file writes; and otherwise upstream's type stands,
  # so an OpenCL kernel, which writes none of them, stays OpenCL.
  #
  # AND IT REFUSES TO GO STALE: the type has to exist, and may not already
  # claim the glob upstream at 50 or more (the row would add nothing); a row
  # that does not say `shares` has to be its glob's only claimant (or it
  # would tie in silence); the types it `shares` with — one, or a list — have
  # to be exactly the glob's other claimants upstream, at the same weight;
  # and `content` comes with `shares`, and only with it.
  #
  # TYPESCRIPT (`application/typescript`, upstream since shared-mime-info
  # 2.5.1) is claimed the same way, on two names. Upstream gives it `*.ts` at
  # weight 40, under the MPEG transport stream and the Qt Linguist catalogue
  # at 50 — `.ts` meant a transport stream years before the language existed,
  # and the database is right to keep it — and does not give it `*.tsx`,
  # which is a Tiled tileset's. By name alone a TypeScript file was never
  # TypeScript here. The rows join it to both ties: a stream and a catalogue
  # have magic, which decides first; the markers below are what a script
  # writes and they do not; and with neither, upstream's first claimant
  # stands. Not a bare `export ` for `*.tsx`: a tileset writes
  # `<export target=…>`.
  claims = [
    {
      glob = "*.lsp";
      mime = "text/x-common-lisp";
    }
    {
      glob = "*.cl";
      mime = "text/x-common-lisp";
      shares = "text/x-opencl-csrc";
      content = [
        "(in-package"
        "(defpackage"
        "(defsystem"
        "(uiop:define-package"
      ];
    }
    {
      glob = "*.ts";
      mime = "application/typescript";
      shares = [
        "text/vnd.trolltech.linguist"
        "video/mp2t"
      ];
      content = [
        "import "
        "export "
        "const "
        "interface "
        "declare "
        "function "
        "require("
      ];
    }
    {
      glob = "*.tsx";
      mime = "application/typescript";
      shares = "application/x-tiled-tsx";
      content = [
        "import "
        "export default"
        "export const"
        "export function"
        "const "
        "function "
      ];
    }
  ];

  # A NAME UPSTREAM GIVES A TYPE ITS FILES ARE NOT.
  #
  # `*.fasl` is Common Lisp's COMPILED file — bytes for one implementation's
  # loader — and upstream gives it `text/x-common-lisp`, under `text/plain`:
  # the preview would show a fasl as text and colour it as Lisp. The row
  # removes the glob; with no claimant left, a fasl is what its bytes say, or
  # unknown, which is the truth. Refused when upstream stops making the
  # claim, since the row would then be doing nothing while looking deliberate.
  drop = [
    {
      glob = "*.fasl";
      mime = "text/x-common-lisp";
    }
  ];

  # WHAT FREEDESKTOP DOES NOT KNOW, AND THIS MACHINE DOES.
  #
  # `shared-mime-info` had no type for a Nix expression until 2.5.1, so every
  # `.nix` file on a NixOS distribution — the whole of this repository and the
  # whole of `/etc/nixos` — came back as plain text; this block is where
  # `text/x-nix` was invented then. These rows are added to the glob table
  # with the database's own default weight, and they are OURS: the `x-` prefix
  # says nobody registered them, which is the truth.
  #
  # The bar for adding one is that upstream has no type at all. A type that
  # exists and is merely inconvenient belongs in `exact` (its glyph) or in
  # `claims` (a name it is given too weakly). And the generator refuses a row
  # on a name upstream gives to ANY type, at any weight: at 50 or more the
  # row ties or loses in silence, below it hides upstream's own answer. Both
  # happened before that was asked — `model/x-amf` lost `*.amf` to upstream's
  # `application/x-amf`, and `text/x-typescript` never once won `*.ts` or
  # `*.tsx`.
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
    # (TYPESCRIPT was invented here as `text/x-typescript` until upstream named
    # it `application/typescript` in 2.5.1; its names are in `claims` now.)
    #
    # ZIG, which upstream has no type for at all.
    {
      mime = "text/x-zig";
      glob = "*.zig";
      under = "text/plain";
      token = "code";
    }

    # THE THREE 3D FORMATS THE DATABASE HAS NO TYPE FOR.
    #
    # Checked against the pinned `globs2` (shared-mime-info 2.5.1): not one of
    # `*.ply`, `*.off` or `*.dae` has a claimant upstream. There were five:
    # 2.5.1 gained `model/step` (`*.step`, `*.stp`) and `application/x-amf`
    # (`*.amf`), which are in `exact` now. The generator refused the STEP rows
    # by name, because the type itself arrived; the AMF one tied in silence
    # under another name, which is why it now refuses an `ours` row on any
    # name upstream gives. The day one of these three gains a claimant, the
    # build says so.
    #
    # `model/vnd.collada+xml` is COLLADA's registered name, so it is used
    # rather than an `x-` one invented here: what is missing upstream is the
    # entry, not the name.
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
  ]
  # THE SOUNDS ONLY THE PLAYER'S DECODERS OPEN, declared where the player's
  # desktop entry and the system database read them too.
  ++ (import ../player-types.nix).rows;
}
