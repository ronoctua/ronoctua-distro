//! radio-browser: the world's radio stations.
//!
//! # Which server
//!
//! radio-browser is a handful of mirrors, and which exist changes: on
//! 2026-09-28 `/json/servers` named only `de1`, `fi1` no longer resolved and
//! `de2` pointed at `de1`'s address. So nothing is baked in but the ENTRY,
//! `all.api.radio-browser.info` (a name for every mirror at once, which the
//! certificate covers), and the list is asked at run time
//! ([`Radio::discover`]). A server the list names is used only when it is a
//! name under `api.radio-browser.info` ([`is_server`]): an answer never sends
//! the player to a host it did not choose.
//!
//! # What is asked
//!
//! Stations by country, genre (tag), language and name, ordered by how often
//! they are played, with the broken ones hidden by the server. Countries, tags
//! and languages come with their number of stations. A station's coordinate is
//! often null — the top three Brazilian stations have none — and that is
//! normal ([`Station::at`] is an `Option`).
//!
//! # The click
//!
//! radio-browser counts a station as played when `/json/url/<uuid>` is asked,
//! and asks clients to do it "every time a user starts playing a stream" —
//! counted once per address, station and day. The daemon asks it AFTER the
//! sound starts, never on a list ([`Radio::click`]).

use ronoctua_playlist::own::{Origin, Stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ask::{Net, Request, answered};
use crate::country::{self, Code};
use crate::json::{array, flag, is_web_address, number, text, texts, value, whole};
use crate::query::{escape, words};
use crate::{Refusal, Source, Spot, Why};

/// The name for every mirror at once.
pub const ENTRY: &str = "all.api.radio-browser.info";

/// The domain every server of radio-browser is under.
const DOMAIN: &str = ".api.radio-browser.info";

/// The longest search or tag or language, in characters.
pub const WORDS_MOST: usize = 100;

/// The most stations one list may hold, whatever the caller asks.
pub const STATIONS_CEILING: u32 = 1000;

/// Whether `host` is a radio-browser server: one DNS label, then
/// `.api.radio-browser.info`.
#[must_use]
pub fn is_server(host: &str) -> bool {
    host.strip_suffix(DOMAIN).is_some_and(|label| {
        (1..=63).contains(&label.len())
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    })
}

fn refused(why: Why) -> Refusal {
    Refusal::new(Source::Radio, why)
}

/// One radio station.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Station {
    /// Its `stationuuid`: what the click and a favourite name it by.
    pub uuid: String,
    /// What it is called.
    pub name: String,
    /// The address that plays it: the one radio-browser resolved, else the
    /// one it was given.
    pub url: String,
    /// The other address, when the two differ.
    pub alternate: Option<String>,
    /// Its web page, when said.
    pub homepage: Option<String>,
    /// Its country, when the code names one.
    pub country: Option<Code>,
    /// Its genres, as radio-browser's tags.
    pub tags: Vec<String>,
    /// Its languages, as radio-browser writes them (`brazilian portuguese`).
    pub languages: Vec<String>,
    /// Its codec (`MP3`, `AAC+`), when said.
    pub codec: Option<String>,
    /// Its bitrate in kilobits per second; `0` when not said.
    pub bitrate_kbps: u32,
    /// Whether it is an HLS playlist rather than a plain stream.
    pub hls: bool,
    /// How many people voted for it.
    pub votes: u32,
    /// How many times it was played recently.
    pub clicks: u32,
    /// Where it is, when said — often not.
    pub at: Option<Spot>,
}

impl Station {
    /// The stream the queue holds: live, the other address as the one tried
    /// next, and where it was found (for the click).
    #[must_use]
    pub fn stream(&self) -> Stream {
        Stream {
            url: self.url.clone(),
            title: Some(self.name.clone()),
            referrer: None,
            user_agent: None,
            alternates: self.alternate.iter().cloned().collect(),
            origin: Origin::RadioBrowser {
                uuid: self.uuid.clone(),
            },
            live: true,
        }
    }
}

