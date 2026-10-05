use super::*;

/// FAILS IF: a pasted link is taken for the wrong kind of source, or one
/// that is none is taken for one instead of refused.
///
/// The first bytes are HAND-WRITTEN in each format's own opening: an IPTV
/// provider's `#EXTM3U` with its `x-tvg-url`, a podcast's RSS with its
/// declaration and a comment, an Atom feed, RSS 1.0's `rdf:RDF`, an HLS
/// index, a web page.
#[test]
fn a_link_is_told_by_its_address_or_its_first_bytes() {
    let m3u = b"\xef\xbb\xbf#EXTM3U x-tvg-url=\"https://epg.example/guide.xml\"\n#EXTINF:-1 group-title=\"News\",A\nhttps://a.example/a.m3u8\n";
    assert_eq!(
        sniff(
            "https://iptv.example/get.php?username=a&password=b&type=m3u_plus",
            m3u
        ),
        Ok(Sniffed::Playlist),
        "a BOM and all"
    );
    let rss = b"<?xml version=\"1.0\"?>\n<!-- a podcast -->\n<rss version=\"2.0\" xmlns:itunes=\"x\"><channel>";
    assert_eq!(
        sniff("https://feeds.example/show.xml", rss),
        Ok(Sniffed::Feed)
    );
    let atom = b"<?xml version='1.0' encoding='UTF-8'?><feed xmlns=\"http://www.w3.org/2005/Atom\"><title>";
    assert_eq!(sniff("https://blog.example/atom", atom), Ok(Sniffed::Feed));
    let rdf = b"<?xml version=\"1.0\"?><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">";
    assert_eq!(
        sniff("https://old.example/index.rdf", rdf),
        Ok(Sniffed::Feed)
    );

    // ONE STREAM, not a list: an HLS index begins with `#EXTM3U` too.
    let hls = b"#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-STREAM-INF:BANDWIDTH=800000\nlow.m3u8\n";
    assert_eq!(
        sniff("https://tv.example/live.m3u8", hls),
        Err(Refused::NotAList)
    );
    let page = b"<!DOCTYPE html><html><head><title>A page</title>";
    assert_eq!(sniff("https://www.example/", page), Err(Refused::Unknown));
    assert_eq!(sniff("https://www.example/", b""), Err(Refused::Unknown));
    assert_eq!(
        sniff("ftp://files.example/list.m3u", m3u),
        Err(Refused::NotAnAddress)
    );
    assert_eq!(sniff("not an address", m3u), Err(Refused::NotAnAddress));
    // AN ADDRESS WHOSE HOST WOULD BE A PIECE OF THE ACCOUNT: a raw `/` or
    // `?` in the user or the password, which every parser takes for the end
    // of the host.
    for url in [
        "https://ana/w:pw@iptv.example/list.m3u",
        "https://ana?x:pw@iptv.example/list.m3u",
        "https://ana:p/w@iptv.example/list.m3u",
        "https://iptv.example:port/list.m3u",
    ] {
        assert_eq!(sniff(url, m3u), Err(Refused::NotAnAddress), "{url}");
    }
    assert_eq!(
        sniff(
            "https://iptv.example/get.php?username=ana@mail.example&password=b",
            m3u
        ),
        Ok(Sniffed::Playlist),
        "an `@` in the query of an address with a path is the query's"
    );
}

/// FAILS IF: a feed or a list at a YouTube or archive.org address is refused
/// for its host before its bytes are read — a YouTube channel's own feed is
/// what `feed` was written to read.
#[test]
fn a_feed_on_youtube_or_the_archive_is_told_by_its_bytes() {
    let youtube_feed =
        "https://www.youtube.com/feeds/videos.xml?channel_id=UCSMOQeBJ2RAnuFungnQOxLg";
    assert_eq!(Sniffed::by_address(youtube_feed), None, "its bytes say");
    // The opening YouTube writes there (the whole feed is read in
    // `feed::tests`).
    let atom = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<feed xmlns:yt=\"http://www.youtube.com/xml/schemas/2015\" xmlns:media=\"http://search.yahoo.com/mrss/\" xmlns=\"http://www.w3.org/2005/Atom\">";
    assert_eq!(sniff(youtube_feed, atom), Ok(Sniffed::Feed));

    let archive_rss = "https://archive.org/services/collection-rss.php?collection=prelinger";
    assert_eq!(Sniffed::by_address(archive_rss), None);
    let rss = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\" xmlns:media=\"http://search.yahoo.com/mrss/\"><channel><title>prelinger</title>";
    assert_eq!(sniff(archive_rss, rss), Ok(Sniffed::Feed));
    let archive_m3u = "https://archive.org/download/some_item/some_item_vbr.m3u";
    assert_eq!(
        sniff(
            archive_m3u,
            b"#EXTM3U\n#EXTINF:-1,One\nhttps://archive.org/download/some_item/one.mp3\n"
        ),
        Ok(Sniffed::Playlist)
    );
    // A PAGE on those hosts is still no list.
    let page = b"<!DOCTYPE html><html><head><title>Search</title>";
    assert_eq!(
        sniff("https://archive.org/search?query=x", page),
        Err(Refused::Unknown)
    );
    assert_eq!(
        sniff("https://www.youtube.com/feed/trending", page),
        Err(Refused::Unknown)
    );
}

