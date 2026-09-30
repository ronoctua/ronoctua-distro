//! The addresses handed to yt-dlp: checked, escaped, and a YouTube list
//! written the way yt-dlp lists it.

/// The parts of an `http(s)` address this module looks at.
struct Parts<'a> {
    host: &'a str,
    path: &'a str,
    query: &'a str,
}

/// An `http` or `https` address, split; `None` for anything else.
fn parts(url: &str) -> Option<Parts<'_>> {
    let rest = strip_scheme(url)?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    // A USER NAME IN THE AUTHORITY is how `https://youtube.com@elsewhere/`
    // points somewhere else while reading as YouTube.
    if authority.contains('@') {
        return None;
    }
    let host = authority.split(':').next().unwrap_or("");
    let rest = &rest[end..];
    let rest = rest.split('#').next().unwrap_or("");
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    Some(Parts { host, path, query })
}

fn strip_scheme(url: &str) -> Option<&str> {
    for scheme in ["https://", "http://"] {
        // `get`, never an index: a first character of two bytes would put the
        // cut inside it, and slicing there panics.
        if url
            .get(..scheme.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
        {
            return url.get(scheme.len()..);
        }
    }
    None
}

/// Whether `url` is an address worth handing to yt-dlp: `http` or `https`,
/// with a host, and no whitespace or control character (an argument with a
/// line break in it is two lines of somebody's log).
#[must_use]
pub fn is_address(url: &str) -> bool {
    !url.is_empty()
        && url.len() <= 4096
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
        && parts(url).is_some_and(|p| !p.host.is_empty())
}

/// The hosts whose watch pages carry a `list=` this module rewrites.
const YOUTUBE: [&str; 4] = [
    "youtube.com",
    "www.youtube.com",
    "m.youtube.com",
    "music.youtube.com",
];

/// The value of `name` in a query, undecoded.
fn query_value<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

/// A list id as YouTube writes it: letters, digits, `-` and `_`.
fn is_list_id(id: &str) -> bool {
    (2..=128).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Whether a list id names a MIX: a list YouTube makes up around one video,
/// which exists only beside that video.
///
/// `RD…` is a mix and `UL…` an uploads mix; asked as `/playlist?list=RD…`
/// they answer nothing, so their watch address is kept as it is.
fn is_mix(id: &str) -> bool {
    id.starts_with("RD") || id.starts_with("UL")
}

/// The address of the LIST behind `url`, the way yt-dlp lists it; `url`
/// itself when it is not a YouTube watch page with a list. Pure.
///
/// # Why
///
/// yt-dlp answers `watch?v=…&list=…` with the ONE video (measured on the
/// pinned 2026.07.04), which a person who pasted a list did not mean. The
/// list's own page, `/playlist?list=…`, answers every entry. A mix is the
/// exception (see [`is_mix`]) and is asked with `--yes-playlist` instead.
#[must_use]
pub fn listing_address(url: &str) -> String {
    let Some(p) = parts(url) else {
        return url.to_owned();
    };
    let host = p.host.to_ascii_lowercase();
    let watch = (YOUTUBE.contains(&host.as_str()) && p.path == "/watch")
        || (host == "youtu.be" && p.path.len() > 1);
    if !watch {
        return url.to_owned();
    }
    match query_value(p.query, "list") {
        Some(id) if is_list_id(id) && !is_mix(id) => {
            format!("https://www.youtube.com/playlist?list={id}")
        }
        _ => url.to_owned(),
    }
}

/// Percent-escapes `text` for a query value: everything but the unreserved
/// characters of RFC 3986, byte by byte of its UTF-8. Pure.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_watch_page_with_a_list_is_asked_as_the_list() {
        assert_eq!(
            listing_address("https://www.youtube.com/watch?v=abc123&list=PLx_y-z9&index=3"),
            "https://www.youtube.com/playlist?list=PLx_y-z9"
        );
        assert_eq!(
            listing_address("https://youtu.be/abc123?list=PLq1"),
            "https://www.youtube.com/playlist?list=PLq1"
        );
        assert_eq!(
            listing_address("HTTPS://music.youtube.com/watch?list=OLAK5uy_k&v=zz"),
            "https://www.youtube.com/playlist?list=OLAK5uy_k"
        );
    }

    #[test]
    fn a_mix_a_single_video_and_other_hosts_are_left_alone() {
        let mix = "https://www.youtube.com/watch?v=abc&list=RDabc";
        assert_eq!(listing_address(mix), mix);
        let one = "https://www.youtube.com/watch?v=abc";
        assert_eq!(listing_address(one), one);
        let list = "https://www.youtube.com/playlist?list=PLq";
        assert_eq!(listing_address(list), list);
        let elsewhere = "https://example.com/watch?list=PLq";
        assert_eq!(listing_address(elsewhere), elsewhere);
        let hidden = "https://www.youtube.com@example.com/watch?list=PLq";
        assert_eq!(listing_address(hidden), hidden, "a user name hides a host");
        let odd = "https://www.youtube.com/watch?v=a&list=PL%2Fx";
        assert_eq!(listing_address(odd), odd, "a list id is letters and digits");
    }

    #[test]
    fn only_a_plain_web_address_is_handed_over() {
        assert!(is_address("https://www.youtube.com/playlist?list=PLq"));
        assert!(is_address("http://example.com/a"));
        assert!(!is_address("file:///etc/passwd"));
        assert!(!is_address("--exec=rm"));
        assert!(!is_address("https://a b/"));
        assert!(!is_address("https://x/\nnext"));
        assert!(!is_address("https:///nohost"));
        assert!(!is_address(""));
    }

    #[test]
    fn words_are_escaped_byte_by_byte() {
        assert_eq!(escape("lo-fi radio 24/7"), "lo-fi%20radio%2024%2F7");
        assert_eq!(escape("café&x=1"), "caf%C3%A9%26x%3D1");
    }
}
