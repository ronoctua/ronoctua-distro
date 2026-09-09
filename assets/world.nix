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

  # WHERE THE TWO ANSWERS COME FROM, and both are named so that the screen can
  # say who is being asked. Neither needs an account or a key.
  #
  # Every fetch tells a third party this machine's address and which places it
  # watches, which is why `LEADER s l` can turn each of them off and why they
  # are named here rather than buried in Rust.
  sources = {
    # Open-Meteo: current temperature by coordinate.
    weather = {
      host = "api.open-meteo.com";
      every-minutes = 30;
    };
    # Frankfurter, which serves the European Central Bank's daily reference
    # rates. Daily is what the ECB publishes, so asking more often would be
    # asking for the same number.
    rate = {
      host = "api.frankfurter.dev";
      every-minutes = 360;
    };
  };
}
