//! Playing on two machines: the sockets. The rules are
//! `ronoctua_player::remote`, and its header says why each is what it is.
//!
//! Four threads' worth of work, and none holds the engine's lock across the
//! network:
//!
//! - the LISTENER, on this machine's tailnet addresses only, re-reading the
//!   tailnet every half minute (it can come up after the player does);
//! - one thread per CONNECTION: a knock (JSON, one line) or a `GET` of a file;
//! - PAIRING with another machine, from a person's order;
//! - FOLLOWING a machine that was joined, four times a second.

use std::{
    io::{BufRead as _, BufReader, Read as _, Seek as _, Write as _},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use ronoctua_player::{
    remote::{self, Answer, Knock, Moment, Peer, Role},
    sync::{self, Correction},
};

use crate::engine::Shared;

/// How long a knock waits for its answer.
const PATIENCE: Duration = Duration::from_secs(5);

/// How long after a load the follower SEEKS instead of nudging.
///
/// Loading over the tailnet takes a moment the start position cannot know —
/// measured, 0.7 s on a local file — and a 5% nudge pays that back at 50 ms a
/// second: fourteen seconds out of step. Right after a load, anything past
/// [`SETTLE_MS`] is a seek; after that, the nudges of `sync::correct`.
const SETTLING: Duration = Duration::from_secs(4);

/// What counts as out of step while settling.
const SETTLE_MS: i64 = 120;

/// How often the tailnet is read again.
const LOOK_EVERY: Duration = Duration::from_secs(30);

/// What the engine keeps of the tailnet.
#[derive(Debug, Clone, Default)]
pub struct Tailnet {
    /// This machine's name on it.
    pub name: String,
    /// This machine's addresses on it.
    pub addresses: Vec<IpAddr>,
    /// The machines online on it: name and addresses.
    pub peers: Vec<(String, Vec<IpAddr>)>,
}

/// A machine being followed.
pub struct Following {
    /// Its name.
    pub host: String,
    /// What this machine plays of it.
    pub role: Role,
    /// How far behind or ahead, in ms.
    pub drift_ms: i64,
    /// Raised to stop the thread.
    pub stop: Arc<AtomicBool>,
}

/// The tailnet, when this machine is on one.
fn look() -> Option<Tailnet> {
    let scene =
        ronoctua_tunnel::tailscale::look(std::path::Path::new(ronoctua_tunnel::tailscale::SOCKET))?;
    if !scene.running {
        return None;
    }
    let ips = |v: &[String]| -> Vec<IpAddr> { v.iter().filter_map(|a| a.parse().ok()).collect() };
    Some(Tailnet {
        name: scene
            .name
            .split('.')
            .next()
            .unwrap_or(&scene.name)
            .to_owned(),
        addresses: ips(&scene.addresses),
        peers: scene
            .peers
            .iter()
            .filter(|p| p.online)
            .map(|p| {
                (
                    p.name.split('.').next().unwrap_or(&p.name).to_owned(),
                    ips(&p.addresses),
                )
            })
            .collect(),
    })
}

/// Starts the listener: binds this machine's tailnet addresses as they
/// appear, and keeps the engine's picture of the tailnet current.
pub fn listen(shared: Shared) {
    let _ = std::thread::Builder::new()
        .name("player-remote".into())
        .spawn(move || {
            let mut bound: Vec<IpAddr> = Vec::new();
            loop {
                let seen = look();
                if let Some(t) = &seen {
                    for ip in &t.addresses {
                        if bound.contains(ip) {
                            continue;
                        }
                        match TcpListener::bind(SocketAddr::new(*ip, remote::port())) {
                            Ok(l) => {
                                tracing::info!(address = %ip, "listening on the tailnet");
                                bound.push(*ip);
                                let shared = shared.clone();
                                let _ = std::thread::Builder::new()
                                    .name("player-accept".into())
                                    .spawn(move || accept(&l, &shared));
                            }
                            Err(e) => tracing::info!(address = %ip, error = %e, "not yet"),
                        }
                    }
                }
                let (lock, _) = &*shared;
                if let Ok(mut e) = lock.lock() {
                    e.set_tailnet(seen);
                    e.broadcast();
                }
                std::thread::sleep(LOOK_EVERY);
            }
        });
}

fn accept(listener: &TcpListener, shared: &Shared) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        // THE TAILNET AND NOTHING ELSE, even on an address that is the
        // tailnet's: a route somebody added is not a reason to trust it.
        if !stream
            .peer_addr()
            .is_ok_and(|a| remote::on_the_tailnet(a.ip()))
        {
            continue;
        }
        let shared = shared.clone();
        let _ = std::thread::Builder::new()
            .name("player-knock".into())
            .spawn(move || {
                if let Err(e) = serve(stream, &shared) {
                    tracing::debug!(error = %e, "a knock went nowhere");
                }
            });
    }
}

