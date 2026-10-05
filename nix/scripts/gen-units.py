#!/usr/bin/env python3
"""The calculator's unit table: `assets/units.nix` (as JSON) into Rust.

Writes `units.rs` (included by `ronoctua-calc`'s `units` module) and
`units.json` (the same table, normalised, for the checks).

REFUSES, naming what it refused:
  - a unit, a currency or a word list with no words in some declared language;
  - a word or a symbol that means two different things (in any two languages:
    the calculator understands every language's words at once);
  - a factor or an offset that is not exact (a decimal, or a product and
    quotient of decimals — never a float), or that does not fit in an i128;
  - a category or a dimension the engine does not know;
  - a reciprocal unit outside the fuel-economy category;
  - a currency code that is not three capital letters, is quoted twice, or is
    not quoted at all (a named currency the rates never price);
  - a `known` equality between units that do not exist or are not of one kind.

It does NOT check that the `known` equalities hold: that is the
`units-are-total` check's question (and the engine's test), so a factor typed
wrong fails there, by name, rather than here.
"""

import argparse
import json
import re
import sys
from fractions import Fraction

CATEGORIES = [
    "length", "mass", "volume", "area", "temperature", "time", "speed", "data",
    "energy", "power", "pressure", "angle", "frequency", "fuel-economy", "cooking",
]
DIMENSIONS = ["length", "mass", "time", "temperature", "data", "angle"]
RADICES = {"decimal": "Decimal", "hexadecimal": "Hexadecimal", "binary": "Binary",
           "octal": "Octal", "roman": "Roman"}
INSTANTS = {"now": "Now", "today": "Today"}
CONNECTIVES = {"into": "Into", "of": "Of"}
FUNCTIONS = ["sqrt", "cbrt", "abs", "floor", "ceil", "round", "exp", "ln", "log",
             "log2", "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh"]
