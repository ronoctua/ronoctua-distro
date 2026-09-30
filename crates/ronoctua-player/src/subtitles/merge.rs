//! Up to six lanes over one picture, written as ONE ASS script for mpv.
//!
//! # Why the merge places every line itself
//!
//! libass keeps apart lines that would overlap, but not in LANE order: a line
//! already on screen stays where it is, and only a line appearing in this
//! frame is moved away from it (libass 0.17.4, `fix_collisions`). So when the
//! second lane speaks before the first, the first is pushed ABOVE it, and the
//! order the person chose is lost. Layers never collide, and `\pos`/`\move`
//! switch collisions off. And libass reads no `Collisions:` header at all.
//!
//! So the merge cuts the timeline into intervals in which the same cues are
//! showing, and in each one gives every cue its `MarginV`: at the foot, lane
//! 1 nearest the edge and each next lane above the ones below it; at the top,
//! lane 1 nearest the top and each next lane below. A cue's height is its
//! line count — the merge breaks long lines itself and tells libass not to
//! (`WrapStyle: 2`), so the count it measures is the count libass draws —
//! times the font size, plus its outline. When the cues under a lane change,
//! its cue is cut there and goes on at its new height: an event already on
//! screen never has to move, so libass never has to decide.
//!
//! # What each lane keeps
//!
//! - Dialogue is drawn in the lane's colour with the lane's edge; italics,
//!   bold and underline stay. A colour the FILE gave a span is kept only when
//!   the lane is alone: in a stack, the colour is how a lane is told apart.
//! - An ASS lane's signs keep their own style and their place, scaled to this
//!   script, on a layer above the dialogue so they never push it.
//! - Each lane's offset is baked into its times.
//!
//! A lane that is ASS and ALONE is not merged at all: the engine loads the
//! file as it is. That is the caller's decision, not this function's.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::subtitles::ass::Sign;
use crate::subtitles::cue::{Band, Cue, Rgb, Span};

/// The most cues shown at once, over every lane: sixteen lines already fill
/// a picture, and a file that overlaps more is not dialogue. A cue that would
/// start past it is left out, which keeps the merge's work bounded by the
/// number of cues.
pub const SHOWN_MOST: usize = 16;

/// The rows of every merged script. Its width follows the picture
/// ([`Look::play_res_for`]).
pub const PLAY_RES_Y: u32 = 1080;

/// How the lanes are drawn: sizes in thousandths of the script's height,
/// the font, and the colour of every lane's edge. The caller fills it from
/// its tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Look {
    /// The script's width, in its units ([`Look::play_res_for`]).
    pub play_res_x: u32,
    /// The script's height, in its units ([`PLAY_RES_Y`]).
    pub play_res_y: u32,
    /// The font family.
    pub font: String,
    /// The font size, thousandths of the height.
    pub size_permille: u32,
    /// The outline's width, thousandths of the height.
    pub outline_permille: u32,
    /// The margin at every edge, thousandths of the height.
    pub margin_permille: u32,
    /// The air between two lanes' cues, thousandths of the height.
    pub gap_permille: u32,
    /// How wide one text cell is drawn, thousandths of the font size: what
    /// the merge breaks lines by. Wider than the font's real average is
    /// safe (a line breaks a little early); narrower lets a line run off the
    /// picture.
    pub advance_permille: u32,
    /// The colour of every lane's outline.
    pub edge_colour: Rgb,
}

impl Look {
    /// The script's size for a picture of `width_px` by `height_px`:
    /// [`PLAY_RES_Y`] rows, and as many columns as keep a unit square — so a
    /// line measured here is as wide on a 4:3 picture as on a 16:9 one.
    #[must_use]
    pub fn play_res_for(width_px: u32, height_px: u32) -> (u32, u32) {
        if width_px == 0 || height_px == 0 {
            return (PLAY_RES_Y * 16 / 9, PLAY_RES_Y);
        }
        let x = (u64::from(width_px) * u64::from(PLAY_RES_Y) + u64::from(height_px) / 2)
            / u64::from(height_px);
        (u32::try_from(x).unwrap_or(u32::MAX).max(1), PLAY_RES_Y)
    }
}

