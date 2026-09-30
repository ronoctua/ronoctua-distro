# THE LANGUAGES A FILM IS SPOKEN AND SUBTITLED IN — the one place they are named.
#
# Not the languages this system SPEAKS (`assets/i18n/languages.nix`, two):
# these are the languages of CONTENT, the ones a person searches subtitles in
# and a film's streams are tagged with. `nix/tongues.nix` turns the list into
# the table `ronoctua_player::tongues` compiles, and the text keys both
# languages have to carry (`tongue-<code>`, lowercase).
#
# # One row, four spellings
#
#   code         the tag this system writes and matches: in a subtitle file's
#                name (`film.pt-BR.srt`), in a search the door sends, in a
#                key. The primary subtag, and a region or script only where
#                the two are different languages to a reader (Brazilian and
#                European Portuguese, simplified and traditional Chinese).
#   twoLetter    ISO 639-1.
#   threeLetter  ISO 639-2/B — what a Matroska stream and ffprobe say.
#   opensubtitles  the code OpenSubtitles' REST API takes in `languages=`;
#                `null` when it has none.
#   subdl        the code SubDL takes in `languages=`; `null` when it has none.
#
# The two service columns were written from each service's published list,
# not measured against it: neither list can be read without an account's key.
# A wrong code is a language whose search finds nothing, never a request
# somewhere else — the generator refuses the shape, not the spelling.
#
# # What the generator refuses
#
#   - a code, a two-letter or a three-letter code of the wrong shape;
#   - a two-letter code that is not the code's primary subtag;
#   - a code twice, or a service's code twice;
#   - a row neither service searches in (a language nobody can ask in);
#   - a list without English, the second of every default search.
#
# # The default search
#
# The interface's language, then English (`tongues::default_for`): a person
# reading in Portuguese searches `pt-BR` and `en` until they add another.
[
  {
    code = "en";
    twoLetter = "en";
    threeLetter = "eng";
    opensubtitles = "en";
    subdl = "EN";
  }
  {
    code = "pt-BR";
    twoLetter = "pt";
    threeLetter = "por";
    opensubtitles = "pt-br";
    subdl = "BR_PT";
  }
  {
    code = "pt-PT";
    twoLetter = "pt";
    threeLetter = "por";
    opensubtitles = "pt-pt";
    subdl = "PT";
  }
  {
    code = "es";
    twoLetter = "es";
    threeLetter = "spa";
    opensubtitles = "es";
    subdl = "ES";
  }
  {
    code = "fr";
    twoLetter = "fr";
    threeLetter = "fre";
    opensubtitles = "fr";
    subdl = "FR";
  }
  {
    code = "de";
    twoLetter = "de";
    threeLetter = "ger";
    opensubtitles = "de";
    subdl = "DE";
  }
  {
    code = "it";
    twoLetter = "it";
    threeLetter = "ita";
    opensubtitles = "it";
    subdl = "IT";
  }
  {
    code = "nl";
    twoLetter = "nl";
    threeLetter = "dut";
    opensubtitles = "nl";
    subdl = "NL";
  }
  {
    code = "sv";
    twoLetter = "sv";
    threeLetter = "swe";
    opensubtitles = "sv";
    subdl = "SV";
  }
  {
    code = "no";
    twoLetter = "no";
    threeLetter = "nor";
    opensubtitles = "no";
    subdl = "NO";
  }
  {
    code = "da";
    twoLetter = "da";
    threeLetter = "dan";
    opensubtitles = "da";
    subdl = "DA";
  }
  {
    code = "fi";
    twoLetter = "fi";
    threeLetter = "fin";
    opensubtitles = "fi";
    subdl = "FI";
  }
  {
    code = "is";
    twoLetter = "is";
    threeLetter = "ice";
    opensubtitles = "is";
    subdl = "IS";
  }
  {
    code = "pl";
    twoLetter = "pl";
    threeLetter = "pol";
    opensubtitles = "pl";
    subdl = "PL";
  }
  {
    code = "cs";
    twoLetter = "cs";
    threeLetter = "cze";
    opensubtitles = "cs";
    subdl = "CS";
  }
  {
    code = "sk";
    twoLetter = "sk";
    threeLetter = "slo";
    opensubtitles = "sk";
    subdl = "SK";
  }
  {
    code = "sl";
    twoLetter = "sl";
    threeLetter = "slv";
    opensubtitles = "sl";
    subdl = "SL";
  }
  {
    code = "hr";
    twoLetter = "hr";
    threeLetter = "hrv";
    opensubtitles = "hr";
    subdl = "HR";
  }
  {
    code = "sr";
    twoLetter = "sr";
    threeLetter = "srp";
    opensubtitles = "sr";
    subdl = "SR";
  }
  {
    code = "bs";
    twoLetter = "bs";
    threeLetter = "bos";
    opensubtitles = "bs";
    subdl = "BS";
  }
  {
    code = "mk";
    twoLetter = "mk";
    threeLetter = "mac";
    opensubtitles = "mk";
    subdl = "MK";
  }
  {
    code = "bg";
    twoLetter = "bg";
    threeLetter = "bul";
    opensubtitles = "bg";
    subdl = "BG";
  }
  {
    code = "ro";
    twoLetter = "ro";
    threeLetter = "rum";
    opensubtitles = "ro";
    subdl = "RO";
  }
  {
    code = "hu";
    twoLetter = "hu";
    threeLetter = "hun";
    opensubtitles = "hu";
    subdl = "HU";
  }
  {
    code = "el";
    twoLetter = "el";
    threeLetter = "gre";
    opensubtitles = "el";
    subdl = "EL";
  }
  {
    code = "tr";
    twoLetter = "tr";
    threeLetter = "tur";
    opensubtitles = "tr";
    subdl = "TR";
  }
  {
    code = "ru";
    twoLetter = "ru";
    threeLetter = "rus";
    opensubtitles = "ru";
    subdl = "RU";
  }
  {
    code = "uk";
    twoLetter = "uk";
    threeLetter = "ukr";
    opensubtitles = "uk";
    subdl = "UK";
  }
  {
    code = "be";
    twoLetter = "be";
    threeLetter = "bel";
    opensubtitles = "be";
    subdl = "BE";
  }
  {
    code = "lt";
    twoLetter = "lt";
    threeLetter = "lit";
    opensubtitles = "lt";
    subdl = "LT";
  }
  {
    code = "lv";
    twoLetter = "lv";
    threeLetter = "lav";
    opensubtitles = "lv";
    subdl = "LV";
  }
  {
    code = "et";
    twoLetter = "et";
    threeLetter = "est";
    opensubtitles = "et";
    subdl = "ET";
  }
  {
    code = "ka";
    twoLetter = "ka";
    threeLetter = "geo";
    opensubtitles = "ka";
    subdl = "KA";
  }
  {
    code = "hy";
    twoLetter = "hy";
    threeLetter = "arm";
    opensubtitles = "hy";
    subdl = null;
  }
  {
    code = "az";
    twoLetter = "az";
    threeLetter = "aze";
    opensubtitles = "az";
    subdl = "AZ";
  }
  {
    code = "sq";
    twoLetter = "sq";
    threeLetter = "alb";
    opensubtitles = "sq";
    subdl = "SQ";
  }
  {
    code = "ca";
    twoLetter = "ca";
    threeLetter = "cat";
    opensubtitles = "ca";
    subdl = "CA";
  }
  {
    code = "eu";
    twoLetter = "eu";
    threeLetter = "baq";
    opensubtitles = "eu";
    subdl = null;
  }
  {
    code = "gl";
    twoLetter = "gl";
    threeLetter = "glg";
    opensubtitles = "gl";
    subdl = null;
  }
  {
    code = "eo";
    twoLetter = "eo";
    threeLetter = "epo";
    opensubtitles = "eo";
    subdl = "EO";
  }
  {
    code = "ar";
    twoLetter = "ar";
    threeLetter = "ara";
    opensubtitles = "ar";
    subdl = "AR";
  }
  {
    code = "he";
    twoLetter = "he";
    threeLetter = "heb";
    opensubtitles = "he";
    subdl = "HE";
  }
  {
    code = "fa";
    twoLetter = "fa";
    threeLetter = "per";
    opensubtitles = "fa";
    subdl = "FA";
  }
  {
    code = "ku";
    twoLetter = "ku";
    threeLetter = "kur";
    opensubtitles = "ku";
    subdl = "KU";
  }
  {
    code = "ur";
    twoLetter = "ur";
    threeLetter = "urd";
    opensubtitles = "ur";
    subdl = "UR";
  }
  {
    code = "hi";
    twoLetter = "hi";
    threeLetter = "hin";
    opensubtitles = "hi";
    subdl = "HI";
  }
  {
    code = "bn";
    twoLetter = "bn";
    threeLetter = "ben";
    opensubtitles = "bn";
    subdl = "BN";
  }
  {
    code = "ta";
    twoLetter = "ta";
    threeLetter = "tam";
    opensubtitles = "ta";
    subdl = "TA";
  }
  {
    code = "te";
    twoLetter = "te";
    threeLetter = "tel";
    opensubtitles = "te";
    subdl = "TE";
  }
  {
    code = "ml";
    twoLetter = "ml";
    threeLetter = "mal";
    opensubtitles = "ml";
    subdl = "ML";
  }
  {
    code = "si";
    twoLetter = "si";
    threeLetter = "sin";
    opensubtitles = "si";
    subdl = "SI";
  }
  {
    code = "th";
    twoLetter = "th";
    threeLetter = "tha";
    opensubtitles = "th";
    subdl = "TH";
  }
  {
    code = "vi";
    twoLetter = "vi";
    threeLetter = "vie";
    opensubtitles = "vi";
    subdl = "VI";
  }
  {
    code = "id";
    twoLetter = "id";
    threeLetter = "ind";
    opensubtitles = "id";
    subdl = "ID";
  }
  {
    code = "ms";
    twoLetter = "ms";
    threeLetter = "may";
    opensubtitles = "ms";
    subdl = "MS";
  }
  {
    code = "tl";
    twoLetter = "tl";
    threeLetter = "tgl";
    opensubtitles = "tl";
    subdl = "TL";
  }
  {
    code = "my";
    twoLetter = "my";
    threeLetter = "bur";
    opensubtitles = "my";
    subdl = "MY";
  }
  {
    code = "km";
    twoLetter = "km";
    threeLetter = "khm";
    opensubtitles = "km";
    subdl = null;
  }
  {
    code = "zh-CN";
    twoLetter = "zh";
    threeLetter = "chi";
    opensubtitles = "zh-cn";
    subdl = "ZH";
  }
  {
    code = "zh-TW";
    twoLetter = "zh";
    threeLetter = "chi";
    opensubtitles = "zh-tw";
    subdl = "ZH_BG";
  }
  {
    code = "ja";
    twoLetter = "ja";
    threeLetter = "jpn";
    opensubtitles = "ja";
    subdl = "JA";
  }
  {
    code = "ko";
    twoLetter = "ko";
    threeLetter = "kor";
    opensubtitles = "ko";
    subdl = "KO";
  }
  {
    code = "sw";
    twoLetter = "sw";
    threeLetter = "swa";
    opensubtitles = "sw";
    subdl = null;
  }
  {
    code = "af";
    twoLetter = "af";
    threeLetter = "afr";
    opensubtitles = "af";
    subdl = null;
  }
]
