//! The one [`Reach`] that goes out: YouTube's feeds and thumbnails over
//! plain HTTP through `ronoctua-http`, its tabs and details through
//! `ronoctua-ytdlp` — this crate is the boundary with YouTube over HTTP, and
//! the yt-dlp command lines stay in theirs.
//!
//! # Consent at the moment of asking
//!
//! Every feed and every thumbnail mints its `Pass` from
//! `ronoctua_consent::pass` right before the request: adding a channel let
//! [`crate::HOSTS`], and a person who took that back stops the next request
//! ([`Stop::NotLet`]). The yt-dlp requests go where the person pointed —
//! their channels — and are the same consent.
//!
//! # A test's feeds
//!
//! `RONOCTUA_VIDEO_METRICS_FEEDS` may name a LOOPBACK `http` origin
//! (`http://127.0.0.1:<port>`) that serves the feeds instead of YouTube — the
//! VM test's fixture. Anything else in it is refused
//! ([`crate::Why::Invalid`]), never taken for a plausible default. With a
//! loopback origin a scan asks `i.ytimg.com` for NOTHING: the thumbnails are
//! YouTube's pictures of YouTube's videos, a fixture's videos have none, and
//! a test machine with no way out would otherwise stop every scan at its
//! pictures (`Stop::Offline`). Each one is "not there", which a scan passes
//! over.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use ronoctua_consent::Pass;
use ronoctua_http::web;
use ronoctua_ytdlp::youtube::{self, Details, Said, Tab, TabEntry};

use crate::feed::{self, Which};
use crate::scan::{Missed, Reach};
use crate::thumbs;
use crate::{ChannelId, FEEDS_HOST, Refusal, Stop, THUMBS_HOST, VideoId, Why};

/// The variable that names a loopback origin for the feeds.
pub(crate) const FEEDS_VARIABLE: &str = "RONOCTUA_VIDEO_METRICS_FEEDS";

/// Who is asking.
const USER_AGENT: &str = concat!(
    "ronoctua-video-metrics/",
    env!("CARGO_PKG_VERSION"),
    " ( https://github.com/ronoctua/ronoctua-distro )"
);

/// How long one request waits per hop.
const PATIENCE: Duration = Duration::from_secs(20);

/// The pause between two details: 4 s, give or take 2.
const PAUSE_S: u64 = 4;
const PAUSE_SPREAD_MS: u64 = 2000;

/// Where the feeds are asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    /// `https://www.youtube.com`.
    YouTube,
    /// A loopback fixture.
    Loopback(SocketAddr),
}

/// The feeds' origin a variable names. Pure.
///
/// # Errors
///
/// [`Why::Invalid`] for anything but `http://<loopback address>:<port>`.
pub(crate) fn origin_of(set: Option<&str>) -> Result<Origin, Refusal> {
    let Some(set) = set.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(Origin::YouTube);
    };
    let invalid = || Refusal::of(Why::Invalid);
    let address: SocketAddr = set
        .strip_prefix("http://")
        .map(|a| a.trim_end_matches('/'))
        .and_then(|a| a.parse().ok())
        .ok_or_else(invalid)?;
    if !IpAddr::is_loopback(&address.ip()) {
        return Err(invalid());
    }
    Ok(Origin::Loopback(address))
}

/// The machine's network.
pub(crate) struct Internet {
    feeds: Origin,
    seed: std::cell::Cell<u64>,
}

impl Internet {
    /// The network, with the feeds' origin the environment names.
    pub(crate) fn new() -> Result<Self, Refusal> {
        let feeds = origin_of(std::env::var(FEEDS_VARIABLE).ok().as_deref())?;
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| u64::from(d.subsec_nanos()))
            ^ u64::from(std::process::id());
        Ok(Self {
            feeds,
            seed: std::cell::Cell::new(seed | 1),
        })
    }

    /// The next pause between two details: 4 s, give or take 2 (xorshift —
    /// a spread, not a secret).
    fn next_pause(&self) -> Duration {
        let mut x = self.seed.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.seed.set(x);
        let spread = x % (2 * PAUSE_SPREAD_MS + 1);
        Duration::from_millis((PAUSE_S * 1000 + spread).saturating_sub(PAUSE_SPREAD_MS))
    }
}

/// A `Pass` for `host`, or the run's stop.
fn pass(host: &str) -> Result<Pass, Missed> {
    ronoctua_consent::pass(host).map_err(|_| Missed::Stop(Stop::NotLet, None))
}