/// One lane to merge.
#[derive(Debug, Clone, Copy)]
pub struct MergeLane<'a> {
    /// Its dialogue.
    pub cues: &'a [Cue],
    /// Its signs (an ASS lane's typesetting; empty for every other format).
    pub signs: &'a [Sign],
    /// Its colour.
    pub colour: Rgb,
    /// Where it is stacked.
    pub stack: Band,
    /// How far its file's clock is moved, in milliseconds: a positive offset
    /// shows every cue later.
    pub offset_ms: i64,
}

/// The sizes of the script, in its units.
struct Metrics {
    size: u64,
    outline: u64,
    margin: u64,
    gap: u64,
    /// Width the text may take, in thousandths of a unit.
    room_milli: u64,
    /// Width of one text cell, in thousandths of a unit.
    cell_milli: u64,
}

impl Metrics {
    fn of(look: &Look) -> Self {
        let y = u64::from(look.play_res_y.max(1));
        let part = |permille: u32| (y * u64::from(permille) + 500) / 1000;
        let size = part(look.size_permille).max(1);
        let margin = part(look.margin_permille);
        let x = u64::from(look.play_res_x.max(1));
        Self {
            size,
            outline: part(look.outline_permille),
            margin,
            gap: part(look.gap_permille),
            room_milli: x.saturating_sub(2 * margin).max(1) * 1000,
            cell_milli: (size * u64::from(look.advance_permille)).max(1),
        }
    }
}

/// A cue ready to place: on the merged clock, in centiseconds (what ASS
/// writes), with its lines broken and its text written.
struct Item {
    lane: usize,
    order: usize,
    start_cs: u64,
    end_cs: u64,
    band: Band,
    height: u64,
    text: String,
}

/// A piece of an item at one margin.
struct Run {
    item: usize,
    from_cs: u64,
    to_cs: u64,
    margin: u64,
}

