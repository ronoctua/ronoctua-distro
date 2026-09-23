# The cursors: one Xcursor theme per cursor colour, `ronoctua-pointer-1` … `-8`.
#
# # Why these exist
#
# Every seat draws its own cursor, and a person with two mice has to be able
# to tell which hand is which at a glance. The compositor takes a cursor
# THEME per seat (`seat <s> xcursor_theme <name> <size>`), so a colour per
# cursor is a theme per colour; `ronoctua-pointers` writes which seat wears
# which.
#
# # Where the shapes come from
#
# Bibata Modern's SVG sources — the same pinned source nixpkgs builds its own
# `bibata-cursors` from (`bibata-cursors.src`), so nothing new is downloaded.
# Its SVGs carry three placeholder colours that its own build replaces:
# `#00FF00` the inside, `#0000FF` the rim, `#FF0000` the inside again in the
# few shapes that repeat it. The rest — the ring of the busy cursor — keeps
# its own colours.
#
# # Where the colours come from
#
# `gen-theme.py`, as `cursor-colours` beside every theme: the eight insides
# and the rim, solved and gated there (`cursors` in the report). They are the
# SAME in every theme, by design, which is why this reads the default theme's
# file and builds eight themes rather than eight per theme.
{
  lib,
  runCommand,
  bibata-cursors,
  clickgen,
  librsvg,
  themed,
  defaultTheme,
}:

let
  source = bibata-cursors.src;
in
runCommand "ronoctua-cursors"
  {
    nativeBuildInputs = [
      clickgen
      librsvg
    ];
  }
  ''
    colours=${themed}/${defaultTheme}/cursor-colours
    test -s "$colours" || { echo "no cursor colours in $colours" >&2; exit 1; }

    cp -r ${source}/configs configs
    chmod -R u+w configs
    # THE SIZES A PERSON CAN PICK, and no more. Bibata ships fourteen; each is
    # a bitmap in every file, and eight themes of fourteen sizes is weight for
    # sizes nobody chose. The screen offers these (`prefs::SIZES` in
    # `ronoctua-pointers`). Measured: with 16 and 96 added the eight themes
    # were 92 MB; with these four, 43 MB — the 96 px bitmaps alone were half.
    sed -i 's/^x11_sizes = .*/x11_sizes = [24, 32, 48, 64]/' configs/normal/x.build.toml
    grep -q '^x11_sizes = \[24, 32, 48, 64\]' configs/normal/x.build.toml

    n=0
    while read -r fill rim; do
      n=$((n + 1))
      bitmaps="bitmaps-$n"
      mkdir -p "$bitmaps"
      # EVERY SVG, frames of the animated ones included — they live in a
      # folder each and the build wants them flat, `wait-01.png`.
      find -L ${source}/svg/modern -name '*.svg' | while read -r svg; do
        name=$(basename "$svg" .svg)
        sed -e "s/#00FF00/$fill/gI" -e "s/#0000FF/$rim/gI" -e "s/#FF0000/$fill/gI" "$svg" > "$name.svg"
        rsvg-convert --width 256 --height 256 "$name.svg" -o "$bitmaps/$name.png"
        rm "$name.svg"
      done
      ctgen configs/normal/x.build.toml -p x11 -d "$bitmaps" \
        -n "ronoctua-pointer-$n" -c "ronoctua cursor, colour $n"
    done < "$colours"
    test "$n" = 8 || { echo "expected eight cursor colours, found $n" >&2; exit 1; }

    mkdir -p $out/share/icons
    cp -r themes/* $out/share/icons/
    for n in 1 2 3 4 5 6 7 8; do
      test -f $out/share/icons/ronoctua-pointer-$n/cursors/left_ptr \
        || { echo "ronoctua-pointer-$n has no left_ptr" >&2; exit 1; }
    done
  ''
