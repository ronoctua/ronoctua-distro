"""The lexicon's contract — shared by all THREE generators.

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

# THE SOURCE LANGUAGE. The names being tokenised are English — `nf-md-arrow_up`,
# `LEFTWARDS ARROW WITH HOOK` — so English is where the words come FROM, and
# every other language is a translation of them. It is a subtag, like the
# lexicon's own keys.
SOURCE = "en"


def no_accent(s: str) -> str:
    """`localização` -> `localizacao`.

    The accent is resolved at BUILD time, and not at runtime: normalising in the component
    would cost walking thousands of records times dozens of tags on every
    keystroke. Here it costs once.
    """
    return "".join(
        c for c in unicodedata.normalize("NFD", s) if not unicodedata.combining(c)
    )


def normalize_entry(v) -> dict[str, list[str]]:
    """One lexicon entry, in either accepted form, becomes `subtag -> words`.

        folder   = ["pasta", "diretorio"]
        location = { en = ["locale", "map"], pt = ["localização", "mapa"] }
        window   = { en = ["pane"], pt = ["janela"], de = ["fenster"] }

    The LIST form still holds, and not out of nostalgia: rewriting the four
    thousand existing entries would be that many more chances to get it wrong
    for no gain at all. A list = the Portuguese translation, which is what the
    file has always meant by it.

    The dict form is what makes a THIRD language cost an added key rather than
    a new file: the subtags here are the `pt` of `pt-BR`, and a language with
    no key simply has no translation for that token — which the caller turns
    into the untranslated word, exactly as it always did for a missing entry.
    """
    if isinstance(v, dict):
        return {k: list(w) for k, w in v.items()}
    return {"pt": list(v)}


def words_of(name: str) -> list[str]:
    """A name becomes its list of words, in lower case.

    It serves both vocabularies: `md-arrow_up_bold` (once the prefix is
    stripped) and `LEFTWARDS ARROW WITH HOOK`. The separator is "everything that is not
    a letter or a digit", so `_`, `-` and space all fall together.
    """
    return [p.lower() for p in re.split(r"[^A-Za-z0-9]+", name) if p]


def translate(tokens, lexicon, languages, no_translation=None):
    """Translates a list of words. Returns `(tags, translated)`.

    `tags` is `subtag -> set of words`, one entry per language asked for.
    `languages` are SUBTAGS (`en`, `pt`), not full tags — `subtag_of` turns
    `pt-BR` into `pt`.

    English is the SOURCE and not a translation: the tokens are the words of
    the Nerd Fonts and Unicode names, so `tags["en"]` always holds the token
    itself, plus whatever English synonyms the entry lists. Every other
    language gets its translation, or — when the lexicon has none — the token
    untranslated. The list never comes out empty; it comes out full of English
    pretending to be the other language, which is why the second half of the
    answer exists.

    `translated` says, per language, whether a translation actually came from
    the lexicon. It can NOT be derived from `tags`, for that same reason, and
    it is what lets a generator report honestly whether a record is findable in
    a given language.

    `no_translation` is an optional `Counter`, so the coverage report can say
    what to translate next.
    """
    tags: dict[str, set[str]] = {l: set() for l in languages}
    translated: dict[str, bool] = {l: False for l in languages}
    for p in tokens:
        if p in STOPWORDS:
            continue
        entry = normalize_entry(lexicon[p]) if p in lexicon else None
        if entry is None and no_translation is not None:
            no_translation[p] += 1
        for language in languages:
            words = entry.get(language, []) if entry else []
            if language == SOURCE:
                # THE TOKEN ITSELF, always — plus any English synonym. Without
                # the synonyms `location` could only be found by `location`:
                # there was no way to say that `locale` and `map` lead to the
                # same place.
                tags[language].add(p)
                tags[language].update(words)
                translated[language] = True
            elif words:
                tags[language].update(words)
                translated[language] = True
            else:
                # No translation: IT PASSES UNTRANSLATED, and it counts.
                tags[language].add(p)
    return tags, translated


def subtag_of(language: str) -> str:
    """`pt-BR` -> `pt`. The lexicon keys on the language, not on the region.

    A translation is of the LANGUAGE: `pt-BR` and `pt-PT` differ in a handful
    of words and share four thousand, and keying the lexicon on the full tag
    would ask for the whole file again to gain those few.
    """
    return language.split("-")[0].lower()


def search_forms(tags) -> set[str]:
    """The UNACCENTED forms of every tag — only the ones that DIFFER.

    They do not go to the screen: they exist so `localizacao` finds what is
    written `localização`. `mapa` does not get a twin identical to itself.

    Over ALL the languages and not just Portuguese: accents are not a
    Portuguese peculiarity, and a language added later would otherwise be the
    one language whose words had to be typed exactly right.
    """
    every = set()
    for words in tags.values():
        every.update(words)
    return {sa for t in every if (sa := no_accent(t)) != t}