/// Writes the lanes as one ASS script.
#[must_use]
pub fn merge(lanes: &[MergeLane<'_>], look: &Look) -> String {
    let metrics = Metrics::of(look);
    let alone = lanes.len() == 1;
    let mut items = Vec::new();
    for (lane_index, lane) in lanes.iter().enumerate() {
        for (order, cue) in lane.cues.iter().enumerate() {
            let Some((start_cs, end_cs)) = shifted(cue.start_ms, cue.end_ms, lane.offset_ms) else {
                continue;
            };
            let lines: Vec<Vec<Span>> = cue
                .lines
                .iter()
                .flat_map(|line| wrap(line, metrics.room_milli, metrics.cell_milli))
                .collect();
            if lines.is_empty() {
                continue;
            }
            items.push(Item {
                lane: lane_index,
                order,
                start_cs,
                end_cs,
                band: cue.band.unwrap_or(lane.stack),
                height: lines.len() as u64 * metrics.size + 2 * metrics.outline,
                text: text_of(&lines, alone),
            });
        }
    }
    let runs = place(&items, &metrics);

    let to = (look.play_res_x, look.play_res_y);
    let mut out = String::new();
    let _ = write!(
        out,
        "[Script Info]\nScriptType: v4.00+\nPlayResX: {}\nPlayResY: {}\nWrapStyle: 2\nScaledBorderAndShadow: yes\nYCbCr Matrix: None\n\n",
        to.0, to.1
    );
    out.push_str("[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
    let font = look.font.replace(',', " ");
    let edge = look.edge_colour.ass_style();
    for (n, lane) in lanes.iter().enumerate() {
        let colour = lane.colour.ass_style();
        let _ = writeln!(
            out,
            "Style: Lane{lane},{font},{size},{colour},{colour},{edge},{edge},0,0,0,0,100,100,0,0,1,{outline},0,{align},{m},{m},{m},1",
            lane = n + 1,
            size = metrics.size,
            outline = metrics.outline,
            align = alignment(lane.stack),
            m = metrics.margin,
        );
    }
    // Every sign's style, once per lane and style name.
    let mut sign_styles: Vec<HashMap<&str, String>> = vec![HashMap::new(); lanes.len()];
    for (n, lane) in lanes.iter().enumerate() {
        for sign in lane.signs {
            let named = &mut sign_styles[n];
            if named.contains_key(sign.style.name.as_str()) {
                continue;
            }
            let name = format!("Lane{}Sign{}", n + 1, named.len() + 1);
            out.push_str(&sign.style_line(&name, to));
            out.push('\n');
            named.insert(sign.style.name.as_str(), name);
        }
    }
    out.push_str("\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");

    for run in &runs {
        let item = &items[run.item];
        let lane = &lanes[item.lane];
        let turn = if item.band == lane.stack {
            String::new()
        } else {
            format!("{{\\an{}}}", alignment(item.band))
        };
        let _ = writeln!(
            out,
            "Dialogue: 0,{},{},Lane{},,0,0,{},,{turn}{}",
            clock(run.from_cs),
            clock(run.to_cs),
            item.lane + 1,
            run.margin,
            item.text
        );
    }
    for (n, lane) in lanes.iter().enumerate() {
        for sign in lane.signs {
            let Some((start_cs, end_cs)) = shifted(sign.start_ms, sign.end_ms, lane.offset_ms)
            else {
                continue;
            };
            let style = sign_styles[n]
                .get(sign.style.name.as_str())
                .map_or("", String::as_str);
            let [left, right, vertical] = sign.margins_in(to);
            let _ = writeln!(
                out,
                "Dialogue: {},{},{},{style},,{left},{right},{vertical},{},{}",
                sign.layer.max(0).saturating_add(1),
                clock(start_cs),
                clock(end_cs),
                sign.effect_in(to),
                sign.text_in(to).replace('\n', " "),
            );
        }
    }
    out
}

/// The keypad alignment of a band, centred.
fn alignment(band: Band) -> u8 {
    match band {
        Band::Bottom => 2,
        Band::Top => 8,
    }
}

/// A cue's times on the merged clock, in centiseconds; `None` when nothing
/// of it is left after the offset.
fn shifted(start_ms: u64, end_ms: u64, offset_ms: i64) -> Option<(u64, u64)> {
    let moved = |ms: u64| {
        i64::try_from(ms)
            .unwrap_or(i64::MAX)
            .saturating_add(offset_ms)
    };
    let (start, end) = (moved(start_ms).max(0), moved(end_ms));
    let centis = |ms: i64| (u64::try_from(ms).unwrap_or(0) + 5) / 10;
    let (start_cs, end_cs) = (centis(start), centis(end));
    (end > 0 && end_cs > start_cs).then_some((start_cs, end_cs))
}

/// `h:mm:ss.cc`.
fn clock(cs: u64) -> String {
    let seconds = cs / 100;
    format!(
        "{}:{:02}:{:02}.{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60,
        cs % 100
    )
}

/// Every run of every item: the timeline cut where the cues showing change,
/// and each item's margin in each piece.
fn place(items: &[Item], metrics: &Metrics) -> Vec<Run> {
    let mut by_start: Vec<usize> = (0..items.len()).collect();
    by_start.sort_by_key(|&k| (items[k].start_cs, items[k].lane, items[k].order));
    let mut bounds: Vec<u64> = items.iter().flat_map(|i| [i.start_cs, i.end_cs]).collect();
    bounds.sort_unstable();
    bounds.dedup();

    let mut runs = Vec::new();
    let mut open: Vec<Option<(u64, u64)>> = vec![None; items.len()];
    let mut active: Vec<usize> = Vec::new();
    let mut next = 0;
    for &at in &bounds {
        active.retain(|&k| {
            if items[k].end_cs > at {
                return true;
            }
            if let Some((margin, since)) = open[k].take() {
                runs.push(Run {
                    item: k,
                    from_cs: since,
                    to_cs: at,
                    margin,
                });
            }
            false
        });
        while next < by_start.len() && items[by_start[next]].start_cs <= at {
            // Past the ceiling a cue would be drawn off the picture anyway.
            if active.len() < SHOWN_MOST {
                active.push(by_start[next]);
            }
            next += 1;
        }
        active.sort_by_key(|&k| (items[k].lane, items[k].start_cs, items[k].order));
        let mut reach = [metrics.margin; 2];
        for &k in &active {
            let side = usize::from(items[k].band == Band::Top);
            let margin = reach[side];
            reach[side] += items[k].height + metrics.gap;
            match open[k] {
                Some((old, since)) if old != margin => {
                    runs.push(Run {
                        item: k,
                        from_cs: since,
                        to_cs: at,
                        margin: old,
                    });
                    open[k] = Some((margin, at));
                }
                Some(_) => {}
                None => open[k] = Some((margin, at)),
            }
        }
    }
    runs.sort_by_key(|r| (r.from_cs, items[r.item].lane, items[r.item].order));
    runs
}

/// A grapheme of a line, where it came from and how wide it is.
struct Cluster<'a> {
    text: &'a str,
    span: usize,
    width_milli: u64,
    space: bool,
}

/// One line broken to fit `room_milli`, into as few lines as greedy
/// breaking gives, as even as that count allows.
fn wrap(line: &[Span], room_milli: u64, cell_milli: u64) -> Vec<Vec<Span>> {
    let clusters: Vec<Cluster> = line
        .iter()
        .enumerate()
        .flat_map(|(span, s)| {
            ronoctua_protocol::cells_of(&s.text).map(move |text| Cluster {
                text,
                span,
                width_milli: u64::from(ronoctua_protocol::cells_of_one(text)) * cell_milli,
                space: text.chars().all(|c| c.is_whitespace() && c != '\u{a0}'),
            })
        })
        .collect();
    if clusters.iter().all(|c| c.space) {
        return Vec::new();
    }
    let most = breaks(&clusters, room_milli);
    if most.len() <= 1 {
        return vec![rebuild(line, &clusters, 0, clusters.len())];
    }
    // The narrowest width that still needs no more lines: even lines.
    let widest = clusters.iter().map(|c| c.width_milli).max().unwrap_or(0);
    let total: u64 = clusters.iter().map(|c| c.width_milli).sum();
    let (mut low, mut high) = ((total / most.len() as u64).max(widest), room_milli);
    while low < high {
        let middle = low + (high - low) / 2;
        if breaks(&clusters, middle).len() <= most.len() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    breaks(&clusters, high)
        .into_iter()
        .map(|(from, to)| rebuild(line, &clusters, from, to))
        .collect()
}

/// Greedy breaking at `most_milli`: each line as a range of clusters, the
/// spaces at its edges left out.
fn breaks(clusters: &[Cluster], most_milli: u64) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut from = 0;
    let mut width = 0u64;
    let mut last_space: Option<usize> = None;
    let mut at = 0;
    while at < clusters.len() {
        let cluster = &clusters[at];
        if width + cluster.width_milli > most_milli && at > from {
            if cluster.space {
                lines.push((from, at));
                from = at + 1;
                width = 0;
                last_space = None;
                at += 1;
                continue;
            }
            if let Some(space) = last_space.filter(|&s| s > from) {
                lines.push((from, space));
                from = space + 1;
                width = clusters[from..at].iter().map(|c| c.width_milli).sum();
                last_space = (from..at).rev().find(|&i| clusters[i].space);
                continue;
            }
            lines.push((from, at));
            from = at;
            width = 0;
            last_space = None;
            continue;
        }
        width += cluster.width_milli;
        if cluster.space {
            last_space = Some(at);
        }
        at += 1;
    }
    if from < clusters.len() {
        lines.push((from, clusters.len()));
    }
    lines
        .into_iter()
        .map(|(mut a, mut b)| {
            while a < b && clusters[a].space {
                a += 1;
            }
            while b > a && clusters[b - 1].space {
                b -= 1;
            }
            (a, b)
        })
        .filter(|(a, b)| a < b)
        .collect()
}

/// The spans of clusters `from..to`, each with its span's look.
fn rebuild(line: &[Span], clusters: &[Cluster], from: usize, to: usize) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut current: Option<usize> = None;
    for cluster in &clusters[from..to] {
        if current != Some(cluster.span) {
            current = Some(cluster.span);
            spans.push(Span {
                text: String::new(),
                ..line[cluster.span].clone()
            });
        }
        if let Some(last) = spans.last_mut() {
            last.text.push_str(cluster.text);
        }
    }
    spans
}