/// A station's id as radio-browser writes one: a UUID.
#[must_use]
pub fn is_uuid(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(n, b)| match n {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// A country, with how many stations it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountryCount {
    /// The country.
    pub country: Code,
    /// How many stations.
    pub stations: u32,
}

/// A genre or a language, with how many stations have it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Named {
    /// The name, as radio-browser writes it (it is what is asked back).
    pub name: String,
    /// An ISO 639 code, for a language that has one.
    pub code: Option<String>,
    /// How many stations.
    pub stations: u32,
}

/// The languages radio-browser names by words that say MORE than the ISO
/// code it gives them — a region the code leaves out — each with the
/// language tag it is. `portuguese` and `brazilian portuguese` both come as
/// `pt`; only the words tell the second apart. Measured on its language
/// list; a word not here is named by its code alone.
const LANGUAGE_REGIONS: [(&str, &str); 1] = [("brazilian portuguese", "pt-BR")];

/// The language tag (`pt-BR`) radio-browser's `name` for a language means,
/// when its words say a region its ISO code does not. Pure.
#[must_use]
pub fn language_region(name: &str) -> Option<&'static str> {
    let name = name.trim();
    LANGUAGE_REGIONS
        .iter()
        .find(|(word, _)| word.eq_ignore_ascii_case(name))
        .map(|(_, tag)| *tag)
}

/// What a list of stations is filtered by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum By {
    /// A country.
    Country(Code),
    /// A genre, exactly as a tag names it.
    Tag(String),
    /// A language, exactly as the language list names it.
    Language(String),
    /// Words in the name.
    Name(String),
}

/// The path of a list of stations, at most `most`, the most played first —
/// from the `skip`th on: a country has thousands, and a door walks them a
/// page at a time. Pure.
///
/// # Errors
///
/// [`Why::NothingToAsk`] for a tag, a language or a name with no words.
pub fn stations_path(by: &By, most: u32, skip: u32) -> Result<String, Why> {
    let filter = match by {
        By::Country(code) => format!("countrycode={code}"),
        By::Tag(t) => format!(
            "tag={}&tagExact=true",
            escape(&words(t, WORDS_MOST).ok_or(Why::NothingToAsk)?)
        ),
        By::Language(l) => format!(
            "language={}&languageExact=true",
            escape(&words(l, WORDS_MOST).ok_or(Why::NothingToAsk)?)
        ),
        By::Name(n) => format!(
            "name={}",
            escape(&words(n, WORDS_MOST).ok_or(Why::NothingToAsk)?)
        ),
    };
    let offset = if skip == 0 {
        String::new()
    } else {
        format!("&offset={skip}")
    };
    Ok(format!(
        "/json/stations/search?{filter}&hidebroken=true&order=clickcount&reverse=true&limit={}{offset}",
        most.clamp(1, STATIONS_CEILING)
    ))
}

/// A list answer's ceiling in bytes: four kilobytes a station (one measures
/// about a kilobyte and a half), and never under [`crate::MOST_BYTES`].
fn most_bytes(most: u32) -> u64 {
    (u64::from(most.clamp(1, STATIONS_CEILING)) * 4096).max(crate::MOST_BYTES)
}

/// The servers `/json/servers` names that are radio-browser's, each once, in
/// the order given. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] for what is not a list.
pub fn parse_servers(bytes: &[u8]) -> Result<Vec<String>, Refusal> {
    let mut out: Vec<String> = Vec::new();
    for item in array(Source::Radio, bytes)? {
        if let Some(name) = text(&item, "name").map(|n| n.to_ascii_lowercase())
            && is_server(&name)
            && !out.contains(&name)
        {
            out.push(name);
        }
    }
    Ok(out)
}