/// One connection: a `GET` of a file, or one knock and its answer.
fn serve(stream: TcpStream, shared: &Shared) -> Result<()> {
    stream.set_read_timeout(Some(PATIENCE))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut first = String::new();
    reader.read_line(&mut first)?;
    if first.starts_with("GET ") {
        return media(stream, &first, &mut reader, shared);
    }
    let knock: Knock = serde_json::from_str(first.trim_end()).context("not a knock")?;
    let answer = answer(knock, shared);
    let mut out = stream;
    let mut line = serde_json::to_vec(&answer)?;
    line.push(b'\n');
    out.write_all(&line)?;
    Ok(())
}

/// What a knock gets.
fn answer(knock: Knock, shared: &Shared) -> Answer {
    let peers = remote::peers().read();
    let (lock, _) = &**shared;
    let Ok(mut e) = lock.lock() else {
        return Answer::Refused;
    };
    match knock {
        Knock::Pair { code, name } => {
            let Some(key) = e.take_invite(&code) else {
                return Answer::Refused;
            };
            let mut p = peers;
            p.guests.retain(|g| g.name != name);
            p.guests.push(Peer {
                name,
                address: None,
                key: key.clone(),
            });
            if let Err(err) = remote::keep(&p) {
                tracing::warn!(error = %err, "the pairing could not be kept");
                return Answer::Refused;
            }
            e.broadcast();
            Answer::Paired {
                key,
                name: e.machine_name(),
            }
        }
        Knock::Time { key } if peers.admits(&key) => Answer::Time {
            ms: remote::clock_ms(),
        },
        Knock::Now { key } if peers.admits(&key) => Answer::Now(e.moment()),
        _ => Answer::Refused,
    }
}

/// `GET /media/<n>`: a file of the queue, to a guest, with `Range`.
fn media(
    mut stream: TcpStream,
    first: &str,
    reader: &mut BufReader<TcpStream>,
    shared: &Shared,
) -> Result<()> {
    let mut range = None;
    let mut key = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line.trim().is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "range" => range = Some(value.trim().to_owned()),
            "authorization" => {
                key = value
                    .trim()
                    .strip_prefix("Bearer ")
                    .unwrap_or_default()
                    .to_owned();
            }
            _ => {}
        }
    }
    let refuse = |s: &mut TcpStream, status: &str| {
        let _ = s.write_all(ronoctua_http::response_head(status, &[], 0).as_bytes());
    };
    if !remote::peers().read().admits(&key) {
        refuse(&mut stream, "403 Forbidden");
        return Ok(());
    }
    let index: Option<usize> = first
        .split_whitespace()
        .nth(1)
        .and_then(|p| p.strip_prefix("/media/"))
        .and_then(|n| n.split('/').next())
        .and_then(|n| n.parse().ok());
    let location = {
        let (lock, _) = &**shared;
        let e = lock
            .lock()
            .map_err(|_| anyhow::anyhow!("the engine is gone"))?;
        index.and_then(|i| e.location_of(i))
    };
    // A FILE OF THE QUEUE, and never an address: an address is handed over as
    // itself, and this is not a proxy.
    let Some(path) = location.filter(|l| !l.contains("://")) else {
        refuse(&mut stream, "404 Not Found");
        return Ok(());
    };
    let mut file = std::fs::File::open(&path)?;
    let len = file.metadata()?.len();
    let (status, from, to) = match remote::range_of(range.as_deref(), len) {
        Some((a, b)) => ("206 Partial Content", a, b),
        None => ("200 OK", 0, len.saturating_sub(1)),
    };
    let count = if len == 0 { 0 } else { to - from + 1 };
    let range_header = format!("bytes {from}-{to}/{len}");
    let mut headers = vec![
        ("Content-Type", "application/octet-stream"),
        ("Accept-Ranges", "bytes"),
    ];
    if status.starts_with("206") {
        headers.push(("Content-Range", range_header.as_str()));
    }
    stream.write_all(ronoctua_http::response_head(status, &headers, count).as_bytes())?;
    file.seek(std::io::SeekFrom::Start(from))?;
    // A PEER THAT CLOSES MID-FILE is mpv seeking: not a fault.
    let _ = std::io::copy(&mut file.take(count), &mut stream);
    Ok(())
}