#[test]
fn a_youtube_or_archive_address_says_what_it_is_without_a_byte() {
    let channel = Ok(Sniffed::YouTube {
        list: YouTubeList::Channel,
    });
    let playlist = Ok(Sniffed::YouTube {
        list: YouTubeList::Playlist,
    });
    assert_eq!(
        sniff("https://www.youtube.com/@BlenderOfficial", b""),
        channel
    );
    assert_eq!(
        sniff("https://m.youtube.com/@BlenderOfficial/videos", b""),
        channel
    );
    assert_eq!(
        sniff(
            "https://www.youtube.com/channel/UCSMOQeBJ2RAnuFungnQOxLg",
            b""
        ),
        channel
    );
    assert_eq!(
        sniff("https://www.youtube.com/c/BlenderFoundation", b""),
        channel
    );
    assert_eq!(
        sniff(
            "https://www.youtube.com/playlist?list=PLa1F2ddGya_-UvuAqHAksYnB0qL9yWDO6",
            b""
        ),
        playlist
    );
    assert_eq!(
        sniff("https://www.youtube.com/watch?v=aaaaaaaaaaa&list=PLx", b""),
        playlist
    );
    assert_eq!(
        sniff("https://youtu.be/aaaaaaaaaaa?list=PLx", b""),
        playlist
    );
    assert_eq!(
        sniff("https://www.youtube.com/watch?v=aaaaaaaaaaa", b""),
        Err(Refused::NotAList),
        "one video"
    );
    assert_eq!(
        sniff("https://youtu.be/aaaaaaaaaaa", b""),
        Err(Refused::NotAList)
    );
    assert_eq!(
        sniff("https://www.youtube.com/shorts/aaaaaaaaaaa", b""),
        Err(Refused::NotAList)
    );
    assert_eq!(
        sniff("https://www.youtube.com/feed/trending", b""),
        Err(Refused::Unknown),
        "not known by its address, and no bytes"
    );
    assert_eq!(
        sniff("https://archive.org/details/prelinger", b""),
        Ok(Sniffed::Collection {
            identifier: "prelinger".to_owned()
        })
    );
    assert_eq!(
        sniff("https://archive.org/search?query=x", b""),
        Err(Refused::Unknown)
    );
    assert_eq!(Sniffed::Playlist.kind(), Kind::Playlist);
    assert!(Sniffed::by_address("https://www.youtube.com/@x").is_some());
    assert!(Sniffed::by_address("https://iptv.example/list.m3u").is_none());
}

/// FAILS IF: a person's group lands under the wrong theme, or an adult one
/// is not sent to the adult store — or an ordinary word containing an adult
/// one is (`Essex`, `Sussex`).
#[test]
fn a_group_finds_its_theme_and_the_adult_store_by_whole_words() {
    assert_eq!(theme_of("Notícias 24h"), Some("news"));
    assert_eq!(
        theme_of("NOTICIAS"),
        Some("news"),
        "accents and case folded"
    );
    assert_eq!(theme_of("Filmes"), Some("films"));
    // pt-BR: groups as a person's list names them — the aliases' subject.
    assert_eq!(theme_of("Desenhos Animados"), Some("cartoons"));
    assert_eq!(
        theme_of("Science Fiction"),
        Some("scifi-horror"),
        "a phrase"
    );
    assert_eq!(theme_of("Documentários"), Some("documentary"));
    assert_eq!(theme_of("Sci-Fi"), Some("scifi-horror"));
    assert_eq!(theme_of("Audiobooks"), Some("audiobooks"));
    assert_eq!(theme_of("Canal 5"), None);
    assert_eq!(theme_of(""), None);

    assert!(is_adult("XXX Movies"));
    assert!(is_adult("Adultos +18"));
    assert!(is_adult("18+ Only"));
    assert!(is_adult("Erótico"));
    assert!(!is_adult("Essex News"));
    assert!(!is_adult("Middlesex"));
    assert!(!is_adult("Adulting 101"));
}