/// One station out of radio-browser's JSON; `None` for one with no id, no
/// name or no web address.
fn station_of(v: &Value) -> Option<Station> {
    let uuid = text(v, "stationuuid").filter(|u| is_uuid(u))?;
    let name = text(v, "name")?;
    let given = text(v, "url").filter(|u| is_web_address(u));
    let resolved = text(v, "url_resolved").filter(|u| is_web_address(u));
    let (url, alternate) = match (resolved, given) {
        (Some(r), Some(g)) if r != g => (r, Some(g)),
        (Some(r), _) => (r, None),
        (None, Some(g)) => (g, None),
        (None, None) => return None,
    };
    let at = match (number(v, "geo_lat"), number(v, "geo_long")) {
        (Some(lat), Some(lon)) => Spot::of(lat, lon),
        _ => None,
    };
    let clamp = |n: Option<u64>| u32::try_from(n.unwrap_or(0)).unwrap_or(u32::MAX);
    Some(Station {
        uuid,
        name,
        url,
        alternate,
        homepage: text(v, "homepage").filter(|u| is_web_address(u)),
        country: text(v, "countrycode").and_then(|c| country::normal(&c).ok()),
        tags: texts(v, "tags"),
        languages: texts(v, "language"),
        codec: text(v, "codec"),
        bitrate_kbps: clamp(whole(v, "bitrate")),
        hls: flag(v, "hls").unwrap_or(false),
        votes: clamp(whole(v, "votes")),
        clicks: clamp(whole(v, "clickcount")),
        at,
    })
}

/// A list of stations, read; each station once (by id), at most `most`.
/// Pure.
///
/// # Errors
///
/// [`Why::Malformed`] for what is not a list.
pub fn parse_stations(bytes: &[u8], most: u32) -> Result<Vec<Station>, Refusal> {
    let mut out: Vec<Station> = Vec::new();
    for item in array(Source::Radio, bytes)? {
        if out.len() >= most as usize {
            break;
        }
        if let Some(s) = station_of(&item)
            && !out.iter().any(|o| o.uuid == s.uuid)
        {
            out.push(s);
        }
    }
    Ok(out)
}

/// `/json/countries`, read: every country the code names, once — `UK` and
/// `GB` added together — the most stations first. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] for what is not a list.
pub fn parse_countries(bytes: &[u8]) -> Result<Vec<CountryCount>, Refusal> {
    let mut out: Vec<CountryCount> = Vec::new();
    for item in array(Source::Radio, bytes)? {
        let Some(code) = text(&item, "iso_3166_1").and_then(|c| country::normal(&c).ok()) else {
            continue;
        };
        let stations = u32::try_from(whole(&item, "stationcount").unwrap_or(0)).unwrap_or(u32::MAX);
        match out.iter_mut().find(|c| c.country == code) {
            Some(c) => c.stations = c.stations.saturating_add(stations),
            None => out.push(CountryCount {
                country: code,
                stations,
            }),
        }
    }
    out.retain(|c| c.stations > 0);
    out.sort_by(|a, b| b.stations.cmp(&a.stations).then(a.country.cmp(&b.country)));
    Ok(out)
}

/// `/json/tags` or `/json/languages`, read: each name once, the most stations
/// first. Pure.
///
/// # Errors
///
/// [`Why::Malformed`] for what is not a list.
pub fn parse_named(bytes: &[u8]) -> Result<Vec<Named>, Refusal> {
    let mut out: Vec<Named> = Vec::new();
    for item in array(Source::Radio, bytes)? {
        let Some(name) = text(&item, "name") else {
            continue;
        };
        if out.iter().any(|n| n.name == name) {
            continue;
        }
        out.push(Named {
            name,
            code: text(&item, "iso_639"),
            stations: u32::try_from(whole(&item, "stationcount").unwrap_or(0)).unwrap_or(u32::MAX),
        });
    }
    out.retain(|n| n.stations > 0);
    out.sort_by(|a, b| b.stations.cmp(&a.stations).then(a.name.cmp(&b.name)));
    Ok(out)
}

/// The path of a list of tags or languages, the most stations first. Pure.
fn named_path(what: &str, most: u32) -> String {
    format!(
        "/json/{what}?order=stationcount&reverse=true&hidebroken=true&limit={}",
        most.clamp(1, STATIONS_CEILING)
    )
}

/// The server a session asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Radio {
    host: String,
}

