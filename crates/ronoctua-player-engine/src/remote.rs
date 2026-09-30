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
//! - FOLLOWING a machine that was joined, four times a second: its moment
//!   read off the lock, and the lanes this machine's role plays handed to
//!   the engine, which plays them in its voices on that machine's clock
//!   (`engine/session.rs`). This machine's queue is never touched.

use std::{
    collections::HashMap,
    io::{BufRead as _, BufReader, Read as _, Seek as _, Write as _},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context as _, Result, bail};
use ronoctua_player::{
    queue::EntryId,
    remote::{self, Answer, Knock, Peer, RemoteSource, Role},
    sync,
};

use crate::engine::Shared;

/// How long a knock waits for its answer.
const PATIENCE: Duration = Duration::from_secs(5);

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
    /// How far behind or ahead, in ms: the voice furthest from it.
    pub drift_ms: i64,
    /// Raised to stop the thread.
    pub stop: Arc<AtomicBool>,
    /// Where this machine's own queue was — its leaf and the place in it —
    /// to go back to when it leaves.
    pub resume: (Option<EntryId>, f64),
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

/// `GET /media/<leaf>/<lane>`: a lane's file of the leaf that plays, to a
/// guest, with `Range`.
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
    // THE LEAF BY ITS ENTRY ID, which a reordered queue does not change under
    // a guest that is reading it, and the lane by its id in the set.
    let asked = first.split_whitespace().nth(1).and_then(remote::media_of);
    let location = {
        let (lock, _) = &**shared;
        let e = lock
            .lock()
            .map_err(|_| anyhow::anyhow!("the engine is gone"))?;
        asked.and_then(|(leaf, lane)| e.media_location(leaf, lane))
    };
    // A FILE OF WHAT PLAYS, and never an address: an address is handed over
    // as itself, and this is not a proxy.
    let Some(path) = location else {
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
                    eng.left(&halt, &format!("{e:#}"));
                    eng.broadcast();
                }
                bell.notify_all();
            }
        });
    stop
}

fn follow_loop(host: &Peer, role: Role, shared: &Shared, stop: &Arc<AtomicBool>) -> Result<()> {
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

    let host_part = match address {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => format!("[{v6}]"),
    };
    let base = format!("http://{host_part}:{}", remote::port());
    // THE KEY TRAVELS AS A HEADER, and only to the master's own files.
    let header = format!("Authorization: Bearer {}", host.key);
    // A FORMAT ONLY OUR DECODERS READ comes over whole, once per file of the
    // leaf, and goes when the leaf changes.
    let mut fetched: HashMap<String, PathBuf> = HashMap::new();
    let mut leaf: Option<u64> = None;
    let outcome = (|| -> Result<()> {
        while !stop.load(Ordering::SeqCst) {
            let started = std::time::Instant::now();
            let Answer::Now(m) = exchange(
                address,
                &Knock::Now {
                    key: host.key.clone(),
                },
            )?
            else {
                bail!("{} refused the key", host.name);
            };
            if m.item != leaf {
                for (_, f) in fetched.drain() {
                    let _ = std::fs::remove_file(f);
                }
                leaf = m.item;
            }
            // FETCHED BEFORE the engine's lock is taken: the decoders want
            // a path, and a stream of bytes from another machine is not one.
            for lane in m.lanes.iter().filter(|l| role.plays(l.kind)) {
                // AND A SUBTITLE FILE, which is read here to be merged with
                // the others over its picture — a few kilobytes, once.
                let words = lane.kind == ronoctua_player::own::LaneKind::Subtitle
                    && Path::new(name_of(&lane.source))
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(ronoctua_player::subtitles::claims);
                if let RemoteSource::Media { path, name } = &lane.source
                    && !fetched.contains_key(path)
                    && (words || ronoctua_player::decoders::for_path(Path::new(name)).is_some())
                {
                    let n = fetched.len();
                    fetched.insert(path.clone(), fetch(address, host, path, name, n)?);
                }
            }
            let mut keyed = Vec::new();
            let lanes = remote::joined_lanes(role, &m.lanes, |source| match source {
                RemoteSource::Address { url } => url.clone(),
                RemoteSource::Media { path, .. } => match fetched.get(path) {
                    Some(copy) => crate::engine::loadable(&copy.to_string_lossy()),
                    None => format!("{base}{path}"),
                },
            });
            for lane in &m.lanes {
                if let RemoteSource::Media { path, .. } = &lane.source
                    && !fetched.contains_key(path)
                {
                    keyed.push(format!("{base}{path}"));
                }
            }
            if m.item.is_some() {
                let (lock, bell) = &**shared;
                let mut e = lock
                    .lock()
                    .map_err(|_| anyhow::anyhow!("the engine is gone"))?;
                if let Err(crate::engine::Refused(t)) =
                    e.follow_step(stop, m, lanes, (offset, keyed, header.clone()))
                {
                    bail!("{t:?}");
                }
                e.broadcast();
                drop(e);
                bell.notify_all();
            }
            std::thread::sleep(sync::TICK.saturating_sub(started.elapsed()));
        }
        Ok(())
    })();
    for (_, f) in fetched {
        let _ = std::fs::remove_file(f);
    }
    outcome
}

/// The name of the file a lane of the other machine's comes from; nothing
/// for an address, which is handed over as itself.
fn name_of(source: &RemoteSource) -> &str {
    match source {
        RemoteSource::Media { name, .. } => name,
        RemoteSource::Address { .. } => "",
    }
}

/// The master's file at `path`, whole, into this machine's cache — named
/// after the original so its extension picks the decoder; `n` tells two of
/// one leaf apart.
fn fetch(address: IpAddr, host: &Peer, path: &str, name: &str, n: usize) -> Result<PathBuf> {
    let dir = ronoctua_xdg::cache_dir().context("no cache directory")?;
    std::fs::create_dir_all(&dir)?;
    // THE NAME'S EXTENSION, and nothing else of it: a name from another
    // machine is not a path to trust.
    let ext = Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let to = dir.join(format!("player-remote-{}-{n}.{ext}", std::process::id()));
    let mut s = TcpStream::connect_timeout(&SocketAddr::new(address, remote::port()), PATIENCE)
        .with_context(|| format!("{address} did not answer"))?;
    s.set_read_timeout(Some(PATIENCE))?;
    let bearer = format!("Bearer {}", host.key);
    let host_name = address.to_string();
    let headers = [("Authorization", bearer.as_str())];
    let call = ronoctua_http::Call::new("GET", path, &host_name).with_headers(&headers);
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