/// Lines as an event's text: `\N` between them, override tags where the look
/// changes, the colour only when the lane is alone.
fn text_of(lines: &[Vec<Span>], keep_colours: bool) -> String {
    let mut out = String::new();
    let (mut italic, mut bold, mut underline) = (false, false, false);
    let mut colour: Option<Rgb> = None;
    for (n, line) in lines.iter().enumerate() {
        if n > 0 {
            out.push_str("\\N");
        }
        for span in line {
            let wanted = if keep_colours { span.colour } else { None };
            let mut tags = String::new();
            let mut toggle = |now: &mut bool, to: bool, tag: &str| {
                if *now != to {
                    *now = to;
                    let _ = write!(tags, "\\{tag}{}", u8::from(to));
                }
            };
            toggle(&mut italic, span.italic, "i");
            toggle(&mut bold, span.bold, "b");
            toggle(&mut underline, span.underline, "u");
            if wanted != colour {
                colour = wanted;
                match wanted {
                    Some(c) => {
                        let _ = write!(tags, "\\c{}", c.ass_override());
                    }
                    // Back to the style's own.
                    None => tags.push_str("\\c"),
                }
            }
            if !tags.is_empty() {
                let _ = write!(out, "{{{tags}}}");
            }
            out.push_str(&escaped(&span.text));
        }
    }
    out
}