impl Radio {
    /// The entry itself: every mirror, by one name.
    #[must_use]
    pub fn entry() -> Self {
        Self {
            host: ENTRY.to_owned(),
        }
    }

    /// One server, when `host` is one of radio-browser's.
    #[must_use]
    pub fn at(host: &str) -> Option<Self> {
        (host == ENTRY || is_server(host)).then(|| Self {
            host: host.to_owned(),
        })
    }

    /// Which server this is.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    fn url(&self, path: &str) -> String {
        format!("https://{}{path}", self.host)
    }

    /// The servers radio-browser names today, asked of the entry.
    ///
    /// # Errors
    ///
    /// The entry's refusal.
    pub fn servers(net: &dyn Net) -> Result<Vec<String>, Refusal> {
        let body = answered(
            net,
            &Request::get(Source::Radio, Self::entry().url("/json/servers")),
        )?;
        parse_servers(&body)
    }

    /// A server to ask: one of those radio-browser names today, chosen by
    /// `seed` (so two machines do not all ask the first), or the entry itself
    /// when the list names none or cannot be had. Never fails: the entry is
    /// always a server.
    #[must_use]
    pub fn discover(net: &dyn Net, seed: u64) -> Self {
        match Self::servers(net) {
            Ok(list) if !list.is_empty() => {
                let n = usize::try_from(seed % list.len() as u64).unwrap_or(0);
                Self::at(&list[n]).unwrap_or_else(Self::entry)
            }
            _ => Self::entry(),
        }
    }

    /// Every country with stations, the most first.
    ///
    /// # Errors
    ///
    /// The server's refusal, or [`Why::Malformed`].
    pub fn countries(&self, net: &dyn Net) -> Result<Vec<CountryCount>, Refusal> {
        let path = "/json/countries?order=stationcount&reverse=true&hidebroken=true";
        parse_countries(&answered(
            net,
            &Request::get(Source::Radio, self.url(path)),
        )?)
    }

    /// Stations, filtered `by`, at most `most`, the most played first.
    ///
    /// # Errors
    ///
    /// [`Why::NothingToAsk`], the server's refusal, or [`Why::Malformed`].
    pub fn stations(
        &self,
        net: &dyn Net,
        by: &By,
        most: u32,
        skip: u32,
    ) -> Result<Vec<Station>, Refusal> {
        let path = stations_path(by, most, skip).map_err(refused)?;
        let request = Request::get(Source::Radio, self.url(&path)).most(most_bytes(most));
        parse_stations(&answered(net, &request)?, most.clamp(1, STATIONS_CEILING))
    }

    /// Stations whose NAME has `words`; when none has, stations with that
    /// genre — what a person typing "jazz" means either way.
    ///
    /// # Errors
    ///
    /// As [`Radio::stations`].
    pub fn search(
        &self,
        net: &dyn Net,
        words: &str,
        most: u32,
        skip: u32,
    ) -> Result<Vec<Station>, Refusal> {
        // EACH PAGE THE SAME WAY: names from the `skip`th on, and the genre's
        // from the `skip`th on when no name is there — the first page decided
        // which the search is, and the later ones follow it.
        let named = self.stations(net, &By::Name(words.to_owned()), most, skip)?;
        if !named.is_empty() {
            return Ok(named);
        }
        self.stations(net, &By::Tag(words.to_lowercase()), most, skip)
    }

    /// The genres with the most stations, at most `most`.
    ///
    /// # Errors
    ///
    /// The server's refusal, or [`Why::Malformed`].
    pub fn tags(&self, net: &dyn Net, most: u32) -> Result<Vec<Named>, Refusal> {
        let request =
            Request::get(Source::Radio, self.url(&named_path("tags", most))).most(most_bytes(most));
        parse_named(&answered(net, &request)?)
    }

    /// The languages with the most stations, at most `most`.
    ///
    /// # Errors
    ///
    /// The server's refusal, or [`Why::Malformed`].
    pub fn languages(&self, net: &dyn Net, most: u32) -> Result<Vec<Named>, Refusal> {
        let request = Request::get(Source::Radio, self.url(&named_path("languages", most)))
            .most(most_bytes(most));
        parse_named(&answered(net, &request)?)
    }

    /// Tells radio-browser the station `uuid` was played. Ask it once the
    /// sound has started, never for a list.
    ///
    /// # Errors
    ///
    /// [`Why::NotAName`] for an id that is not a UUID (nothing is sent), the
    /// server's refusal, or [`Why::Declined`] when it answered `ok: false`.
    pub fn click(&self, net: &dyn Net, uuid: &str) -> Result<(), Refusal> {
        if !is_uuid(uuid) {
            return Err(refused(Why::NotAName));
        }
        let body = answered(
            net,
            &Request::get(Source::Radio, self.url(&format!("/json/url/{uuid}"))),
        )?;
        let answer = value(Source::Radio, &body)?;
        match flag(&answer, "ok") {
            Some(true) => Ok(()),
            Some(false) => Err(refused(Why::Declined)),
            None => Err(refused(Why::Malformed)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canned::Canned;

    const UUID_A: &str = "7ada8a81-5ae1-418c-8f18-51d2f38d86a4";
    const UUID_B: &str = "901c92bd-a757-449a-b8ce-652b079fd38a";

    fn stations_json() -> String {
        format!(
            r#"[
            {{"stationuuid":"{UUID_A}","name":"Bossa Jazz Brazil",
              "url":"https://centova5.example:20104/live",
              "url_resolved":"https://centova5.example:20104/live",
              "homepage":"https://bossajazz.example/","tags":"bossa nova,jazz,mpb",
              "countrycode":"BR","language":"brazilian portuguese","codec":"AAC+",
              "bitrate":64,"hls":0,"votes":8856,"clickcount":91,
              "geo_lat":null,"geo_long":null}},
            {{"stationuuid":"{UUID_B}","name":"  Mix 106.3 FM ",
              "url":"https://playerservices.example/redirect/MIXFM.aac",
              "url_resolved":"https://relay.example/MIXFM.aac",
              "tags":"","countrycode":"UK","language":"english,portuguese",
              "bitrate":"128","hls":1,"votes":10,"clickcount":5,
              "geo_lat":-23.55,"geo_long":-46.63}},
            {{"stationuuid":"not-a-uuid","name":"Nameless id","url":"https://x.example/"}},
            {{"stationuuid":"{UUID_A}","name":"Same again","url":"https://y.example/"}},
            {{"stationuuid":"11111111-2222-3333-4444-555555555555","name":"No address",
              "url":"rtmp://x.example/live","url_resolved":""}},
            {{"stationuuid":"11111111-2222-3333-4444-555555555556","name":"Null island",
              "url":"http://z.example/","geo_lat":0.0,"geo_long":0.0}}
        ]"#
        )
    }

    #[test]
    fn a_station_without_a_coordinate_has_none_and_is_kept() {
        let got = parse_stations(stations_json().as_bytes(), 100).unwrap();
        let names: Vec<&str> = got.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Bossa Jazz Brazil", "Mix 106.3 FM", "Null island"]);
        let bossa = &got[0];
        assert_eq!(bossa.at, None);
        assert_eq!(bossa.alternate, None, "the same address twice is one");
        assert_eq!(bossa.tags, ["bossa nova", "jazz", "mpb"]);
        assert_eq!(bossa.country, Some(Code(*b"BR")));
        assert_eq!(bossa.bitrate_kbps, 64);
        assert!(!bossa.hls);
        let mix = &got[1];
        assert_eq!(
            mix.url, "https://relay.example/MIXFM.aac",
            "the resolved one first"
        );
        assert_eq!(
            mix.alternate.as_deref(),
            Some("https://playerservices.example/redirect/MIXFM.aac")
        );
        assert_eq!(mix.country, Some(Code(*b"GB")), "UK is GB");
        assert_eq!(mix.bitrate_kbps, 128);
        assert!(mix.hls);
        assert_eq!(mix.languages, ["english", "portuguese"]);
        let at = mix.at.expect("a coordinate");
        assert!((at.lat + 23.55).abs() < 1e-4 && (at.lon + 46.63).abs() < 1e-4);
        assert_eq!(got[2].at, None, "0,0 is unset, not the Gulf of Guinea");

        assert_eq!(
            parse_stations(stations_json().as_bytes(), 1).unwrap().len(),
            1
        );
        assert_eq!(parse_stations(b"{}", 10).unwrap_err().why, Why::Malformed);
    }

    #[test]
    fn a_station_becomes_a_live_stream_that_remembers_where_it_came_from() {
        let got = parse_stations(stations_json().as_bytes(), 100).unwrap();
        let s = got[1].stream();
        assert!(s.live);
        assert_eq!(s.url, "https://relay.example/MIXFM.aac");
        assert_eq!(
            s.alternates,
            ["https://playerservices.example/redirect/MIXFM.aac"]
        );
        assert_eq!(
            s.origin,
            Origin::RadioBrowser {
                uuid: UUID_B.to_owned()
            }
        );
        assert_eq!(s.title.as_deref(), Some("Mix 106.3 FM"));
    }

    #[test]
    fn only_radio_browsers_own_servers_are_asked() {
        let body = br#"[{"ip":"91.98.4.78","name":"de1.api.radio-browser.info"},
            {"ip":"2a01:4f8::1","name":"de1.api.radio-browser.info"},
            {"name":"evil.example"},{"name":"x.api.radio-browser.info.evil.example"},
            {"name":"FI1.api.radio-browser.info"},{"name":"-a.api.radio-browser.info"}]"#;
        assert_eq!(
            parse_servers(body).unwrap(),
            ["de1.api.radio-browser.info", "fi1.api.radio-browser.info"]
        );
        assert!(Radio::at("evil.example").is_none());
        assert!(Radio::at(ENTRY).is_some());
    }

    #[test]
    fn discovery_asks_the_entry_and_falls_back_to_it() {
        let net = Canned::new(&[(
            "https://all.api.radio-browser.info/json/servers",
            200,
            br#"[{"name":"de1.api.radio-browser.info"},{"name":"nl1.api.radio-browser.info"}]"#,
        )]);
        assert_eq!(
            Radio::discover(&net, 1).host(),
            "nl1.api.radio-browser.info"
        );
        assert_eq!(
            net.urls(),
            ["https://all.api.radio-browser.info/json/servers"]
        );
        // THE LIST CANNOT BE HAD: the entry answers for every mirror.
        let down = Canned::new(&[]);
        assert_eq!(Radio::discover(&down, 0).host(), ENTRY);
        let empty = Canned::new(&[(
            "https://all.api.radio-browser.info/json/servers",
            200,
            b"[]",
        )]);
        assert_eq!(Radio::discover(&empty, 0).host(), ENTRY);
    }

    #[test]
    fn each_list_is_asked_by_its_own_filter() {
        let br = country::normal("BR").unwrap();
        assert_eq!(
            stations_path(&By::Country(br), 500, 0).unwrap(),
            "/json/stations/search?countrycode=BR&hidebroken=true&order=clickcount&reverse=true&limit=500"
        );
        assert_eq!(
            stations_path(&By::Tag("bossa nova".into()), 5000, 0).unwrap(),
            "/json/stations/search?tag=bossa%20nova&tagExact=true&hidebroken=true&order=clickcount&reverse=true&limit=1000"
        );
        assert_eq!(
            stations_path(&By::Language("brazilian portuguese".into()), 50, 0).unwrap(),
            "/json/stations/search?language=brazilian%20portuguese&languageExact=true&hidebroken=true&order=clickcount&reverse=true&limit=50"
        );
        assert_eq!(
            stations_path(&By::Name("  ".into()), 50, 0),
            Err(Why::NothingToAsk)
        );
        // THE NEXT PAGE starts where the one before stopped.
        assert_eq!(
            stations_path(&By::Country(br), 500, 1000).unwrap(),
            "/json/stations/search?countrycode=BR&hidebroken=true&order=clickcount&reverse=true&limit=500&offset=1000"
        );
    }

    #[test]
    fn a_language_whose_words_say_its_region_is_that_region() {
        assert_eq!(language_region("brazilian portuguese"), Some("pt-BR"));
        assert_eq!(language_region(" Brazilian Portuguese "), Some("pt-BR"));
        assert_eq!(language_region("portuguese"), None);
    }

    #[test]
    fn a_search_by_name_that_finds_nothing_asks_the_genre() {
        let server = Radio::at("de1.api.radio-browser.info").unwrap();
        let net = Canned::new(&[
            (
                "https://de1.api.radio-browser.info/json/stations/search?name=Jazz&hidebroken=true&order=clickcount&reverse=true&limit=10",
                200,
                b"[]",
            ),
            (
                "https://de1.api.radio-browser.info/json/stations/search?tag=jazz&tagExact=true&hidebroken=true&order=clickcount&reverse=true&limit=10",
                200,
                stations_json().as_bytes(),
            ),
        ]);
        let found = server.search(&net, "Jazz", 10, 0).unwrap();
        assert_eq!(found.len(), 3);
        assert_eq!(net.urls().len(), 2);
    }

    #[test]
    fn countries_are_counted_once_each_and_uk_is_gb() {
        let body = br#"[{"name":"Andorra","iso_3166_1":"AD","stationcount":12},
            {"name":"The United Kingdom","iso_3166_1":"GB","stationcount":100},
            {"name":"UK","iso_3166_1":"UK","stationcount":5},
            {"name":"Nowhere","iso_3166_1":"ZZ","stationcount":9},
            {"name":"Empty","iso_3166_1":"FR","stationcount":0},
            {"name":"Brazil","iso_3166_1":"BR","stationcount":"105"}]"#;
        let got = parse_countries(body).unwrap();
        let seen: Vec<(&str, u32)> = got
            .iter()
            .map(|c| (c.country.as_str(), c.stations))
            .collect();
        // A TIE is broken by the code, so the order never depends on the
        // server's.
        assert_eq!(seen, [("BR", 105), ("GB", 105), ("AD", 12)]);
    }

    #[test]
    fn tags_and_languages_are_read_with_their_counts() {
        let body = br#"[{"name":"pop","stationcount":900},{"name":"jazz","stationcount":"300"},
            {"name":"pop","stationcount":1},{"name":"","stationcount":5},
            {"name":"english","iso_639":"en","stationcount":5000}]"#;
        let got = parse_named(body).unwrap();
        let names: Vec<(&str, Option<&str>, u32)> = got
            .iter()
            .map(|n| (n.name.as_str(), n.code.as_deref(), n.stations))
            .collect();
        assert_eq!(
            names,
            [
                ("english", Some("en"), 5000),
                ("pop", None, 900),
                ("jazz", None, 300)
            ]
        );
    }

    #[test]
    fn the_click_is_sent_only_for_a_station_id_and_read_back() {
        let server = Radio::at("de1.api.radio-browser.info").unwrap();
        let url = format!("https://de1.api.radio-browser.info/json/url/{UUID_A}");
        let net = Canned::new(&[(
            url.as_str(),
            200,
            br#"{"ok":true,"message":"retrieved station url"}"#,
        )]);
        assert_eq!(server.click(&net, UUID_A), Ok(()));
        let nothing = Canned::new(&[]);
        assert_eq!(
            server.click(&nothing, "../json/servers").unwrap_err().why,
            Why::NotAName
        );
        assert!(nothing.asked().is_empty(), "a bad id is never sent");
        let no = Canned::new(&[(url.as_str(), 200, br#"{"ok":false}"#)]);
        assert_eq!(server.click(&no, UUID_A).unwrap_err().why, Why::Declined);
        let gone = Canned::new(&[(url.as_str(), 404, b"")]);
        assert_eq!(
            server.click(&gone, UUID_A).unwrap_err().why,
            Why::NothingFound
        );
    }
}
