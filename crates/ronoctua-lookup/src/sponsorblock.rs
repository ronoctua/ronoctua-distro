//! Skipping what is not the video: SponsorBlock's segments.
//!
//! Moved here from `ronoctua-player`, with [`fetch`]: the one request,
//! through the crate's [`Net`](crate::Net). The player's daemon asks through
//! it when the person switched sponsored segments on.
//!
//! The stream yt-dlp hands mpv already carries no YouTube advertising — that is
//! the site's player, and this is not it. What is left is advertising INSIDE
//! the video: a creator reading a sponsor's text, asking for a subscription.
//! SponsorBlock is a public list of those, by video, and this is the client.
//!
//! # Privacy: the prefix, not the id
//!
//! The request carries the first four hex digits of the video id's SHA-256,
//! and the service answers every video whose hash starts with them; the one
//! wanted is picked here. The service never learns which video is playing —
//! that is its own recommended form, and the reason SHA-256 is written out
//! below rather than skipped.

use crate::ask::{Net, Request};
use crate::{Host, Refusal, Service, Why};

/// A stretch to jump over: `(from, to)` in seconds.
pub type Segment = (f64, f64);

/// The segments to skip in the video `id`, from the categories chosen.
///
/// An empty list is an answer: the service's 404 means it knows no segment of
/// any video with that prefix.
///
/// # Errors
///
/// [`Why::Malformed`] for an answer that is not a list, [`Why::Declined`] for
/// a category the service does not know (its 400), and what the network
/// answers.
pub fn fetch(net: &dyn Net, id: &str, categories: &[String]) -> Result<Vec<Segment>, Refusal> {
    let refused = |why| Refusal::new(Service::SponsorBlock, why);
    let url = format!("{}{}", Host::SponsorBlock.origin(), path(id, categories));
    let reply = net
        .send(&Request::get(Host::SponsorBlock, url))
        .map_err(refused)?;
    if reply.status == 404 {
        return Ok(Vec::new());
    }
    if let Some(why) = crate::ask::why_of_status(Service::SponsorBlock, reply.status) {
        return Err(refused(why));
    }
    let listed =
        serde_json::from_slice::<serde_json::Value>(&reply.body).is_ok_and(|v| v.is_array());
    if !listed {
        return Err(refused(Why::Malformed));
    }
    Ok(parse(&String::from_utf8_lossy(&reply.body), id))
}

/// The video id of a YouTube address, when it is one.
#[must_use]
pub fn video_id(url: &str) -> Option<String> {
    let id = if let Some(rest) = url.split("youtu.be/").nth(1) {
        rest
    } else if let Some(rest) = url
        .split("/shorts/")
        .nth(1)
        .filter(|_| url.contains("youtube."))
    {
        rest
    } else if url.contains("youtube.") {
        url.split(['?', '&']).find_map(|p| p.strip_prefix("v="))?
    } else {
        return None;
    };
    let id: String = id
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    (id.len() == 11).then_some(id)
}

/// The request's path: the hash prefix and the categories.
#[must_use]
pub fn path(id: &str, categories: &[String]) -> String {
    let hash = sha256_hex(id.as_bytes());
    let list: Vec<String> = categories.iter().map(|c| format!("%22{c}%22")).collect();
    format!(
        "/api/skipSegments/{}?categories=%5B{}%5D",
        &hash[..4],
        list.join(",")
    )
}