/// What an HTTP status means to a scan. Pure.
pub(crate) fn missed_of_status(status: u16) -> Option<Missed> {
    match status {
        200..=299 => None,
        404 | 410 => Some(Missed::NotFound),
        429 => Some(Missed::Stop(
            Stop::TooManyRequests,
            Some(format!("HTTP {status}")),
        )),
        _ => Some(Missed::Failed(format!("HTTP {status}"))),
    }
}

/// What a web failure means to a scan. Pure.
pub(crate) fn missed_of_web(e: &web::Error) -> Missed {
    match e.kind {
        web::Kind::Offline => Missed::Stop(Stop::Offline, Some(e.trouble.clone())),
        _ => Missed::Failed(e.to_string()),
    }
}

/// What a yt-dlp failure means to a scan. Pure.
pub(crate) fn missed_of_tool(failed: youtube::Failed) -> Missed {
    match (failed.said, failed.trouble) {
        (Said::BotCheck, _) => Missed::Stop(Stop::BotCheck, failed.line),
        (Said::TooManyRequests, _) => Missed::Stop(Stop::TooManyRequests, failed.line),
        (Said::Gone, _) => Missed::Gone,
        (_, ronoctua_ytdlp::Trouble::Absent | ronoctua_ytdlp::Trouble::Unstartable) => {
            Missed::NoTool
        }
        (_, trouble) => Missed::Failed(
            failed
                .line
                .unwrap_or_else(|| format!("yt-dlp: {}", trouble.id())),
        ),
    }
}

/// One GET of `url` under `pass`.
///
/// The first host is checked here too, so a pass for another host is the
/// scan's own refusal (`Stop::NotLet`) and not a transport error; the request
/// itself goes through `request_let`, which checks it again where the bytes
/// leave and carries no consent across a redirect to another host.
fn get(pass: &Pass, url: &str, most: u64) -> Result<Vec<u8>, Missed> {
    let host = url
        .strip_prefix("https://")
        .and_then(|rest| rest.split(['/', '?']).next())
        .unwrap_or_default();
    if !pass.covers(host) {
        return Err(Missed::Stop(Stop::NotLet, None));
    }
    let ask = web::Ask::get(url, USER_AGENT)
        .most(most)
        .waiting(PATIENCE)
        .coded(web::Coding::Gzip);
    let got = web::request_let(pass, &ask).map_err(|e| missed_of_web(&e))?;
    if let Some(missed) = missed_of_status(got.reply.status) {
        return Err(missed);
    }
    Ok(got.reply.body)
}

/// A video's `mqdefault` picture from [`THUMBS_HOST`], asked under `pass`
/// and stopped by `abort` — fast-access's YouTube results, found by a search
/// rather than a scan, whose pictures this crate asks as it asks its own:
/// it is the one boundary with YouTube over HTTP.
///
/// `None` when nothing came: a pass for another host (nothing is sent), an
/// id that is not one, an abort, or the host not answering with a picture.
/// A picture is a picture or none — the row draws without it.
#[must_use]
pub fn fetch_thumb(id: &VideoId, pass: &Pass, abort: &web::Abort) -> Option<Vec<u8>> {
    let fits = id.0.len() == 11
        && id
            .0
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !pass.covers(THUMBS_HOST) || !fits {
        return None;
    }
    let url = format!("https://{THUMBS_HOST}/vi/{}/mqdefault.jpg", id.0);
    let ask = web::Ask::get(&url, USER_AGENT)
        .most(thumbs::MOST_BYTES)
        .waiting(PATIENCE)
        .abortable(abort);
    let got = web::request_let(pass, &ask).ok()?;
    missed_of_status(got.reply.status)
        .is_none()
        .then_some(got.reply.body)
}

impl Reach for Internet {
    fn feed(&self, which: Which, channel: &ChannelId) -> Result<Vec<u8>, Missed> {
        let pass = pass(FEEDS_HOST)?;
        let path = which
            .path(channel)
            .ok_or_else(|| Missed::Failed("not a channel id".to_owned()))?;
        match self.feeds {
            Origin::YouTube => get(
                &pass,
                &format!("https://{FEEDS_HOST}{path}"),
                feed::MOST_BYTES,
            ),
            Origin::Loopback(address) => {
                let call = ronoctua_http::Call::new("GET", &path, "127.0.0.1").waiting(PATIENCE);
                let answer = ronoctua_http::over_tcp(address, &call)
                    .map_err(|e| Missed::Stop(Stop::Offline, Some(e.to_string())))?;
                if let Some(missed) = missed_of_status(answer.status) {
                    return Err(missed);
                }
                Ok(answer.body.into_bytes())
            }
        }
    }