/// One knock and its answer, to `address`.
fn exchange(address: IpAddr, knock: &Knock) -> Result<Answer> {
    let mut s = TcpStream::connect_timeout(&SocketAddr::new(address, remote::port()), PATIENCE)
        .with_context(|| format!("{address} did not answer"))?;
    s.set_read_timeout(Some(PATIENCE))?;
    let mut line = serde_json::to_vec(knock)?;
    line.push(b'\n');
    s.write_all(&line)?;
    let mut back = String::new();
    BufReader::new(s).read_line(&mut back)?;
    Ok(serde_json::from_str(back.trim_end())?)
}

/// Where `to` is: an address as written, or a machine's name on the tailnet.
fn resolve(to: &str, tailnet: Option<&Tailnet>) -> Option<IpAddr> {
    if let Ok(ip) = to.trim().parse::<IpAddr>() {
        return Some(ip);
    }
    let want = to
        .trim()
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let (_, ips) = tailnet?
        .peers
        .iter()
        .find(|(n, _)| n.to_ascii_lowercase() == want)?;
    // IPv4 FIRST: an IPv6 address in a URL needs brackets, and one family is
    // enough.
    ips.iter()
        .find(|i| i.is_ipv4())
        .or_else(|| ips.first())
        .copied()
}

/// Pairs with another machine: its code, read aloud, for a key kept here.
///
/// # Errors
///
/// Nobody by that name, no answer, a refused code, or the file.
pub fn pair_with(to: &str, code: &str, shared: &Shared) -> Result<String> {
    let (tailnet, me) = {
        let (lock, _) = &**shared;
        let e = lock
            .lock()
            .map_err(|_| anyhow::anyhow!("the engine is gone"))?;
        (e.tailnet().cloned(), e.machine_name())
    };
    let Some(address) = resolve(to, tailnet.as_ref()) else {
        bail!("no machine called {to} is online on the tailnet");
    };
    let answer = exchange(
        address,
        &Knock::Pair {
            code: code.trim().to_owned(),
            name: me,
        },
    )?;
    let Answer::Paired { key, name } = answer else {
        bail!("{to} did not take the code");
    };
    let mut p = remote::peers().read();
    p.hosts.retain(|h| h.name != name);
    p.hosts.push(Peer {
        name: name.clone(),
        address: Some(address.to_string()),
        key,
    });
    remote::keep(&p)?;
    Ok(name)
}

/// Starts following `host` as `role`. The thread measures the clocks' offset
/// once and then corrects the local player four times a second.
pub fn follow(host: Peer, role: Role, shared: Shared) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let halt = stop.clone();
    let _ = std::thread::Builder::new()
        .name("player-follow-remote".into())
        .spawn(move || {
            if let Err(e) = follow_loop(&host, role, &shared, &halt) {
                tracing::warn!(host = %host.name, error = %e, "following ended");
                let (lock, bell) = &*shared;
                if let Ok(mut eng) = lock.lock() {
                    eng.left(&format!("{e:#}"));
                    eng.broadcast();
                }
                bell.notify_all();
            }
        });
    stop
}

