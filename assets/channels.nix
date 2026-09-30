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
#   - a category has no source at all;
#   - an id repeats, or is not kebab-case (it becomes the text key
#     `player-channels-<id>`, which both languages must carry).
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

  categories = [
    {
      id = "action";
      iptv = [ ];
      archive = [ "feature_films" ];
      subjects = [ "action" ];
      lives = [ "action movies 24/7" ];
    }
    {
      # NO LIVE TV: iptv-org has no category for anime, and its `animation`
      # is every cartoon — which is the Cartoons theme's.
      id = "anime";
      iptv = [ ];
      archive = [ "animationandcartoons" ];
      subjects = [ "anime" ];
      lives = [ "anime 24/7" ];
    }
    {
      id = "films";
      iptv = [ "movies" ];
      archive = [ "feature_films" ];
      subjects = [ ];
      lives = [ "movies 24/7" ];
    }
    {
      id = "series";
      iptv = [ "series" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "tv series 24/7" ];
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
    }
    {
      id = "documentary";
      iptv = [ "documentary" ];
      archive = [ "feature_films" ];
      subjects = [ "documentary" ];
      lives = [ "documentary 24/7" ];
    }
    {
      id = "news";
      iptv = [ "news" ];
      archive = [ "newsandpublicaffairs" ];
      subjects = [ ];
      lives = [ "news live" ];
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
    }
    {
      id = "sports";
      iptv = [ "sports" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "sports 24/7" ];
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
    }
    {
      id = "comedy";
      iptv = [ "comedy" ];
      archive = [ "feature_films" ];
      subjects = [ "comedy" ];
      lives = [ "comedy 24/7" ];
    }
    {
      id = "scifi-horror";
      iptv = [ ];
      archive = [ "SciFi_Horror" ];
      subjects = [ ];
      lives = [ "horror movies 24/7" ];
    }
    {
      id = "travel";
      iptv = [ "travel" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "travel 24/7" ];
    }
    # EVERY OTHER CATEGORY iptv-org has, each a theme of its own — so no
    # channel of the index is reachable only by a search.
    {
      id = "general";
      iptv = [ "general" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
    }
    {
      id = "entertainment";
      iptv = [ "entertainment" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
    }
    {
      id = "culture";
      iptv = [ "culture" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
    }
    {
      id = "lifestyle";
      iptv = [ "lifestyle" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
    }
    {
      id = "cooking";
      iptv = [ "cooking" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "cooking live" ];
    }
    {
      id = "outdoor";
      iptv = [ "outdoor" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
    }
    {
      id = "auto";
      iptv = [ "auto" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
    }
    {
      id = "business";
      iptv = [ "business" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "business news live" ];
    }
    {
      id = "weather";
      iptv = [ "weather" ];
      archive = [ ];
      subjects = [ ];
      lives = [ "weather live" ];
    }
    {
      id = "public";
      iptv = [ "public" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
    }
    {
      id = "legislative";
      iptv = [ "legislative" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
    }
    {
      id = "religious";
      iptv = [ "religious" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
    }
    {
      id = "shop";
      iptv = [ "shop" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
    }
    {
      id = "interactive";
      iptv = [ "interactive" ];
      archive = [ ];
      subjects = [ ];
      lives = [ ];
    }
  ];
}