/// Text that libass draws as it is: `{` and `}` escaped, and a backslash that
/// would start `\N`, `\n` or `\h` kept apart from its letter by a word joiner.
fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\\' if matches!(chars.peek(), Some('N' | 'n' | 'h')) => out.push_str("\\\u{2060}"),
            '\n' | '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitles::{self, Opened};

    fn look() -> Look {
        Look {
            play_res_x: 1920,
            play_res_y: 1080,
            font: "Sans".to_owned(),
            size_permille: 50,
            outline_permille: 3,
            margin_permille: 40,
            gap_permille: 10,
            advance_permille: 600,
            edge_colour: Rgb::new(0x01, 0x02, 0x03),
        }
    }

    // size 54, outline 3, margin 43, gap 11: one line is 60 tall, two 114.
    const BASE: u64 = 43;
    const ONE: u64 = 60 + 11;
    const TWO: u64 = 114 + 11;

    #[derive(Debug)]
    struct Event {
        layer: i64,
        start_cs: u64,
        end_cs: u64,
        style: String,
        margin_v: String,
        text: String,
    }

    fn events(script: &str) -> Vec<Event> {
        let centis = |clock: &str| {
            let (whole, cs) = clock.split_once('.').unwrap_or((clock, "0"));
            let seconds = whole
                .split(':')
                .fold(0u64, |t, f| t * 60 + f.parse::<u64>().unwrap_or(0));
            seconds * 100 + cs.parse::<u64>().unwrap_or(0)
        };
        script
            .lines()
            .filter_map(|l| l.strip_prefix("Dialogue: "))
            .map(|l| {
                let f: Vec<&str> = l.splitn(10, ',').collect();
                Event {
                    layer: f[0].parse().unwrap_or(-1),
                    start_cs: centis(f[1]),
                    end_cs: centis(f[2]),
                    style: f[3].to_owned(),
                    margin_v: f[7].to_owned(),
                    text: f[9].to_owned(),
                }
            })
            .collect()
    }

    /// The margin of `style`'s one event showing at `cs`.
    fn margin_at(script: &str, cs: u64, style: &str) -> Option<u64> {
        let showing: Vec<Event> = events(script)
            .into_iter()
            .filter(|e| e.style == style && e.start_cs <= cs && cs < e.end_cs)
            .collect();
        assert!(showing.len() <= 1, "{style} twice at {cs}: {showing:?}");
        showing.first().and_then(|e| e.margin_v.parse().ok())
    }

    fn lane<'a>(cues: &'a [Cue], stack: Band) -> MergeLane<'a> {
        MergeLane {
            cues,
            signs: &[],
            colour: Rgb::new(255, 255, 255),
            stack,
            offset_ms: 0,
        }
    }

    #[test]
    fn lane_order_is_kept_when_the_second_lane_speaks_first() {
        let first = [Cue::plain(2000, 5000, "one")];
        let second = [Cue::plain(1000, 6000, "two")];
        let script = merge(
            &[lane(&first, Band::Bottom), lane(&second, Band::Bottom)],
            &look(),
        );
        assert_eq!(
            margin_at(&script, 150, "Lane2"),
            Some(BASE),
            "alone, at the edge"
        );
        assert_eq!(margin_at(&script, 300, "Lane1"), Some(BASE));
        assert_eq!(
            margin_at(&script, 300, "Lane2"),
            Some(BASE + ONE),
            "above lane 1, though it came first"
        );
        assert_eq!(margin_at(&script, 550, "Lane2"), Some(BASE), "back down");
        assert!(!script.contains("Collisions"));
        assert!(script.contains("WrapStyle: 2"));
    }

    #[test]
    fn two_line_cues_push_the_lane_above_and_long_lines_are_broken() {
        let first = [Cue::plain(0, 3000, "a\nb")];
        let second = [Cue::plain(0, 3000, "c")];
        let script = merge(
            &[lane(&first, Band::Bottom), lane(&second, Band::Bottom)],
            &look(),
        );
        assert_eq!(margin_at(&script, 100, "Lane2"), Some(BASE + TWO));

        let long = "a line of words that is far too long to fit across the picture in one go";
        let first = [Cue::plain(0, 3000, long)];
        let script = merge(
            &[lane(&first, Band::Bottom), lane(&second, Band::Bottom)],
            &look(),
        );
        assert_eq!(
            margin_at(&script, 100, "Lane2"),
            Some(BASE + TWO),
            "wrapped to two lines"
        );
        let lane1 = events(&script)
            .into_iter()
            .find(|e| e.style == "Lane1")
            .map(|e| e.text)
            .unwrap_or_default();
        let halves: Vec<&str> = lane1.split("\\N").collect();
        assert_eq!(halves.len(), 2);
        assert!(
            halves[0].len().abs_diff(halves[1].len()) < 12,
            "even lines: {halves:?}"
        );
    }

    #[test]
    fn offsets_shift_every_cue() {
        let cues = [
            Cue::plain(0, 1000, "gone"),
            Cue::plain(1000, 3000, "cut"),
            Cue::plain(4000, 5000, "moved"),
        ];
        let late = MergeLane {
            offset_ms: 1500,
            ..lane(&cues, Band::Bottom)
        };
        let early = MergeLane {
            offset_ms: -1500,
            ..lane(&cues, Band::Bottom)
        };
        let late: Vec<(u64, u64)> = events(&merge(&[late], &look()))
            .iter()
            .map(|e| (e.start_cs, e.end_cs))
            .collect();
        assert_eq!(late, vec![(150, 250), (250, 450), (550, 650)]);
        let early: Vec<(u64, u64)> = events(&merge(&[early], &look()))
            .iter()
            .map(|e| (e.start_cs, e.end_cs))
            .collect();
        assert_eq!(
            early,
            vec![(0, 150), (250, 350)],
            "the first is gone, the second cut at zero"
        );
    }

    #[test]
    fn colours_are_written_blue_first() {
        let cues = [Cue::plain(0, 1000, "x")];
        let coloured = MergeLane {
            colour: Rgb::new(0x12, 0x34, 0x56),
            ..lane(&cues, Band::Bottom)
        };
        let script = merge(&[coloured], &look());
        assert!(
            script.contains("Style: Lane1,Sans,54,&H00563412,&H00563412,&H00030201,&H00030201,"),
            "{script}"
        );
    }

    #[test]
    fn a_lone_lane_keeps_the_files_colours_and_a_stack_does_not() {
        let Ok(Opened::Ours(track)) = subtitles::open(
            b"1\n00:00:01,000 --> 00:00:02,000\n<font color=\"#ff0000\">red</font> {x}\\N\n",
            Some("srt"),
            None,
        ) else {
            panic!("not read");
        };
        let alone = merge(&[lane(&track.cues, Band::Bottom)], &look());
        let text = &events(&alone)[0].text;
        assert_eq!(text, "{\\c&H0000FF&}red{\\c} \\{x\\}\\\u{2060}N");
        let stacked = merge(
            &[
                lane(&track.cues, Band::Bottom),
                lane(&track.cues, Band::Bottom),
            ],
            &look(),
        );
        assert!(events(&stacked).iter().all(|e| !e.text.contains("\\c")));
    }

    #[test]
    fn ass_signs_stay_in_place_and_dialogue_takes_the_lane_colour() {
        let script_in = "[Script Info]\nPlayResX: 1280\nPlayResY: 720\n\n[V4+ Styles]\n\
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,20,20,30,1\n\
Style: Board,Georgia,36,&H00336699,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,7,10,10,10,1\n\n\
[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,{\\c&H0000FF&}Spoken\n\
Dialogue: 2,0:00:01.00,0:00:03.00,Board,,0,0,0,,{\\pos(640,360)}SHOP\n";
        let Ok(Opened::Ours(ass_track)) = subtitles::open(script_in.as_bytes(), Some("ass"), None)
        else {
            panic!("not read");
        };
        let other = [Cue::plain(1000, 3000, "other")];
        let with_signs = MergeLane {
            signs: &ass_track.signs,
            offset_ms: 500,
            colour: Rgb::new(0, 255, 0),
            ..lane(&ass_track.cues, Band::Bottom)
        };
        let script = merge(&[lane(&other, Band::Bottom), with_signs], &look());
        let all = events(&script);
        let sign = all
            .iter()
            .find(|e| e.style == "Lane2Sign1")
            .map(|e| (e.layer, e.start_cs, e.text.clone()));
        assert_eq!(
            sign,
            Some((3, 150, "{\\q0}{\\pos(960,540)}SHOP".to_owned()))
        );
        assert!(
            script.contains("Style: Lane2Sign1,Georgia,54,&H00336699,"),
            "{script}"
        );
        let spoken = all
            .iter()
            .find(|e| e.style == "Lane2")
            .map(|e| e.text.clone());
        assert_eq!(
            spoken.as_deref(),
            Some("Spoken"),
            "the lane's colour, not the file's"
        );
        assert_eq!(margin_at(&script, 200, "Lane2"), Some(BASE + ONE));
    }

    #[test]
    fn the_top_band_stacks_downwards() {
        let first = [Cue::plain(0, 2000, "first")];
        let second = [Cue::plain(0, 2000, "second")];
        let script = merge(
            &[lane(&first, Band::Top), lane(&second, Band::Top)],
            &look(),
        );
        assert!(script.contains("Style: Lane1,Sans,54,&H00FFFFFF,&H00FFFFFF,&H00030201,&H00030201,0,0,0,0,100,100,0,0,1,3,0,8,43,43,43,1"));
        assert_eq!(margin_at(&script, 100, "Lane1"), Some(BASE));
        assert_eq!(margin_at(&script, 100, "Lane2"), Some(BASE + ONE));

        // A cue that asks for the top, in a lane at the foot, joins the top.
        let mut asks = Cue::plain(0, 2000, "sign-ish");
        asks.band = Some(Band::Top);
        let asking = [asks];
        let script = merge(
            &[lane(&first, Band::Top), lane(&asking, Band::Bottom)],
            &look(),
        );
        let event = events(&script).into_iter().find(|e| e.style == "Lane2");
        assert_eq!(
            event.map(|e| (e.margin_v, e.text)),
            Some(((BASE + ONE).to_string(), "{\\an8}sign-ish".to_owned()))
        );
    }

    #[test]
    fn the_scripts_width_follows_the_picture() {
        assert_eq!(Look::play_res_for(1920, 1080), (1920, 1080));
        assert_eq!(Look::play_res_for(640, 480), (1440, 1080));
        assert_eq!(Look::play_res_for(0, 0), (1920, 1080));
    }
}
