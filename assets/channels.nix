# The player's "channels by theme", and the numbers of its radio and TV lists.
#
# ONE DECLARATION, read by `nix/broadcast.nix` and turned into the table
# `ronoctua_broadcast::table` compiles (`CATEGORIES` and the constants below).
# The generator refuses the build when:
#
#   - an `iptv` id is not a category iptv-org has (the list is a pinned
#     capture of theirs: a typo would be a category that never finds a
#     channel, and nothing on screen could tell);
#   - an `iptv` id is one of `excluded`;
#   - a category has no source at all — no desk source, and no default
#     source of the catalogue naming it;
#   - an id repeats, or is not kebab-case (it becomes the text key
#     `player-channels-<id>`, which both languages must carry; a default
#     source's name is `player-channels-source-<id>`);
#   - two themes claim one alias, a default source names a theme that does
#     not exist, a host that is not a host's name, or a PeerTube category
#     PeerTube does not document.
#
# THREE SOURCES PER CATEGORY, each optional:
#
#   iptv      iptv-org category ids: live TV channels from the public index.
#   archive   Internet Archive collections, asked with `mediatype:(movies)`
#             (a collection answers its sub-collections first without it).
#             `subjects` narrows them — there is no "action" collection, but
#             there are feature films whose subject says action.
#   lives     words for a YouTube search with the Live filter: broadcasts that
#             are live now, "24/7" channels among them. The owner kept them
#             for films too, with the source said on screen.
#
# AND ONE FOR THE PERSON'S OWN SOURCES: `aliases`, the words (in any of the
# languages people name their lists in) that file a group of somebody's IPTV
# list, a podcast's category or a YouTube playlist under the theme. Matched as
# whole words, accents and case folded; the longest alias found wins ("science
# fiction" over "science"), and on a tie the theme listed first.
#
# Every collection named here was checked to exist, and to answer
# `mediatype: collection`, on 2026-09-28: feature_films, animationandcartoons,
# classic_cartoons, Film_Noir, SciFi_Horror, silent_films,
# newsandpublicaffairs. A new one is checked the same way before
# it is added — the generator cannot ask the archive.
{
  # HOW LONG AN ANSWER IS SHOWN WITHOUT ASKING AGAIN, in minutes.
  #
  # Radio stations come and go by the hour; iptv-org's index is rebuilt once
  # a day, and so is what the archive ranks by downloads. A live is live now
  # or not at all, so its list ages fastest.
  fresh = {
    radio = 360;
    iptv = 1440;
    archive = 1440;
    lives = 30;
    # THE CATALOGUE'S OTHER DEFAULT SOURCES. NASA's library and Commons'
    # categories grow by a few files a week; PeerTube's index by the hour, so
    # it ages by the day.
    nasa = 10080;
    commons = 10080;
    peertube = 1440;
    # A PERSON'S OWN SOURCES, by kind. A podcast publishes by the day and is
    # asked four times as often; an IPTV provider's list, a YouTube channel and
    # an archive.org collection change by the day.
    playlist = 1440;
    feed = 360;
    youtube = 1440;
    collection = 1440;
  };

  # How many stations one list asks radio-browser for (a country, a genre, a
  # language, a search). Ordered by how often they are played, so the cut
  # takes the least played.
  stationsMost = 500;

  # How many rows one page of the archive's search holds; the page ends in a
  # "More" row.
  archiveRows = 30;

  # How many lives one category asks for.
  livesMost = 20;

  # The recently played list, kept by the daemon, newest first.
  recentMost = 40;

  # The favourites a person can keep. A ceiling and not a preference: the
  # file is read whole by every door.
  favouritesMost = 500;

  # iptv-org categories that NEVER reach the index, whatever a channel's other
  # categories are. `xxx` is adult programming; `is_nsfw` channels are dropped
  # as well, by the flag.
  excluded = [ "xxx" ];

  # THE LOCAL CATALOGUE (`ronoctua_broadcast::catalog`): what one ingest may
  # read and keep. Bounds, not preferences — the disk's own ceiling is
  # `playerCatalogMib` in `assets/cache.nix`.
  catalogue = {
    # The most items one source keeps. An IPTV provider's list with its films
    # and series is tens of thousands of lines; past this the source says it
    # was cut.
    sourceItemsMost = 100000;
    # The most of a person's playlist that is read, in MiB (a provider's list
    # with every film is 30 MB), and of a feed (a podcast of a thousand
    # episodes is a few).
    listMib = 64;
    feedMib = 16;
    # The most items one category of a default source asks for, and how many
    # a single request asks (the four services page by a hundred or fewer).
    nativeItemsMost = 1000;
    pageRows = 100;
    # How many of a YouTube channel's playlists become categories, and how
    # many videos one of them lists.
    youtubePlaylistsMost = 50;
    youtubeListMost = 500;
  };

  # WORDS THAT SEND A GROUP TO THE ADULT STORE (`catalog-adult.sqlite`), which
  # a door opens only after the person's password.
  #
  # Matched as WHOLE WORDS of a group's name, accents and case folded (`Adultos`
  # is `adultos`; `Essex` is not `sex`). A source the person marks adult goes
  # there whole. Nothing here decides what a default source carries: those
  # are chosen free of adult programming (iptv-org's `xxx` and `is_nsfw` are
  # dropped above, PeerTube is asked `nsfw=false`).
  adultWords = [
    "xxx"
    "18+"
    "+18"
    "adult"
    "adults"
    "adulto"
    "adultos"
    "adulte"
    "adultes"
    "erwachsene"
    "porn"
    "porno"
    "pornô"
    "erotic"
    "erotica"
    "erótico"
    "erótica"
    "sex"
    "sexy"
    "nsfw"
    "hentai"
    "playboy"
    "hustler"
    "brazzers"
  ];

  categories = [
    {
      id = "action";
      iptv = [ ];
      archive = [ "feature_films" ];
      subjects = [ "action" ];
      lives = [ "action movies 24/7" ];
      aliases = [
        "action"
        "ação"
        "acción"
        "azione"
      ];
    }
    {
      # NO LIVE TV: iptv-org has no category for anime, and its `animation`
      # is every cartoon — which is the Cartoons theme's.
      id = "anime";
      iptv = [ ];
      archive = [ "animationandcartoons" ];
      subjects = [ "anime" ];
      lives = [ "anime 24/7" ];
      aliases = [
        "anime"
        "animes"
      ];
    }
    {
      id = "films";
      iptv = [ "movies" ];
      archive = [ "feature_films" ];
      subjects = [ ];
      lives = [ "movies 24/7" ];
      aliases = [
        "movies"
        "movie"
        "films"
        "film"
        "filmes"
        "filme"
        "cinema"
        "cine"
        "películas"
        "película"
        "vod"
        "kino"
      ];
    }
    {
      id = "series";
      iptv = [ "series" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "tv series 24/7" ];
      aliases = [
        "series"
        "séries"
        "série"
        "serie"
        "tv shows"
        "seriados"
        "novelas"
        "telenovelas"
      ];
    }
    {
      id = "cartoons";
      iptv = [ "animation" ];
      archive = [
        "classic_cartoons"
        "animationandcartoons"
      ];
      subjects = [ ];
      lives = [ "cartoons 24/7" ];
      aliases = [
        "cartoons"
        "cartoon"
        "desenhos"
        "desenho animado"
        "dibujos animados"
        "animation"
        "animação"
        "animación"
        "dessins animés"
        "zeichentrick"
      ];
    }
    {
      id = "documentary";
      iptv = [ "documentary" ];
      archive = [ "feature_films" ];
      subjects = [ "documentary" ];
      lives = [ "documentary 24/7" ];
      aliases = [
        "documentary"
        "documentaries"
        "documentário"
        "documentários"
        "documental"
        "documentales"
        "documentaire"
        "documentaires"
        "dokumentation"
        "docs"
      ];
    }
    {
      id = "news";
      iptv = [ "news" ];
      archive = [ "newsandpublicaffairs" ];
      subjects = [ ];
      lives = [ "news live" ];
      aliases = [
        "news"
        "notícias"
        "noticias"
        "nachrichten"
        "actualités"
        "telejornal"
        "noticiero"
      ];
    }
    {
      id = "music";
      iptv = [ "music" ];
      archive = [ ];
      subjects = [ ];
      lives = [
        "music 24/7"
        "lofi radio"
      ];
      aliases = [
        "music"
        "música"
        "musique"
        "musik"
        "clipes"
        "videoclipes"
        "concerts"
      ];
    }
    {
      id = "sports";
      iptv = [ "sports" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "sports 24/7" ];
      aliases = [
        "sports"
        "sport"
        "esportes"
        "esporte"
        "deportes"
        "deporte"
        "futebol"
        "fútbol"
        "football"
        "soccer"
      ];
    }
    {
      id = "kids";
      iptv = [
        "kids"
        "family"
      ];
      archive = [ "classic_cartoons" ];
      subjects = [ ];
      lives = [ "kids 24/7" ];
      aliases = [
        "kids"
        "children"
        "infantil"
        "infantis"
        "niños"
        "enfants"
        "kinder"
        "family"
        "família"
        "familia"
      ];
    }
    {
      id = "science";
      iptv = [
        "science"
        "education"
      ];
      archive = [ ];
      subjects = [ ];
      lives = [
        "science 24/7"
        "space live"
      ];
      aliases = [
        "science"
        "ciência"
        "ciencia"
        "sciences"
        "wissenschaft"
        "technology"
        "tecnologia"
        "tecnología"
        "tech"
      ];
    }
    {
      id = "classic";
      iptv = [ "classic" ];
      archive = [
        "Film_Noir"
        "silent_films"
      ];
      subjects = [ ];
      lives = [ "classic movies 24/7" ];
      aliases = [
        "classic"
        "classics"
        "clássicos"
        "clásicos"
        "classiques"
        "retro"
        "vintage"
      ];
    }
    {
      id = "relax";
      iptv = [ "relax" ];
      archive = [ ];
      subjects = [ ];
      lives = [
        "relaxing 24/7"
        "nature live camera"
      ];
      aliases = [
        "relax"
        "relaxing"
        "relaxar"
        "relajación"
        "ambient"
        "chill"
        "meditation"
        "meditação"
        "slow tv"
      ];
    }
    {
      id = "comedy";
      iptv = [ "comedy" ];
      archive = [ "feature_films" ];
      subjects = [ "comedy" ];
      lives = [ "comedy 24/7" ];
      aliases = [
        "comedy"
        "comédia"
        "comedia"
        "humor"
        "comédie"
        "komödie"
      ];
    }
    {
      id = "scifi-horror";
      iptv = [ ];
      archive = [ "SciFi_Horror" ];
      subjects = [ ];
      lives = [ "horror movies 24/7" ];
      aliases = [
        "sci-fi"
        "scifi"
        "science fiction"
        "ficção científica"
        "ciencia ficción"
        "horror"
        "terror"
        "suspense"
        "thriller"
      ];
    }
    {
      id = "travel";
      iptv = [ "travel" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "travel 24/7" ];
      aliases = [
        "travel"
        "viagem"
        "viagens"
        "viajes"
        "turismo"
        "voyage"
        "reisen"
      ];
    }
    # EVERY OTHER CATEGORY iptv-org has, each a theme of its own — so no
    # channel of the index is reachable only by a search.
    {
      id = "general";
      iptv = [ "general" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "general"
        "geral"
        "generalista"
        "variety"
        "variedades"
      ];
    }
    {
      id = "entertainment";
      iptv = [ "entertainment" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "entertainment"
        "entretenimento"
        "entretenimiento"
        "divertissement"
        "unterhaltung"
        "reality"
      ];
    }
    {
      id = "culture";
      iptv = [ "culture" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "culture"
        "cultura"
        "kultur"
        "cultural"
      ];
    }
    {
      id = "lifestyle";
      iptv = [ "lifestyle" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "lifestyle"
        "estilo de vida"
        "moda"
        "fashion"
        "people"
      ];
    }
    {
      id = "cooking";
      iptv = [ "cooking" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "cooking live" ];
      aliases = [
        "cooking"
        "culinária"
        "cocina"
        "cuisine"
        "food"
        "comida"
        "gastronomia"
        "gastronomía"
        "receitas"
        "recetas"
      ];
    }
    {
      id = "outdoor";
      iptv = [ "outdoor" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "outdoor"
        "outdoors"
        "hunting"
        "fishing"
        "pesca"
        "caça"
        "caza"
      ];
    }
    {
      id = "auto";
      iptv = [ "auto" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "auto"
        "cars"
        "carros"
        "coches"
        "motor"
        "motors"
        "vehicles"
        "veículos"
        "vehículos"
      ];
    }
    {
      id = "business";
      iptv = [ "business" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "business news live" ];
      aliases = [
        "business"
        "negócios"
        "negocios"
        "economia"
        "economía"
        "finance"
        "finanças"
        "finanzas"
      ];
    }
    {
      id = "weather";
      iptv = [ "weather" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "weather live" ];
      aliases = [
        "weather"
        "clima"
        "météo"
        "meteo"
        "wetter"
      ];
    }
    {
      id = "public";
      iptv = [ "public" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "public broadcasting"
        "tv pública"
        "televisión pública"
        "emissoras públicas"
      ];
    }
    {
      id = "legislative";
      iptv = [ "legislative" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "legislative"
        "legislativo"
        "parliament"
        "parlamento"
        "congress"
        "congresso"
        "senado"
        "senate"
      ];
    }
    {
      id = "religious";
      iptv = [ "religious" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "religious"
        "religion"
        "religião"
        "religión"
        "religiosos"
        "gospel"
        "catholic"
        "católica"
        "evangélica"
        "christian"
        "igreja"
        "church"
        "islamic"
      ];
    }
    {
      id = "shop";
      iptv = [ "shop" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "shop"
        "shopping"
        "compras"
        "teleshopping"
        "televendas"
      ];
    }
    {
      id = "interactive";
      iptv = [ "interactive" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "interactive"
        "interativo"
        "interactivo"
      ];
    }
    # THEMES ONLY THE CATALOGUE REACHES — no iptv-org category, no archive
    # subject, no live search: the default sources below name them, and the
    # desks of radio and TV do not list them.
    {
      id = "education";
      iptv = [ ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "education"
        "educational"
        "educação"
        "educación"
        "éducation"
        "bildung"
        "ensino"
        "aulas"
        "lectures"
        "how to"
        "howto"
        "tutorial"
        "tutoriais"
        "tutoriales"
      ];
    }
    {
      id = "space";
      iptv = [ ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "space"
        "espaço"
        "espacio"
        "nasa"
        "astronomy"
        "astronomia"
        "astronomía"
        "cosmos"
        "universe"
        "universo"
      ];
    }
    {
      id = "history";
      iptv = [ ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "history"
        "história"
        "historia"
        "histoire"
        "geschichte"
        "historical"
        "histórico"
        "archival"
      ];
    }
    {
      id = "nature";
      iptv = [ ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "nature"
        "natureza"
        "naturaleza"
        "natur"
        "animals"
        "animais"
        "animales"
        "animaux"
        "tiere"
        "wildlife"
        "pets"
      ];
    }
    {
      id = "art";
      iptv = [ ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "art"
        "arts"
        "arte"
        "artes"
        "kunst"
        "dance"
        "dança"
        "danza"
        "danse"
        "theatre"
        "theater"
        "teatro"
      ];
    }
    {
      id = "gaming";
      iptv = [ ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "gaming"
        "games"
        "jogos"
        "juegos"
        "videogames"
        "video games"
        "esports"
      ];
    }
    {
      id = "audiobooks";
      iptv = [ ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
      aliases = [
        "audiobooks"
        "audiobook"
        "audiolivros"
        "audiolivro"
        "audiolibros"
        "livres audio"
        "hörbücher"
        "librivox"
      ];
    }
  ];

  # THE CATALOGUE'S DEFAULT SOURCES, each with its own consent row (`hosts`,
  # every host it asks) and a pinned capture for its tests.
  #
  # Three of them are the desks' own, read into the catalogue: iptv-org's
  # categories, the archive's collections and the live searches are the
  # themes' `iptv`, `archive`/`subjects` and `lives` above, and are not
  # repeated here. What is here is what is NEW:
  #
  #   collections  public-domain collections of the Internet Archive, each
  #                under one theme; `media` is `movies` or `audio`
  #   searches     words the NASA Image and Video Library is asked (public
  #                domain, no key)
  #   categories   Wikimedia Commons categories of free-licensed video (by
  #                `name`), or PeerTube categories asked of SepiaSearch (by
  #                `id`, the numbers PeerTube documents: 1 music, 2 films,
  #                3 vehicles, 4 art, 5 sports, 6 travels, 7 gaming, 8 people,
  #                9 comedy, 10 entertainment, 11 news and politics, 12 how to,
  #                13 education, 14 activism, 15 science and technology,
  #                16 animals, 17 kids, 18 food; 8 and 14 have no theme)
  #
  # POSTERS: `posterHosts` names where a source's pictures come from, and a
  # source shows them from the start only when every one is among its own
  # `hosts` — the person let those already. An empty list is "any site" (a
  # channel's logo, a PeerTube instance), and waits for the person's switch.
  #
  # Nothing adult, nothing of doubtful rights. Every Commons category was
  # checked to exist, with videos directly in it, on 2026-09-30 (the API's
  # `categoryinfo`); every collection answered `mediatype: collection` the
  # same day.
  sources = [
    {
      id = "iptv";
      kind = "iptv-org";
      hosts = [ "iptv-org.github.io" ];
      posterHosts = [ ];
    }
    {
      id = "archive";
      kind = "archive";
      hosts = [ "archive.org" ];
      posterHosts = [ "archive.org" ];
      collections = [
        {
          id = "prelinger";
          theme = "history";
          media = "movies";
        }
        {
          id = "classic_tv";
          theme = "series";
          media = "movies";
        }
        {
          id = "FedFlix";
          theme = "documentary";
          media = "movies";
        }
        {
          id = "nasa";
          theme = "space";
          media = "movies";
        }
        {
          id = "librivoxaudio";
          theme = "audiobooks";
          media = "audio";
        }
      ];
    }
    {
      id = "lives";
      kind = "youtube-lives";
      hosts = [ "youtube.com" ];
      posterHosts = [ "i.ytimg.com" ];
    }
    {
      id = "nasa";
      kind = "nasa";
      hosts = [
        "images-api.nasa.gov"
        "images-assets.nasa.gov"
      ];
      posterHosts = [ "images-assets.nasa.gov" ];
      searches = [
        {
          words = "apollo";
          theme = "space";
        }
        {
          words = "artemis";
          theme = "space";
        }
        {
          words = "international space station";
          theme = "space";
        }
        {
          words = "hubble";
          theme = "space";
        }
        {
          words = "mars";
          theme = "space";
        }
        {
          words = "earth science";
          theme = "science";
        }
        {
          words = "aeronautics";
          theme = "science";
        }
      ];
    }
    {
      id = "commons";
      kind = "commons";
      hosts = [ "commons.wikimedia.org" ];
      posterHosts = [ "upload.wikimedia.org" ];
      categories = [
        {
          name = "Animated films";
          theme = "cartoons";
        }
        {
          name = "Silent films";
          theme = "classic";
        }
        {
          name = "Documentary films";
          theme = "documentary";
        }
        {
          name = "Time-lapse videos";
          theme = "relax";
        }
        {
          name = "Videos of animals";
          theme = "nature";
        }
        {
          name = "Videos of cats";
          theme = "nature";
        }
        {
          name = "Videos of dogs";
          theme = "nature";
        }
        {
          name = "Videos of nature";
          theme = "nature";
        }
        {
          name = "Videos of sports";
          theme = "sports";
        }
        {
          name = "Videos of dance";
          theme = "art";
        }
        {
          name = "Videos of concerts";
          theme = "music";
        }
        {
          name = "Videos of cooking";
          theme = "cooking";
        }
        {
          name = "Videos of science";
          theme = "science";
        }
        {
          name = "Videos of physics";
          theme = "science";
        }
        {
          name = "Videos of mathematics";
          theme = "education";
        }
        {
          name = "Videos of rocket launches";
          theme = "space";
        }
        {
          name = "Videos of Earth from space";
          theme = "space";
        }
        {
          name = "Videos of history";
          theme = "history";
        }
        {
          name = "Videos of trains";
          theme = "travel";
        }
      ];
    }
    {
      id = "peertube";
      kind = "peertube";
      hosts = [ "sepiasearch.org" ];
      posterHosts = [ ];
      categories = [
        {
          id = 1;
          theme = "music";
        }
        {
          id = 2;
          theme = "films";
        }
        {
          id = 3;
          theme = "auto";
        }
        {
          id = 4;
          theme = "art";
        }
        {
          id = 5;
          theme = "sports";
        }
        {
          id = 6;
          theme = "travel";
        }
        {
          id = 7;
          theme = "gaming";
        }
        {
          id = 9;
          theme = "comedy";
        }
        {
          id = 10;
          theme = "entertainment";
        }
        {
          id = 11;
          theme = "news";
        }
        {
          id = 12;
          theme = "education";
        }
        {
          id = 13;
          theme = "education";
        }
        {
          id = 15;
          theme = "science";
        }
        {
          id = 16;
          theme = "nature";
        }
        {
          id = 17;
          theme = "kids";
        }
        {
          id = 18;
          theme = "cooking";
        }
      ];
    }
  ];
}