/// FAILS IF: a person's address reaches a screen or a log with their
/// account in it.
#[test]
fn a_person_s_address_is_shown_as_its_host_and_never_printed() {
    let a =
        Address::new("  https://user:pw@iptv.example:8080/get.php?username=ana&password=hunter2  ");
    assert_eq!(a.shown(), "iptv.example/…");
    let printed = format!("{a:?}");
    assert_eq!(printed, "Address(iptv.example/…)");
    assert!(!printed.contains("hunter2") && !printed.contains("pw"));
    assert_eq!(
        a.expose(),
        "https://user:pw@iptv.example:8080/get.php?username=ana&password=hunter2"
    );
    assert_eq!(shown("https://Feeds.Example"), "feeds.example");
    assert_eq!(shown("https://feeds.example/"), "feeds.example");
    assert_eq!(shown("https://feeds.example/?a=b"), "feeds.example/…");
    assert_eq!(
        shown("https://www.youtube.com/@Blender"),
        "www.youtube.com/…"
    );
    assert_eq!(
        shown("https://iptv.example/get.php?username=ana@mail.example&password=hunter2"),
        "iptv.example/…"
    );
    // A USER OR A PASSWORD WITH A RAW `/`, `?`, `#` OR `\`: every parser
    // reads its start as the host, so no host is shown at all.
    for url in [
        "https://ana:p/w@iptv.example/list.m3u",
        "https://ana/w:hunter2@iptv.example/list.m3u",
        "https://ana?x:hunter2@iptv.example/list.m3u",
        "https://ana#x:hunter2@iptv.example/list.m3u",
        "https://ana:p\\w@iptv.example/list.m3u",
        "not an address",
        "https://",
    ] {
        assert_eq!(shown(url), MASK, "{url}");
        let a = Address::new(url);
        assert!(!format!("{a:?}").contains("ana"), "{url}");
    }
}

#[test]
fn every_name_is_its_serde_name() {
    for r in Refused::ALL {
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            format!("\"{}\"", r.id())
        );
    }
    for k in Kind::ALL {
        assert_eq!(
            serde_json::to_string(&k).unwrap(),
            format!("\"{}\"", k.id())
        );
        assert_eq!(Kind::of(k.id()), Some(k));
    }
    assert_eq!(Refused::from(crate::Why::NotAName), Refused::NotAnAddress);
    assert_eq!(
        Refused::from(ronoctua_ytdlp::Trouble::Absent),
        Refused::NoYtDlp
    );
}

/// FAILS IF: a default source's request goes out without the person's
/// consent for its host — through any source, whatever network it is handed.
#[cfg(feature = "net")]
#[test]
fn a_default_source_asks_only_on_the_person_s_consent() {
    use crate::canned::{Canned, pass_for};
    let net = Canned::new(&[(
        "https://images-api.nasa.gov/search?q=apollo&media_type=video&page=1&page_size=100",
        200,
        crate::nasa::tests::SEARCH.as_bytes(),
    )]);
    let wrong = pass_for("sepiasearch.org", "wrong");
    assert_eq!(
        crate::nasa::search(&net, &wrong, "apollo", 1).unwrap_err(),
        Refused::NotLet
    );
    assert!(net.asked().is_empty(), "nothing was sent");
    let right = pass_for("images-api.nasa.gov", "right");
    let page = crate::nasa::search(&net, &right, "apollo", 1).unwrap();
    assert_eq!(page.videos.len(), 1);
    assert_eq!(net.urls().len(), 1);

    let peertube = Canned::new(&[]);
    let nasa_pass = pass_for("images-api.nasa.gov", "other");
    assert_eq!(
        crate::peertube::search(&peertube, &nasa_pass, 2, 0).unwrap_err(),
        Refused::NotLet
    );
    assert!(peertube.asked().is_empty());
    assert_eq!(
        crate::commons::members(&peertube, &nasa_pass, "Videos of cats", None).unwrap_err(),
        Refused::NotLet
    );
    assert!(peertube.asked().is_empty());
}