fn follow_loop(host: &Peer, role: Role, shared: &Shared, stop: &AtomicBool) -> Result<()> {
    let address: IpAddr = host
        .address
        .as_deref()
        .and_then(|a| a.parse().ok())
        .context("the paired machine has no address")?;
    // THE CLOCKS, once: eight round trips and the median.
    let mut samples = Vec::with_capacity(remote::CLOCK_SAMPLES);
    for _ in 0..remote::CLOCK_SAMPLES {
        let sent = remote::clock_ms();
        let Answer::Time { ms } = exchange(
            address,
            &Knock::Time {
                key: host.key.clone(),
            },
        )?
        else {
            bail!("{} refused the key", host.name);
        };
        samples.push((sent, ms, remote::clock_ms()));
    }
    let offset = sync::offset(&samples).unwrap_or(0.0);
    tracing::info!(host = %host.name, offset_ms = offset, "the clocks are measured");

    let mut loaded: Option<(Option<usize>, Option<String>)> = None;
    let mut settling: Option<Instant> = None;
    let mut fetched: Option<PathBuf> = None;
    let outcome = (|| -> Result<()> {
        while !stop.load(Ordering::SeqCst) {
            let started = Instant::now();
            let Answer::Now(m) = exchange(
                address,
                &Knock::Now {
                    key: host.key.clone(),
                },
            )?
            else {
                bail!("{} refused the key", host.name);
            };
            // A FORMAT ONLY OUR DECODERS READ comes over whole, into the cache,
            // BEFORE the engine's lock is taken: the decoders want a path, and
            // a stream of bytes from another machine is not one.
            let changed = loaded.as_ref() != Some(&(m.item, m.address.clone()));
            if changed && let Some(f) = fetched.take() {
                let _ = std::fs::remove_file(f);
            }
            if changed
                && m.address.is_none()
                && let (Some(item), Some(name)) = (m.item, m.name.as_deref())
                && ronoctua_player::decoders::for_path(Path::new(name)).is_some()
            {
                fetched = Some(fetch(address, host, item, name)?);
            }
            step(
                &m,
                role,
                address,
                (host, offset, fetched.as_deref()),
                (&mut loaded, &mut settling),
                shared,
            )?;
            std::thread::sleep(sync::TICK.saturating_sub(started.elapsed()));
        }
        Ok(())
    })();
    if let Some(f) = fetched {
        let _ = std::fs::remove_file(f);
    }
    outcome
}

/// The master's file `item`, whole, into this machine's cache — named after
/// the original so its extension picks the decoder.
fn fetch(address: IpAddr, host: &Peer, item: usize, name: &str) -> Result<PathBuf> {
    let dir = ronoctua_xdg::cache_dir().context("no cache directory")?;
    std::fs::create_dir_all(&dir)?;
    // THE NAME'S EXTENSION, and nothing else of it: a name from another
    // machine is not a path to trust.
    let ext = Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let to = dir.join(format!("player-remote-{}.{ext}", std::process::id()));
    let mut s = TcpStream::connect_timeout(&SocketAddr::new(address, remote::port()), PATIENCE)
        .with_context(|| format!("{address} did not answer"))?;
    s.set_read_timeout(Some(PATIENCE))?;
    let bearer = format!("Bearer {}", host.key);
    let path = format!("/media/{item}");
    let host_name = address.to_string();
    let headers = [("Authorization", bearer.as_str())];
    let call = ronoctua_http::Call::new("GET", &path, &host_name).with_headers(&headers);
    s.write_all(ronoctua_http::request_of(&call).as_bytes())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw)?;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .context("no answer")?;
    let head = String::from_utf8_lossy(&raw[..split]);
    if ronoctua_http::status_of(&head) != Some(200) {
        bail!("{} would not hand over {name}", host.name);
    }
    std::fs::write(&to, &raw[split + 4..])?;
    Ok(to)
}

