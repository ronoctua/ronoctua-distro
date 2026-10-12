# THE PERSON'S APPLIER REFUSES WHAT WOULD MAKE ROOT READ SOMETHING ELSE —
# and a refused tree leaves the machine's data exactly as it was.
#
# `person-apply-refuses-links` runs this with the EXTRACTED binary (the one
# the unit runs as root) as `$1`, in the build's own folder, as the build's
# user: the owner the staged tree must belong to is whoever runs it
# (`--owner $(id -u)`), which is how the unit names the person. Root's lock
# is a file of this folder (`--lock`), made `0600` by the binary. `$2` is
# `ronoctua-test-pq`, which makes the person's signing key and signs a
# staged tree as the belongings sign one; the machine PINS that key
# (`ronoctua-person pin`, as the sudo door's unit does) before the first
# apply, so what the good tree holds — options and brain module included —
# arrives only because the pinned key signed it.
#
# A good tree is applied first; then each way of lying is planted into a copy
# of it, and every one must be REFUSED, for its own reason (the last line on
# standard output, and `refused.why` in the verdict file), with the machine's
# `person/` byte for byte what it was and no `person.next` left behind:
#
#   - a symbolic link where the walk reads a file, and where it walks a
#     folder, and as the staged folder itself;
#   - a second name for a file (a hard link);
#   - a pipe where a file belongs;
#   - a file over its cap;
#   - a folder somebody else may write;
#   - a NEWER format: refused, and the old data stays — the update goes on;
#   - an empty staging folder, which is what every boot leaves;
#   - a nonce that is not one;
#   - a tree signed by another key than the pinned one, a tree with no
#     signature, and the person's own tree signed again with a counter
#     lower than the machine took (`person-apply-refuses-unsigned-tree`).
#
# And what must NOT refuse the tree: an icon no entry names (a link, one over
# its cap — never even opened); an editor's dropping in a brain module (left
# out, never opened); and a link inside ONE module, which leaves that module
# out with the machine keeping its copy, and lets the rest arrive; and an
# options file changed after the person signed, which is KEPT BACK (the
# machine keeps its own) while the rest arrives. On a machine with no key
# pinned, the declarations arrive and the options and the brain module do
# not. The verdict carries the staging's nonce. Root's lock, held by `hold`, is handed
# down to what it runs, and keeps out whoever was not handed it.
#
# It is a script of its own, and not inline, so the same text can be run
# against a binary `cargo` built — which is how it was proved, and sabotaged.
set -euo pipefail

person=$1
testpq=$2
me=$(id -u)
work=$PWD/person-apply-work
rm -rf "$work"
mkdir -p "$work"
cd "$work"

icon=e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855

# THE GOOD TREE, as the person's software stages it.
mkdir -p good/person/software/icons good/person/machines good/brain-modules/my-mod/i18n
cat > good/person/software/declared.json <<JSON
{"format": 1, "apps": [
  {"source": "nix", "id": "nix:ripgrep", "machines": null, "edited-at": "2026-10-05T12:00:00Z",
   "edited-on": "noctua", "attr": "ripgrep", "unfree": false},
  {"source": "web", "id": "web:notes", "machines": null, "edited-at": "2026-10-05T12:00:00Z",
   "edited-on": "noctua", "slug": "notes", "name": "Notes", "url": "https://notes.example.org/",
   "profile": "Default", "icon": "$icon", "category": "Office"}
]}
JSON
printf '\x89PNG\r\n\x1a\nthe icon' > "good/person/software/icons/$icon.png"
printf '{"format": 1, "ssh-signing": null}\n' > good/person/options.json
printf '{}\n' > good/person/machines/vm.json
printf '{ ... }: { }\n' > good/brain-modules/my-mod/module.nix
printf 'x = y\n' > good/brain-modules/my-mod/i18n/en-GB.ftl
printf '{"format": 1, "apps": []}\n' > outside.json
mkdir machine
chmod 0755 machine

# THE PERSON'S KEY, the good tree signed with it, and the machine's pin of it
# — from a file of the pinning user's, `0600`, consumed.
"$testpq" signing-key "$work/key.sk" > key.pub
"$testpq" sign-tree "$work/key.sk" good 1
(umask 077 && cp key.pub pin)
"$person" pin --pin "$work/pin" --machine "$work/machine" --lock "$work/lock" > out.txt 2> err.txt || {
  echo "ERROR: the key was not pinned:" >&2
  cat out.txt err.txt >&2
  exit 1
}
grep -q '^pinned ' out.txt || { echo "ERROR: pin said $(cat out.txt)" >&2; exit 1; }
[ ! -e pin ] || { echo "ERROR: the pin's file outlived it" >&2; exit 1; }
chmod -R go-w good

