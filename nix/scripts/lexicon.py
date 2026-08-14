"""The bilingual lexicon's contract — shared by BOTH generators.

# Why this is a module, and not repeated code

Because the format of `assets/lexicon/tags.en-pt.toml` is ONE fact, and it now
has two readers: the glyph database generator (Nerd Fonts) and the symbol
database one (Unicode). Two copies of the same reading diverge the first time
the format gains a new shape — and the divergence would show up as "search in
Portuguese works in one buffer and not in the other", which nobody connects to the
file's format.

The lexicon is the only translation source for both: the words of the Nerd
Fonts names (`arrow`, `folder`) and those of the Unicode names (`ARROW`, `LEFTWARDS`) fall into the
same vocabulary, so translating once serves both.
"""

from __future__ import annotations

import re
import unicodedata

# Tokens the tokenisation produces but that do not help the search.
STOPWORDS = {"o", "alt", "variant"}


def no_accent(s: str) -> str:
    """`localização` -> `localizacao`.

    The accent is resolved at BUILD time, and not at runtime: normalising in the component
    would cost walking thousands of records times dozens of tags on every
    keystroke. Here it costs once.
    """
    return "".join(
        c for c in unicodedata.normalize("NFD", s) if not unicodedata.combining(c)
    )


def normalize_entry(v) -> tuple[list[str], list[str]]:
    """One lexicon entry, in either accepted form, becomes `(en, pt)`.

        folder   = ["pasta", "diretorio"]
        location = { en = ["locale", "map"], pt = ["localização", "mapa"] }

    The LIST form still holds, and not out of nostalgia: rewriting the
    existing entries would be that many more chances to get it wrong for no gain
    at all. A list = translation only, which is what the file has always done.
    """
    if isinstance(v, dict):
        return list(v.get("en", [])), list(v.get("pt", []))
    return [], list(v)


def palavras(name: str) -> list[str]:
    """A name becomes its list of words, in lower case.

    It serves both vocabularies: `md-arrow_up_bold` (once the prefix is
    stripped) and `LEFTWARDS ARROW WITH HOOK`. The separator is "everything that is not
    a letter or a digit", so `_`, `-` and space all fall together.
    """
    return [p.lower() for p in re.split(r"[^A-Za-z0-9]+", name) if p]


def translate(tokens, lexicon, no_translation=None) -> tuple[set[str], set[str], bool]:
    """Translates a list of words. Returns `(tags_en, tags_pt, tem_pt)`.

    `tem_pt` is the honest answer to "can this record be found in Portuguese?", and
    it can NOT be derived from `tags_pt`: an untranslated word goes in there
    untranslated — the list never comes out empty, it comes out full of English pretending to be
    Portuguese. Only by marking when a translation actually came from the lexicon.

    `sem_traducao` is an optional `Counter`, so the coverage report can
    say what to translate next.
    """
    tags_en: set[str] = set()
    tags_pt: set[str] = set()
    has_pt = False
    for p in tokens:
        if p in STOPWORDS:
            continue
        tags_en.add(p)
        entry = lexicon.get(p)
        if entry is not None:
            en, pt = normalize_entry(entry)
            # AN ENGLISH SYNONYM. Without it, `location` could only be found by
            # `location`: there was no way to say that `locale` and `map` lead to the
            # same place.
            tags_en.update(en)
            tags_pt.update(pt)
            has_pt = has_pt or bool(pt)
        else:
            # No translation: IT PASSES UNTRANSLATED, and it counts.
            tags_pt.add(p)
            if no_translation is not None:
                no_translation[p] += 1
    return tags_en, tags_pt, has_pt


def search_forms(tags_pt) -> set[str]:
    """The UNACCENTED forms of the pt tags — only the ones that DIFFER.

    They do not go to the screen: they exist so `localizacao` finds what is
    written `localização`. `mapa` does not get a twin identical to itself.
    """
    return {sa for t in tags_pt if (sa := no_accent(t)) != t}