/// One tick: load what the master plays when it changed, else correct.
#[allow(clippy::type_complexity)]
fn step(
    m: &Moment,
    role: Role,
    address: IpAddr,
    (host, offset, fetched): (&Peer, f64, Option<&Path>),
    (loaded, settling): (
        &mut Option<(Option<usize>, Option<String>)>,
        &mut Option<Instant>,
    ),
    shared: &Shared,
) -> Result<()> {
    let (lock, _) = &**shared;
    let mut e = lock
        .lock()
        .map_err(|_| anyhow::anyhow!("the engine is gone"))?;
    let mpv = e.main(shared)?;
    let now = (m.item, m.address.clone());
    if m.item.is_none() {
        return Ok(());
    }
    if loaded.as_ref() != Some(&now) {
        let url = fetched
            .map(|p| crate::engine::loadable(&p.to_string_lossy()))
            .or_else(|| m.address.clone())
            .unwrap_or_else(|| {
                let host_part = match address {
                    IpAddr::V4(v4) => v4.to_string(),
                    IpAddr::V6(v6) => format!("[{v6}]"),
                };
                format!(
                    "http://{host_part}:{}/media/{}",
                    remote::port(),
                    m.item.unwrap_or(0)
                )
            });
        // THE KEY TRAVELS AS A HEADER, and only to the master's own files.
        let header = if m.address.is_none() && fetched.is_none() {
            format!("Authorization: Bearer {}", host.key)
        } else {
            String::new()
        };
        let _ = mpv.set("http-header-fields", &header);
        // THE ROLE: a screen drops the sound, a speaker drops the picture.
        let _ = mpv.set("aid", if role == Role::Screen { "no" } else { "auto" });
        let _ = mpv.set("vid", if role == Role::Sound { "no" } else { "auto" });
        let at = remote::master_now(m, remote::clock_ms(), offset);
        let _ = mpv.set("start", &format!("{at:.3}"));
        mpv.command(&["loadfile", &url, "replace"])?;
        let _ = mpv.set("pause", if m.playing { "no" } else { "yes" });
        *loaded = Some(now);
        *settling = Some(Instant::now());
        return Ok(());
    }
    let _ = mpv.set("pause", if m.playing { "no" } else { "yes" });
    let Some(here) = mpv.float("time-pos") else {
        return Ok(());
    };
    let there = remote::master_now(m, remote::clock_ms(), offset);
    #[allow(clippy::cast_possible_truncation)]
    let drift = ((here - there) * 1000.0).round() as i64;
    e.drifted(drift);
    if settling.is_some_and(|s| s.elapsed() < SETTLING) {
        if drift.abs() > SETTLE_MS {
            // AHEAD OF WHERE IT WILL BE: the seek itself takes a moment.
            let to = there + f64::from(u16::try_from(SETTLE_MS / 2).unwrap_or(0)) / 1000.0;
            let _ = mpv.command(&["seek", &format!("{to:.3}"), "absolute+exact"]);
        }
        return Ok(());
    }
    *settling = None;
    match sync::correct(there, here, m.speed) {
        Correction::Keep => {
            let _ = mpv.set("speed", &m.speed.to_string());
        }
        Correction::Speed(s) => {
            let _ = mpv.set("speed", &s.to_string());
        }
        Correction::Seek(to) => {
            let _ = mpv.command(&["seek", &format!("{to:.3}"), "absolute+exact"]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_found_on_the_tailnet_and_an_address_is_itself() {
        let t = Tailnet {
            name: "here".into(),
            addresses: vec![],
            peers: vec![(
                "Den".into(),
                vec![
                    "fd7a:115c:a1e0::5".parse().expect("v6"),
                    "100.64.0.5".parse().expect("v4"),
                ],
            )],
        };
        assert_eq!(
            resolve("den", Some(&t)),
            Some("100.64.0.5".parse().expect("v4")),
            "IPv4 first"
        );
        assert_eq!(
            resolve("den.tail1234.ts.net", Some(&t)),
            Some("100.64.0.5".parse().expect("v4"))
        );
        assert_eq!(
            resolve("100.64.0.9", None),
            Some("100.64.0.9".parse().expect("v4"))
        );
        assert_eq!(resolve("kitchen", Some(&t)), None);
    }
}