    fn tab(&self, channel: &ChannelId, tab: Tab, most: usize) -> Result<Vec<TabEntry>, Missed> {
        youtube::tab(&channel.0, tab, most).map_err(missed_of_tool)
    }

    fn details(&self, url: &str) -> Result<Details, Missed> {
        youtube::details(url).map_err(missed_of_tool)
    }

    fn thumb(&self, id: &VideoId) -> Result<Vec<u8>, Missed> {
        // A FIXTURE'S VIDEOS HAVE NO PICTURES (see the module's head), and
        // nothing is asked of anyone: no pass is minted for it.
        if let Origin::Loopback(_) = self.feeds {
            return Err(Missed::NotFound);
        }
        let pass = pass(THUMBS_HOST)?;
        get(
            &pass,
            &format!("https://{THUMBS_HOST}/vi/{}/mqdefault.jpg", id.0),
            thumbs::MOST_BYTES,
        )
    }

    fn pause(&self) {
        std::thread::sleep(self.next_pause());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_loopback_http_origin_replaces_youtube() {
        assert_eq!(origin_of(None).unwrap(), Origin::YouTube);
        assert_eq!(origin_of(Some("  ")).unwrap(), Origin::YouTube);
        assert_eq!(
            origin_of(Some("http://127.0.0.1:8080/")).unwrap(),
            Origin::Loopback("127.0.0.1:8080".parse().unwrap())
        );
        assert!(origin_of(Some("http://[::1]:9")).is_ok());
        for refused in [
            "https://127.0.0.1:8080",
            "http://192.168.1.2:80",
            "http://example.com:80",
            "127.0.0.1:80",
            "http://127.0.0.1",
        ] {
            assert_eq!(
                origin_of(Some(refused)).unwrap_err().why,
                Why::Invalid,
                "{refused}"
            );
        }
    }

    #[test]
    fn statuses_and_troubles_mean_what_a_scan_does() {
        assert_eq!(missed_of_status(200), None);
        assert_eq!(missed_of_status(404), Some(Missed::NotFound));
        assert!(matches!(
            missed_of_status(429),
            Some(Missed::Stop(Stop::TooManyRequests, _))
        ));
        assert!(matches!(missed_of_status(500), Some(Missed::Failed(_))));
        let offline = web::Error::new(web::Kind::Offline, "no route");
        assert!(matches!(
            missed_of_web(&offline),
            Missed::Stop(Stop::Offline, _)
        ));
        let bot = youtube::Failed::from((
            ronoctua_ytdlp::Trouble::Failed,
            Some("ERROR: Sign in to confirm you're not a bot".to_owned()),
        ));
        assert!(matches!(
            missed_of_tool(bot),
            Missed::Stop(Stop::BotCheck, Some(_))
        ));
        let gone = youtube::Failed::from((
            ronoctua_ytdlp::Trouble::Failed,
            Some("ERROR: [youtube] x: Private video".to_owned()),
        ));
        assert_eq!(missed_of_tool(gone), Missed::Gone);
        assert_eq!(
            missed_of_tool(ronoctua_ytdlp::Trouble::Absent.into()),
            Missed::NoTool
        );
        assert_eq!(
            missed_of_tool(ronoctua_ytdlp::Trouble::Slow.into()),
            Missed::Failed("yt-dlp: slow".to_owned())
        );
    }

    /// FAILS IF: a scan against a loopback fixture asks YouTube for its
    /// pictures — a test machine with no way out would stop there.
    #[test]
    fn a_loopback_origin_asks_youtube_for_no_picture() {
        let net = Internet {
            feeds: Origin::Loopback("127.0.0.1:9".parse().unwrap()),
            seed: std::cell::Cell::new(1),
        };
        assert_eq!(
            net.thumb(&VideoId("dQw4w9WgXcQ".to_owned())),
            Err(Missed::NotFound),
            "a loopback origin reached for a picture"
        );
    }

    #[test]
    fn the_pause_is_four_seconds_give_or_take_two() {
        let net = Internet {
            feeds: Origin::YouTube,
            seed: std::cell::Cell::new(12345),
        };
        let pauses: Vec<Duration> = (0..1000).map(|_| net.next_pause()).collect();
        assert!(
            pauses
                .iter()
                .all(|p| (Duration::from_secs(2)..=Duration::from_secs(6)).contains(p))
        );
        let (low, high) = (pauses.iter().min().unwrap(), pauses.iter().max().unwrap());
        assert!(
            *low < Duration::from_millis(2500) && *high > Duration::from_millis(5500),
            "it spreads over the whole range"
        );
    }
}