fresh() {
  rm -rf staged
  cp -a good staged
  chmod 0700 staged
}

run() {
  # The status, and the last line, whatever happened.
  set +e
  "$person" apply --staged "$1" --machine "$work/machine" --owner "$me" \
    --verdict "$work/verdict.json" --lock "$work/lock" > out.txt 2> err.txt
  status=$?
  set -e
  said=$(tail -n 1 out.txt)
}

fresh
run "$work/staged"
[ "$status" = 0 ] && [ "$said" = applied ] || {
  echo "ERROR: the good tree was not applied ($status, $said)" >&2
  cat err.txt >&2
  exit 1
}
for file in software/declared.json "software/icons/$icon.png" options.json brain/my-mod/module.nix; do
  [ -f "machine/person/$file" ] || { echo "ERROR: $file did not reach the machine" >&2; exit 1; }
done
[ ! -e machine/person/machines ] || { echo "ERROR: the machines' records were copied" >&2; exit 1; }
run "$work/staged"
[ "$said" = unchanged ] || { echo "ERROR: a second apply of the same tree said $said" >&2; exit 1; }
cp -a machine/person before

refused=0
refuse() {
  # $1: the reason it must give; $2: what was planted; $3: the staged folder.
  run "${3:-$work/staged}"
  if [ "$status" = 0 ]; then
    echo "ERROR: $2 was APPLIED" >&2
    exit 1
  fi
  [ "$said" = "refused $1" ] || {
    echo "ERROR: $2 was refused as '$said', not as 'refused $1'" >&2
    cat err.txt >&2
    exit 1
  }
  grep -q "\"why\": \"$1\"" verdict.json || {
    echo "ERROR: the verdict of $2 does not say $1:" >&2
    cat verdict.json >&2
    exit 1
  }
  diff -r before machine/person > /dev/null || {
    echo "ERROR: $2 changed the machine's data:" >&2
    diff -r before machine/person >&2 || true
    exit 1
  }
  [ ! -e machine/person.next ] || { echo "ERROR: $2 left person.next behind" >&2; exit 1; }
  echo "refused, as $1: $2"
  refused=$((refused + 1))
}

fresh
rm staged/person/software/declared.json
ln -s ../../../outside.json staged/person/software/declared.json
refuse link "a link where declared.json belongs"

fresh
mv staged/person/software staged/person/real
ln -s real staged/person/software
refuse link "a link where the software folder belongs"

fresh
ln -s staged staged-link
refuse link "the staged folder itself, as a link" "$work/staged-link"
rm staged-link

fresh
ln staged/person/options.json staged/spare
refuse hard-link "a second name for options.json"

fresh
rm staged/person/options.json
mkfifo staged/person/options.json
refuse special "a pipe where options.json belongs"

fresh
head -c 20000 /dev/zero | tr '\0' ' ' > staged/person/options.json
refuse too-large "an options.json over its cap"

fresh
chmod 0777 staged/person/software
refuse writable "a software folder anybody may write"

fresh
printf '{"format": 2, "apps": []}\n' > staged/person/software/declared.json
refuse newer "a declared.json of a newer format"

fresh
rm -rf staged/person staged/brain-modules
refuse nothing-staged "an empty staging folder"

fresh
printf 'not a nonce!\n' > staged/nonce
refuse not-a-nonce "a nonce that is not one"

fresh
"$testpq" signing-key "$work/stranger.sk" > /dev/null
"$testpq" sign-tree "$work/stranger.sk" staged 2
refuse bad-signature "a tree signed by another key than the pinned one"
rm "$work/stranger.sk"

fresh
rm staged/signed.sig
refuse not-signed "a tree with no signature"

fresh
"$testpq" sign-tree "$work/key.sk" staged 0
refuse rolled-back "the person's tree signed again with a counter lower than the machine took"

[ "$refused" = 13 ] || { echo "ERROR: $refused refusals, and there are 13" >&2; exit 1; }

# WHAT MUST NOT REFUSE THE TREE. Each leaves the machine's data as it was
# (the good tree is what it holds) and says `unchanged`.
kept() {
  # $1: what was planted.
  run "$work/staged"
  [ "$status" = 0 ] && [ "$said" = unchanged ] || {
    echo "ERROR: $1 was not taken in stride ($status, $said)" >&2
    cat err.txt >&2
    exit 1
  }
  diff -r before machine/person > /dev/null || {
    echo "ERROR: $1 changed the machine's data:" >&2
    diff -r before machine/person >&2 || true
    exit 1
  }
  echo "taken in stride: $1"
}

fresh
ln -s /etc/shadow "staged/person/software/icons/$(printf 'a%.0s' $(seq 64)).png"
head -c 300000 /dev/zero > "staged/person/software/icons/$(printf 'b%.0s' $(seq 64)).png"
kept "an icon no entry names, as a link and over its cap"

