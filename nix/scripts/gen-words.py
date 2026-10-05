"""The offline English dictionary: the Open English WordNet (LMF XML) as two
sorted TSV files that `ronoctua_words::wordnet` reads by binary search.

    lemmas.tsv    #ronoctua-words-lemmas<TAB>1<TAB><count>
                  <word, lowercased><TAB><synset><US><synset>...
    synsets.tsv   #ronoctua-words-synsets<TAB>1<TAB><count>
                  <synset><TAB><pos><TAB><gloss><TAB><example><US>...<TAB><member><US>...

<US> is U+001F. Both files are sorted by their first field, byte by byte, after
the header; a synset is WordNet's id without `oewn-`. The build REFUSES:

  - a field with a tab, a line break or a unit separator in it after folding
    white space (it would split a row or a list);
  - a lemma naming a synset the file does not hold, or a synset member that
    is not an entry;
  - a dictionary smaller than WordNet is (a truncated or foreign XML);
  - an answer the reader's own tests rely on the shape of: `house` is a noun
    first, and `abode` shares a synset with `home`.

Nothing here is resident at run time: this is the whole reason for the shape.
"""

import argparse
import gzip
import pathlib
import sys
import xml.etree.ElementTree as ET

FORMAT = "1"
ITEMS = "\x1f"
PREFIX = "oewn-"


def refuse(message):
    print("ERROR: " + message, file=sys.stderr)
    raise SystemExit(1)


def clean(text, where):
    folded = " ".join((text or "").split())
    if any(c in folded for c in "\t\n\r" + ITEMS):
        refuse(where + " holds a character that would split a row")
    return folded


def short(synset):
    return synset[len(PREFIX):] if synset.startswith(PREFIX) else synset


def read(path):
    entries = {}  # entry id -> written form
    senses = {}  # folded lemma -> [synset ids], in WordNet's order
    synsets = {}  # synset id -> (pos, gloss, [examples], [member entry ids])
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rb") as stream:
        for _, node in ET.iterparse(stream, events=("end",)):
            tag = node.tag
            if tag == "LexicalEntry":
                lemma = node.find("Lemma")
                if lemma is None:
                    refuse("an entry without a lemma: " + node.get("id", "?"))
                form = clean(lemma.get("writtenForm"), "the lemma of " + node.get("id", "?"))
                entries[node.get("id")] = form
                key = form.lower()
                listed = senses.setdefault(key, [])
                for sense in node.findall("Sense"):
                    synset = short(sense.get("synset", ""))
                    if synset and synset not in listed:
                        listed.append(synset)
                node.clear()
            elif tag == "Synset":
                ident = short(node.get("id", ""))
                definition = node.find("Definition")
                gloss = clean(definition.text if definition is not None else "", "the gloss of " + ident)
                examples = [clean(e.text, "an example of " + ident) for e in node.findall("Example")]
                members = (node.get("members") or "").split()
                synsets[ident] = (node.get("partOfSpeech", ""), gloss, [e for e in examples if e], members)
                node.clear()
    return entries, senses, synsets


def write(path, header, rows):
    rows.sort(key=lambda r: r[0].encode("utf-8"))
    with open(path, "w", encoding="utf-8", newline="\n") as out:
        out.write(header + "\t" + FORMAT + "\t" + str(len(rows)) + "\n")
        for key, rest in rows:
            out.write(key + "\t" + rest + "\n")


def main():
    parser = argparse.ArgumentParser(description="Convert the Open English WordNet into sorted TSV files.")
    parser.add_argument("--wordnet", required=True, help="the directory holding one WN-LMF .xml.gz")
    parser.add_argument("--out", required=True, help="the directory to write lemmas.tsv and synsets.tsv into")
    args = parser.parse_args()

    found = sorted(pathlib.Path(args.wordnet).glob("*.xml.gz")) + sorted(pathlib.Path(args.wordnet).glob("*.xml"))
    if len(found) != 1:
        refuse("expected exactly one WordNet XML in " + args.wordnet + ", found " + str(len(found)))
    entries, senses, synsets = read(found[0])

    if len(synsets) < 100000 or len(senses) < 140000:
        refuse(f"only {len(synsets)} synsets and {len(senses)} lemmas: not the whole WordNet")

    for key, ids in senses.items():
        for ident in ids:
            if ident not in synsets:
                refuse(f"`{key}` names the synset {ident}, which is not in the file")

    lemma_rows = [(key, ITEMS.join(ids)) for key, ids in senses.items() if key and ids]
    synset_rows = []
    for ident, (pos, gloss, examples, members) in synsets.items():
        forms = []
        for member in members:
            if member not in entries:
                refuse(f"the synset {ident} has the member {member}, which is not an entry")
            if entries[member] not in forms:
                forms.append(entries[member])
        synset_rows.append((ident, "\t".join([pos, gloss, ITEMS.join(examples), ITEMS.join(forms)])))

    # THE SHAPE THE READER RELIES ON, asked of the real data.
    house = senses.get("house", [])
    if not house or synsets[house[0]][0] != "n":
        refuse("`house` is not a noun first")
    abode, home = set(senses.get("abode", [])), set(senses.get("home", []))
    if not abode & home:
        refuse("`abode` and `home` share no synset")

    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    write(out / "lemmas.tsv", "#ronoctua-words-lemmas", lemma_rows)
    write(out / "synsets.tsv", "#ronoctua-words-synsets", synset_rows)
    print(f"OK: {len(lemma_rows)} words, {len(synset_rows)} synsets")


if __name__ == "__main__":
    main()
