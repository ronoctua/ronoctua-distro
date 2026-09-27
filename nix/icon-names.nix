# EVERY NAME THE SUPERPACK ANSWERS TO, one per line (`RONOCTUA_ICON_NAMES`).
#
# A web app's icon can come from "the distro's collection", and what a person
# types there is a NAME — `whatsapp`, `firefox`. Answering which names exist by
# walking the theme would open 335 000 entries across hundreds of directories
# on every keystroke; the merge already wrote them all down, once, in its
# manifest (`nix/scripts/merge-icons.py`). This is that list with the sizes,
# contexts and licences taken out: sorted and unique — 26 807 names in 535 KB,
# measured on 2026-09-26.
#
# A file and not a directory, and it references nothing: the names are text,
# so the 0,61 GiB theme is a BUILD input here and never part of the closure.
# The theme itself reaches the CLI through `RONOCTUA_ICONS`, and only on a
# machine that has it (`ronoctua.icons.superpack`).
{
  runCommand,
  python3,
  iconSuperpack,
}:

runCommand "ronoctua-icon-names" { nativeBuildInputs = [ python3 ]; } ''
  python3 - ${iconSuperpack}/share/doc/noctua-icons/manifest.json <<'END'
  import json, os, sys

  entries = json.load(open(sys.argv[1]))["entries"]
  names = sorted({e["name"] for e in entries})
  # A FLOOR, because a manifest whose shape moved gives back an empty list,
  # and an empty list is a search that finds nothing and says nothing.
  if len(names) < 10000:
      sys.exit(f"only {len(names)} icon names in the superpack's manifest")
  # A name that could split a line would split the file's grammar.
  bad = [n for n in names if not n or any(c in n for c in "\n\r\t")]
  if bad:
      sys.exit(f"{len(bad)} icon names carry a line break or a tab: {bad[:3]}")
  with open(os.environ["out"], "w") as out:
      out.writelines(n + "\n" for n in names)
  print(f"icon names: {len(names)}")
  END
''