fresh
printf 'vim was here' > staged/brain-modules/my-mod/.module.nix.swp
ln -s /etc/shadow staged/brain-modules/my-mod/.#module.nix
kept "an editor's droppings inside a brain module"
grep -q '"brain-modules/my-mod/.module.nix.swp"' verdict.json || {
  echo "ERROR: the verdict does not say the dropping was not taken:" >&2
  cat verdict.json >&2
  exit 1
}

fresh
printf '{ changed }\n' > staged/brain-modules/my-mod/module.nix
ln -s /etc/passwd staged/brain-modules/my-mod/passwd
kept "a link inside a brain module"
grep -q '"module": "my-mod"' verdict.json && grep -q '"why": "link"' verdict.json \
  && grep -q '"kept": true' verdict.json || {
  echo "ERROR: the verdict does not say my-mod was left out, and kept:" >&2
  cat verdict.json >&2
  exit 1
}

fresh
printf '{"format": 1, "ssh-signing": {"key": ".ssh/id_ed25519.pub"}}\n' > staged/person/options.json
kept "the options changed after the person signed"
grep -q '"at": "person/options.json"' verdict.json && grep -q '"why": "not-as-signed"' verdict.json || {
  echo "ERROR: the verdict does not say the options were kept back:" >&2
  cat verdict.json >&2
  exit 1
}

# NO KEY PINNED: the declarations arrive; the options and the brain module
# wait for one.
mkdir bare
chmod 0755 bare
fresh
"$person" apply --staged "$work/staged" --machine "$work/bare" --owner "$me" \
  --verdict "$work/verdict.json" --lock "$work/lock" > out.txt 2> err.txt || {
  echo "ERROR: a machine with no key pinned refused the tree:" >&2
  cat out.txt err.txt >&2
  exit 1
}
[ -f bare/person/software/declared.json ] || { echo "ERROR: the declarations did not arrive with no key pinned" >&2; exit 1; }
[ ! -e bare/person/options.json ] || { echo "ERROR: the options arrived with no key pinned" >&2; exit 1; }
[ ! -e bare/person/brain ] || { echo "ERROR: a brain module arrived with no key pinned" >&2; exit 1; }
grep -q '"why": "no-key-pinned"' verdict.json || {
  echo "ERROR: the verdict does not say no key is pinned:" >&2
  cat verdict.json >&2
  exit 1
}

# THE NONCE: the verdict says which staging it answers.
fresh
printf 'stager-0003\n' > staged/nonce
run "$work/staged"
grep -q '"nonce": "stager-0003"' verdict.json || {
  echo "ERROR: the verdict does not carry the staging's nonce:" >&2
  cat verdict.json >&2
  exit 1
}

# ROOT'S LOCK: made for its owner's eyes only; handed down by `hold` to what
# it runs, which takes it at once; and kept from anybody it was not handed to
# — who waits (here, until `timeout` gives up on it).
[ "$(stat -c %a lock)" = 600 ] || { echo "ERROR: the lock is $(stat -c %a lock), not 600" >&2; exit 1; }
fresh
started=$(date +%s)
"$person" hold --lock "$work/lock" -- "$person" apply --staged "$work/staged" \
  --machine "$work/machine" --owner "$me" --lock "$work/lock" > out.txt 2> err.txt || {
  echo "ERROR: an apply run by hold did not take the lock it was handed:" >&2
  cat out.txt err.txt >&2
  exit 1
}
[ $(($(date +%s) - started)) -lt 10 ] || { echo "ERROR: the handed-down lock was waited for" >&2; exit 1; }
set +e
"$person" hold --lock "$work/lock" -- "$(command -v sh)" -c \
  "exec timeout 4 '$person' apply --staged '$work/staged' --machine '$work/machine' --owner '$me' --lock '$work/lock' < /dev/null" \
  > out.txt 2> err.txt
status=$?
set -e
[ "$status" = 124 ] || {
  echo "ERROR: an apply NOT handed the lock did not wait for it ($status):" >&2
  cat out.txt err.txt >&2
  exit 1
}

# AND THE GOOD TREE STILL APPLIES — nothing above wedged the machine.
fresh
run "$work/staged"
[ "$said" = unchanged ] || { echo "ERROR: after the refusals the good tree said $said" >&2; exit 1; }

echo "OK: a signed tree applied whole under the pinned key, thirteen lies refused for their reasons (another key, no signature, a rollback among them), the machine's data untouched by each; stale icons, droppings, a broken module and unsigned options taken in stride; nothing but declarations with no key pinned; the nonce said; the lock handed down and kept"
