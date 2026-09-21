# The places `LEADER d` glances at — the distro's defaults, in one file.
#
# ONE PLACE, the shape `assets/notifications.nix` and `assets/updates.nix`
# already have: the data is declared here and everything else is generated from
# it — the Rust table the line draws, the text keys both languages have to
# carry, and the refusal of a zone that does not exist.
#
# WHAT A GLANCE IS
# ----------------
# A line above the modeline that says what time it is somewhere, and goes away
# by itself. It is not a clock in the chrome: invariant 14 has no room for one,
# and a clock that is always there is a clock nobody reads. This is a question
# one asks — `LEADER d` — and gets answered.
#
# THE FIELDS
# ----------
#   zone      an IANA name, as `zone1970.tab` spells it. The build REFUSES one
#             that file does not have, which is what `locale-exists` already
#             does for the machine's own zone.
#   shows     which of the four to draw for this place. All four exist for
#             every place; what differs is which are asked for.
#               clock    the time
#               date     the date, in the order `LEADER s l` chose
#               weather  the temperature, if it has arrived
#               rate     one unit of `currency` in `rate-in`
#   currency  and rate-in    ISO 4217, both. Only read when `shows.rate` is on.
#   at        latitude and longitude, for the weather. A COORDINATE and not a
#             city name: a name would need a geocoder, which is a second
#             network service and a second thing to be wrong about.
#
# NOTHING PERSONAL IS IN HERE (invariant 13). London and Brasília are the
# distro's defaults, not a record of where anybody is; the owner's own places
# go in `$XDG_CONFIG_HOME/ronoctua/world.json`, which the repository never sees.
{
  # THE PLACE THE GLANCE IS ABOUT. It draws first and it is the last thing to
  # be dropped when the line does not fit.
  primary = {
    zone = "Europe/London";
    at = {
      latitude = 51.51;
      longitude = -0.13;
    };
    currency = "GBP";
    rate-in = "BRL";
    shows = {
      clock = true;
      date = true;
      weather = true;
      rate = true;
    };
  };

  # AND THE OTHERS, in the order they are drawn. The owner asked for one, and
  # for it to show the clock and NOTHING else — which is the whole reason
  # `shows` is four switches per place and not one switch for the line.
  also = [
    {
      zone = "America/Sao_Paulo";
      at = {
        latitude = -15.79;
        longitude = -47.88;
      };
      currency = "BRL";
      rate-in = "GBP";
      shows = {
        clock = true;
        date = false;
        weather = false;
        rate = false;
      };
    }
  ];

  # HOW LONG THE LINE STAYS, by the name the notifications already use
  # (`quick` 3 s, `normal` 6 s, `long` 20 s). A glance is read in one look and
  # `Esc` closes it early, so `normal` is the answer; there is no "for ever"
  # here for the same reason there is none there.
  life = "normal";

  # WHERE THE TWO ANSWERS COME FROM, and every host is NAMED so that the screen
  # can say who is being asked. None of them needs an account or a key.
  #
  # Every fetch tells a third party this machine's address and which places it
  # watches, which is why `LEADER s l` can turn each SOURCE off and why the
  # hosts are named here rather than buried in Rust.
  #
  # # Why a source is a LIST
  #
  # Because one host being down is one host being down, and the glance then had
  # nothing to show and nothing to say: the miss was a `debug!` line and the
  # temperature simply was not on the bar. The owner read that as the weather
  # not working — *"caso haja algum erro de resposta na API principal, deve
  # haver API de fallback"*.
  #
  # They are asked IN ORDER and only until one answers, so the second costs
  # nothing on a machine where the first is up.
  #
  # # And why each one carries an `id`
  #
  # Because a fallback is not a host swap. Each of these has its own path and
  # its own JSON, so what a declaration can hold is WHO is asked and how often;
  # the reading is Rust, chosen by this id. An id with no reader is a test that
  # fails (`every_declared_source_has_a_reader`), never a surprise at run time.
  sources = {
    weather = {
      # Open-Meteo, current temperature by coordinate, and met.no — the
      # Norwegian Meteorological Institute — behind it. Both are free of
      # charge, keyless, and answer a plain coordinate.
      from = [
        {
          id = "open-meteo";
          host = "api.open-meteo.com";
        }
        {
          id = "met-no";
          host = "api.met.no";
        }
      ];
      every-minutes = 30;
    };
    rate = {
      # Frankfurter, which serves the European Central Bank's daily reference
      # rates, and open.er-api.com behind it. Daily is what the ECB publishes,
      # so asking more often would be asking for the same number.
      from = [
        {
          id = "frankfurter";
          host = "api.frankfurter.dev";
        }
        {
          id = "er-api";
          host = "open.er-api.com";
        }
      ];
      every-minutes = 360;
    };
  };
}