CONSTANTS = ["pi", "e", "tau"]
I128 = 2**127 - 1
ID = re.compile(r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$")

errors = []


def refuse(msg):
    errors.append(msg)


# THE SAME TABLE AS `ronoctua_apps::fold::folded_char`, which is what the
# calculator folds a typed word with: one character for one character. A word
# whose fold leaves anything but ASCII is refused below, so the two folds can
# never disagree about a word of this table.
ACCENTS = {
    "a": "áàâãäå", "e": "éèêë", "i": "íìîï", "o": "óòôõö", "u": "úùûü",
    "c": "ç", "n": "ñ", "y": "ýÿ",
}
UNACCENT = {c: plain for plain, cs in ACCENTS.items() for c in cs}


def fold(word):
    """Lowercase without accents: what a person types is compared in this form."""
    return "".join(UNACCENT.get(c, c) for c in word.lower()).strip()


def exact(text, what):
    """A decimal, or a product and quotient of decimals, as an exact Fraction."""
    if not isinstance(text, str) or not re.fullmatch(r"[0-9.]+([*/][0-9.]+)*", text):
        refuse(f"{what}: `{text}` is not exact — write a decimal, or decimals joined by * and /")
        return Fraction(1)
    parts = re.split(r"([*/])", text)
    value = Fraction(parts[0])
    for op, operand in zip(parts[1::2], parts[2::2]):
        f = Fraction(operand)
        if op == "*":
            value *= f
        elif f == 0:
            refuse(f"{what}: `{text}` divides by zero")
            return Fraction(1)
        else:
            value /= f
    if value.numerator > I128 or value.denominator > I128:
        refuse(f"{what}: `{text}` does not fit in an i128 fraction")
    return value


def signed(text, what):
    """An exact value that may be negative (`-40`)."""
    if isinstance(text, str) and text.startswith("-"):
        return -exact(text[1:], what)
    return exact(text, what)


def rust_str(s):
    return json.dumps(s, ensure_ascii=False)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--units", required=True)
    ap.add_argument("--languages", required=True)
    ap.add_argument("--out-rust", required=True)
    ap.add_argument("--out-json", required=True)
    a = ap.parse_args()

    table = json.load(open(a.units))
    subtags = [lang.split("-")[0].lower() for lang in json.load(open(a.languages))]

    # What each word and symbol means: a symbol kept as written, a word folded.
    seen = {}

    def claim(scope, word, meaning, where):
        key = (scope, word)
        if key in seen and seen[key] != meaning:
            refuse(f"{where}: `{word}` ({scope}) already means {seen[key]}")
        seen[key] = meaning

    def words_of(entry, where, meaning, need_every=True):
        """The (subtag, folded word) list of an entry's `words`, claimed."""
        words = entry.get("words", entry) if isinstance(entry, dict) else {}
        out = []
        for subtag, ws in words.items():
            if subtag != "*" and subtag not in subtags:
                refuse(f"{where}: words for `{subtag}`, which is not a declared language")
                continue
            for w in ws:
                f = fold(w)
                if not f:
                    refuse(f"{where}: an empty word in `{subtag}`")
                    continue
                if any(ord(c) > 127 for c in f) and not (subtag == "*" and len(f) == 1):
                    refuse(f"{where}: `{w}` folds to `{f}`, which is not plain ASCII")
                    continue
                if (subtag, f) not in out:
                    out.append((subtag, f))
        for subtag in subtags:
            if need_every and not any(s in (subtag, "*") for s, _ in out):
                refuse(f"{where}: no words in `{subtag}`")
        # ONE MEANING PER WORD ACROSS LANGUAGES: `parse` understands every
        # language's words at once (a person reading English still types
        # `metros`), so a word may not mean two things in two languages either.
        for subtag, f in out:
            claim("word", f, meaning, where)
        return out

    def symbols_of(entry, where, meaning):
        out = []
        for s in entry.get("symbols", []):
            if not s or s != s.strip():
                refuse(f"{where}: the symbol `{s}` is empty or padded")
                continue
            claim("*symbol", s, meaning, where)
            if s not in out:
                out.append(s)
        return out

    units = []
    ids = set()
    for n, u in enumerate(table.get("units", [])):
        uid = u.get("id", f"#{n}")
        where = f"unit `{uid}`"
        if not ID.match(uid):
            refuse(f"{where}: the id is not kebab-case")
        if uid in ids:
            refuse(f"{where}: declared twice")
        ids.add(uid)
        if u.get("category") not in CATEGORIES:
            refuse(f"{where}: unknown category `{u.get('category')}`")
        dim = u.get("dimension", {})
        for k in dim:
            if k not in DIMENSIONS:
                refuse(f"{where}: unknown dimension `{k}`")
        dimension = [int(dim.get(k, 0)) for k in DIMENSIONS]
        factor = exact(u.get("factor"), f"{where} factor")
        if factor <= 0:
            refuse(f"{where}: the factor must be positive")
        offset = exact(u.get("offset", "0"), f"{where} offset")
        pi = int(u.get("pi", 0))
        reciprocal = bool(u.get("reciprocal", False))
        if reciprocal and u.get("category") != "fuel-economy":
            refuse(f"{where}: only fuel economy converts by the reciprocal")
        if offset and (reciprocal or pi):
            refuse(f"{where}: an affine unit can be neither reciprocal nor carry π")
        meaning = f"the unit `{uid}`"
        symbols = symbols_of(u, where, meaning)
        words = words_of(u, where, meaning, need_every=False)
        # A unit needs words in every language of its OWN, not only `*`: the
        # singular and the plural a person reads are the first two.
        for subtag in subtags:
            if not u.get("words", {}).get(subtag):
                refuse(f"{where}: no words in `{subtag}`")
        units.append({
            "id": uid, "category": u.get("category"), "dimension": dimension,
            "factor": [factor.numerator, factor.denominator],
            "offset": [offset.numerator, offset.denominator],
            "pi": pi, "reciprocal": reciprocal, "symbols": symbols,
            "words": {s: [fold(w) for w in u.get("words", {}).get(s, [])] for s in subtags},
        })

    for cat in CATEGORIES:
        if sum(1 for u in units if u["category"] == cat) < 2:
            refuse(f"the category `{cat}` has fewer than two units: nothing to convert")

    currencies = []
    for c in table.get("currencies", []):
        code = c.get("code", "")
        where = f"currency `{code}`"
        if not re.fullmatch(r"[A-Z]{3}", code):
            refuse(f"{where}: not an ISO 4217 code")
        meaning = f"the currency `{code}`"
        currencies.append({"code": code, "symbols": symbols_of(c, where, meaning),
                           "words": words_of(c, where, meaning)})

    # THE CODES A PERSON MAY TYPE, in any case: a lowered code is a word of
    # the table like any other, so it may not mean something else.
    codes = []
    for n, code in enumerate(table.get("quoted", [])):
        where = f"quoted code #{n} `{code}`"
        if not isinstance(code, str) or not re.fullmatch(r"[A-Z]{3}", code):
            refuse(f"{where}: not an ISO 4217 code")
            continue
        if code in codes:
            refuse(f"{where}: quoted twice")
            continue
        claim("word", code.lower(), f"the currency `{code}`", where)
        codes.append(code)
    for c in currencies:
        if c["code"] not in codes:
            refuse(f"currency `{c['code']}`: not in `quoted`, so no rate would price it")
    codes.sort()

    # THE DEFINITIONS the factors are tested against.
    by_id = {u["id"]: u for u in units}
    known = []
    for n, k in enumerate(table.get("known", [])):
        where = f"known #{n} ({k.get('value')} {k.get('from')} = {k.get('is')} {k.get('to')})"
        source, target = by_id.get(k.get("from")), by_id.get(k.get("to"))
        if source is None or target is None:
            refuse(f"{where}: no such unit")
            continue
        fuel = [u["category"] == "fuel-economy" for u in (source, target)]
        if source["dimension"] != target["dimension"] or fuel[0] != fuel[1]:
            refuse(f"{where}: the two units are not of one kind")
            continue
        value, is_ = signed(k.get("value"), where), signed(k.get("is"), where)
        known.append({"value": [value.numerator, value.denominator], "from": source["id"], "to": target["id"],
                      "is": [is_.numerator, is_.denominator], "pi": int(k.get("pi", 0))})

    keywords = []

    def section(name, known, variant_of):
        entries = table.get(name, {})
        for k in entries:
            if k not in known:
                refuse(f"{name}: unknown entry `{k}`")
        for k in known:
            if k not in entries:
                refuse(f"{name}: `{k}` is missing")
                continue
            meaning = f"{name} `{k}`"
            for subtag, w in words_of(entries[k], f"{name} `{k}`", meaning):
                keywords.append((subtag, w, variant_of(k)))

    section("connectives", CONNECTIVES, lambda k: f"Means::{CONNECTIVES[k]}")
    section("radices", RADICES, lambda k: f"Means::Radix(Radix::{RADICES[k]})")
    section("instants", INSTANTS, lambda k: f"Means::{INSTANTS[k]}")
    section("functions", FUNCTIONS,
            lambda k: f"Means::Function(Function::{k[0].upper() + k[1:]})")
    section("constants", CONSTANTS,
            lambda k: f"Means::Constant(Constant::{k[0].upper() + k[1:]})")

    if errors:
        for e in errors:
            print(f"ERROR: assets/units.nix: {e}", file=sys.stderr)
        sys.exit(1)

    def cat_variant(c):
        return "".join(p.capitalize() for p in c.split("-"))

    lines = [
        "// Generated from `assets/units.nix` by `nix/scripts/gen-units.py`. Do not edit.",
        "",
        f"/// Every unit, in the declared order; a [`UnitId`] indexes this.",
        f"pub static UNITS: [Unit; {len(units)}] = [",
    ]
    for u in units:
        d = ", ".join(str(x) for x in u["dimension"])
        lines.append(
            f"    Unit {{ id: {rust_str(u['id'])}, symbol: {rust_str(u['symbols'][0] if u['symbols'] else '')}, "
            f"category: Category::{cat_variant(u['category'])}, dimension: Dimension([{d}]), "
            f"factor: ({u['factor'][0]}, {u['factor'][1]}), offset: ({u['offset'][0]}, {u['offset'][1]}), "
            f"pi: {u['pi']}, reciprocal: {'true' if u['reciprocal'] else 'false'} }},"
        )
    lines.append("];")

    aliases = []
    for i, u in enumerate(units):
        for s in u["symbols"]:
            aliases.append(("*", s, i))
        for subtag, ws in u["words"].items():
            for w in ws:
                aliases.append((subtag, w, i))
    aliases = sorted(set(aliases), key=lambda t: (t[1], t[0], t[2]))
    lines.append("")
    lines.append("/// Every word and symbol of a unit, sorted by word: a symbol under `*`, as")
    lines.append("/// written; a word under its language, folded.")
    lines.append(f"pub static ALIASES: [Alias; {len(aliases)}] = [")
    for subtag, w, i in aliases:
        lines.append(f"    Alias {{ subtag: {rust_str(subtag)}, word: {rust_str(w)}, unit: UnitId({i}) }},")
    lines.append("];")

    lines.append("")
    lines.append("/// The currencies' words and signs: `(subtag, word, ISO code)`, a sign under `*`.")
    cw = []
    for c in currencies:
        for s in c["symbols"]:
            cw.append(("*", s, c["code"]))
        for subtag, w in c["words"]:
            cw.append((subtag, w, c["code"]))
    lines.append(f"pub static CURRENCY_WORDS: [(&str, &str, &str); {len(cw)}] = [")
    for subtag, w, code in cw:
        lines.append(f"    ({rust_str(subtag)}, {rust_str(w)}, {rust_str(code)}),")
    lines.append("];")

    lines.append("")
    lines.append("/// What the units are by definition: `value from` is `is to` (times π^pi).")
    lines.append(f"pub static KNOWN: [Known; {len(known)}] = [")
    for k in known:
        lines.append(
            f"    Known {{ value: ({k['value'][0]}, {k['value'][1]}), from: {rust_str(k['from'])}, "
            f"to: {rust_str(k['to'])}, is: ({k['is'][0]}, {k['is'][1]}), pi: {k['pi']} }},"
        )
    lines.append("];")

    lines.append("")
    lines.append("/// The ISO codes a person may type (in any case), sorted.")
    lines.append(f"pub static CURRENCY_CODES: [&str; {len(codes)}] = [{', '.join(rust_str(c) for c in codes)}];")

    lines.append("")
    lines.append("/// The words that are not units: `(subtag, folded word, meaning)`.")
    lines.append(f"pub static KEYWORDS: [(&str, &str, Means); {len(keywords)}] = [")
    for subtag, w, m in keywords:
        lines.append(f"    ({rust_str(subtag)}, {rust_str(w)}, {m}),")
    lines.append("];")
    lines.append("")
    lines.append(f"/// The language subtags the words were written for.")
    lines.append(f"pub static SUBTAGS: [&str; {len(subtags)}] = [{', '.join(rust_str(s) for s in subtags)}];")

    open(a.out_rust, "w").write("\n".join(lines) + "\n")
    json.dump({"subtags": subtags, "units": units, "currencies": currencies, "codes": codes,
               "known": known},
              open(a.out_json, "w"), ensure_ascii=False, indent=1)
    print(f"units: {len(units)} units, {len(aliases)} aliases, {len(currencies)} currencies, "
          f"{len(keywords)} keywords")


if __name__ == "__main__":
    main()