/// The segments of `id` in an answer that lists every video with the prefix.
#[must_use]
pub fn parse(body: &str, id: &str) -> Vec<Segment> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let Some(videos) = v.as_array() else {
        return Vec::new();
    };
    let mut out: Vec<Segment> = videos
        .iter()
        .filter(|video| video.get("videoID").and_then(serde_json::Value::as_str) == Some(id))
        .filter_map(|video| video.get("segments")?.as_array())
        .flatten()
        .filter(|s| {
            // A MUTE segment is for a player that can mute a stretch; skipping
            // it would cut out the video around the part meant to be silent.
            s.get("actionType").and_then(serde_json::Value::as_str) != Some("mute")
        })
        .filter_map(|s| {
            let pair = s.get("segment")?.as_array()?;
            let from = pair.first()?.as_f64()?;
            let to = pair.get(1)?.as_f64()?;
            (to > from).then_some((from, to))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// Where to jump when the playhead is at `position`: the end of the segment
/// it is inside, when it is inside one. A segment the person seeked INTO on
/// purpose is not skipped twice — `done` remembers the ones already jumped.
#[must_use]
pub fn skip(position: f64, segments: &[Segment], done: &mut Vec<usize>) -> Option<f64> {
    let (i, s) = segments
        .iter()
        .enumerate()
        .find(|(_, s)| position >= s.0 && position < s.1 - 0.5)?;
    if done.contains(&i) {
        return None;
    }
    done.push(i);
    Some(s.1)
}

/// SHA-256, hex. Written out: one use, and it is the privacy of every request.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_meets_its_published_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn every_spelling_of_a_youtube_address_gives_its_id() {
        for url in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/watch?list=PL1&v=dQw4w9WgXcQ&t=3",
            "https://youtu.be/dQw4w9WgXcQ?si=x",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ",
        ] {
            assert_eq!(video_id(url).as_deref(), Some("dQw4w9WgXcQ"), "{url}");
        }
        assert_eq!(video_id("https://vimeo.com/123"), None);
        assert_eq!(video_id("/music/a.flac"), None);
    }

    #[test]
    fn the_request_carries_the_prefix_and_never_the_id() {
        let p = path("dQw4w9WgXcQ", &["sponsor".into(), "selfpromo".into()]);
        assert!(!p.contains("dQw4w9WgXcQ"), "{p}");
        assert!(p.starts_with(&format!(
            "/api/skipSegments/{}",
            &sha256_hex(b"dQw4w9WgXcQ")[..4]
        )));
        assert!(
            p.ends_with("categories=%5B%22sponsor%22,%22selfpromo%22%5D"),
            "{p}"
        );
    }

    #[test]
    fn only_this_videos_segments_and_never_a_mute_one() {
        let body = r#"[
          {"videoID":"other000000","segments":[{"segment":[1,2],"actionType":"skip"}]},
          {"videoID":"dQw4w9WgXcQ","segments":[
            {"segment":[30.5,45.0],"actionType":"skip"},
            {"segment":[5.0,9.0],"actionType":"skip"},
            {"segment":[50,60],"actionType":"mute"}]}
        ]"#;
        assert_eq!(parse(body, "dQw4w9WgXcQ"), vec![(5.0, 9.0), (30.5, 45.0)]);
        assert!(parse("[]", "dQw4w9WgXcQ").is_empty());
    }

    #[test]
    fn a_video_nobody_marked_has_no_segments_and_a_list_is_asked_for() {
        use crate::canned::Canned;
        let id = "dQw4w9WgXcQ";
        let cats = ["sponsor".to_owned()];
        let url = format!("https://sponsor.ajay.app{}", path(id, &cats));
        let net = Canned::new(&[(url.as_str(), 404, b"Not Found")]);
        assert_eq!(fetch(&net, id, &cats), Ok(Vec::new()));
        let body =
            br#"[{"videoID":"dQw4w9WgXcQ","segments":[{"segment":[1,2],"actionType":"skip"}]}]"#;
        let net = Canned::new(&[(url.as_str(), 200, body)]);
        assert_eq!(fetch(&net, id, &cats), Ok(vec![(1.0, 2.0)]));
        let net = Canned::new(&[(url.as_str(), 200, b"{}")]);
        assert_eq!(
            fetch(&net, id, &cats).unwrap_err().why,
            Why::Malformed,
            "an answer that is not a list is not 'no segments'"
        );
        let asked = net.only();
        assert!(
            !asked.url.contains(id),
            "the id never leaves: {}",
            asked.url
        );
    }

    #[test]
    fn a_segment_is_jumped_once() {
        let s = vec![(5.0, 9.0)];
        let mut done = Vec::new();
        assert_eq!(skip(4.0, &s, &mut done), None);
        assert_eq!(skip(5.2, &s, &mut done), Some(9.0));
        assert_eq!(skip(6.0, &s, &mut done), None, "seeked back into it: stays");
    }
}
