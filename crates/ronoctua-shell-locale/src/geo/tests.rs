//! What the regions' view draws, and what a key does to it.

use super::{DECIDING_WIDTH, Errand, Press, Target, View};

fn texts() -> ronoctua_i18n::Texts {
    ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the default language")
}

/// A view over a scratch root with nothing installed in it.
///
/// THE ROOT IS AN ARGUMENT, which is `ronoctua-xdg`'s rule: a test that found
/// `$XDG_DATA_HOME` on its own would list — and could remove — whatever the
/// person running `cargo test` has installed.
fn view(name: &str) -> (View, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("ronoctua-geo-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    (View::at(dir.clone(), DECIDING_WIDTH), dir)
}

/// What the screen says, printed — the check no assertion replaces.
#[test]
fn what_the_regions_view_draws() {
    let (v, dir) = view("draws");
    let texts = texts();
    for row in v.rows(96, &texts) {
        let line: String = row.fields.iter().map(|f| f.text.clone()).collect();
        println!("{line}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: the cursor is born on a heading.
///
/// Every section opens with one, so a cursor at zero is a cursor on a title —
/// a screen where `RET` does nothing and nothing says why. The locale view
/// shipped exactly that, and the owner read it off the screen.
#[test]
fn the_cursor_is_born_on_a_row_that_acts() {
    let (v, dir) = view("cursor");
    let rows = v.rows(96, &texts());
    let at = rows.get(v.cursor()).expect("the cursor is on a row");
    assert!(!at.heading, "the cursor was born on a heading");
    assert_ne!(at.target, Target::Nothing);
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: `f` stops asking for the filter.
///
/// What the view asks for is a line of the DOOR's — the footer's minibuffer,
/// a window's header — and the answer comes back to `took_a_filter` from
/// either (`geo_window`'s tests hold the window's half).
#[test]
fn f_asks_for_the_filter_on_the_doors_own_line() {
    let (mut v, dir) = view("filter");
    assert_eq!(v.press(Press::Find, DECIDING_WIDTH, &texts()), Errand::Ask);
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: the id the question is asked under stops being the one the key
/// table teaches — the footer runs the table's id, the window asks under
/// `FIND`, and an answer under the other would be taken by nobody.
#[test]
fn the_question_is_asked_under_the_id_the_table_teaches() {
    let taught = super::OF_THE_VIEW
        .iter()
        .find(|(_, id)| *id == super::FIND)
        .map(|(key, _)| *key)
        .expect("the filter's id is not in the key table");
    assert_eq!(
        super::press_of_key(ronoctua_keymap::Key::parse(taught).expect("a key")),
        Some(Press::Find)
    );
}

/// FAILS IF: what was typed does not narrow the list, or narrows it by
/// matching something other than a name or an id.
#[test]
fn what_was_typed_narrows_the_list() {
    let (mut v, dir) = view("narrow");
    let all = v.shown().len();
    v.took_a_filter("zzzzzz-no-region-is-called-this", DECIDING_WIDTH);
    assert_eq!(
        v.shown().len(),
        0,
        "a filter that matches nothing kept rows"
    );
    v.took_a_filter("", DECIDING_WIDTH);
    assert_eq!(v.shown().len(), all, "an empty filter did not come back");
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: a row the filter kept does not say why — the letters that
/// matched come out lit, like the finder's.
#[test]
fn the_letters_the_filter_matched_are_lit() {
    let (mut v, dir) = view("lit");
    v.took_a_filter("zil", DECIDING_WIDTH);
    let rows = v.rows(96, &texts());
    let lit: Vec<&str> = rows
        .iter()
        .flat_map(|r| r.fields.iter())
        .filter(|f| {
            f.color == ronoctua_tokens::color::STATE_ACCENT
                || f.color == ronoctua_tokens::color::TEXT_MATCH_HERE
        })
        .map(|f| f.text.as_str())
        .collect();
    assert!(
        lit.iter().any(|t| t.eq_ignore_ascii_case("zil")),
        "no row lit the letters it matched: {lit:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: `d` on a row that is not an installed region removes something,
/// or says nothing.
#[test]
fn removing_needs_an_installed_region_under_the_cursor() {
    let (mut v, dir) = view("remove");
    // Nothing is installed, so the cursor is on the filter or on a catalogue
    // row — never on a pack.
    assert_eq!(
        v.press(Press::Remove, DECIDING_WIDTH, &texts()),
        Errand::Speak(crate::key::GEO_NOT_INSTALLED)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: `RET` on a catalogue row stops asking the queue for that region,
/// or asks for a different one.
///
/// The URL is the catalogue's and is a `-latest-` REDIRECT: what it points at
/// carries the date that becomes the pack's version, which is why this goes
/// through the queue's engine rather than through `ronoctua-http`.
#[test]
fn entering_a_catalogue_row_asks_the_queue_for_that_region() {
    let (mut v, dir) = view("install");
    let texts = texts();
    let Some(wanted) = v
        .shown()
        .iter()
        .find(|r| !r.url.is_empty())
        .map(|r| (*r).clone())
    else {
        // A build with no world data has no catalogue, and the view says so
        // rather than showing an empty list. Nothing to assert here.
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    let rows = v.rows(96, &texts);
    let want = v
        .shown()
        .iter()
        .position(|r| r.id == wanted.id)
        .expect("the region is shown");
    let at = rows
        .iter()
        .position(|r| matches!(r.target, Target::Install(n) if n == want))
        .expect("the row that installs it");
    v.clicked(at, &rows);
    assert_eq!(
        v.press(Press::Enter, DECIDING_WIDTH, &texts),
        Errand::Install {
            region: wanted.id,
            label: wanted.label,
            url: wanted.url,
        }
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: the question stops opening on `no`.
///
/// The format view's rule, and removing a region is an hour of somebody's
/// connection: the key after the question must not be the one that answers it
/// yes.
#[test]
fn the_question_opens_on_no() {
    let (mut v, dir) = view("sure");
    // The question is only reachable with something installed, so this asserts
    // the arithmetic rather than the flow: row 1 is `no` and row 2 is `yes`,
    // and the cursor is put on 1.
    let texts = texts();
    v.took_a_filter("", DECIDING_WIDTH);
    let before = v.cursor();
    assert_eq!(
        v.press(Press::Remove, DECIDING_WIDTH, &texts),
        Errand::Speak(crate::key::GEO_NOT_INSTALLED),
        "nothing is installed, so there is nothing to ask about"
    );
    assert_eq!(v.cursor(), before, "a refused question moved the cursor");
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: `Esc` stops closing.
#[test]
fn escape_closes() {
    let (mut v, dir) = view("escape");
    assert_eq!(
        v.press(Press::Escape, DECIDING_WIDTH, &texts()),
        Errand::Close
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: a region Geofabrik publishes WITHOUT a shapefile is left out of
/// the list, or pressing it says nothing.
///
/// Twenty-nine of the five hundred and fifty-five are like that, and they are
/// the ones people look for first — Brazil, Germany, France, Russia, the
/// United States. They used to be dropped while baking, and the owner reported
/// the result in four words: *"não achei o Brasil na lista"*.
#[test]
fn a_region_with_no_shapefile_is_drawn_and_refuses() {
    let (mut v, dir) = view("whole");
    let texts = texts();
    let Some(whole) = v
        .shown()
        .iter()
        .position(|r| r.url.is_empty())
        .map(|n| (n, v.shown()[n].label.clone()))
    else {
        // A build with no world data has no catalogue at all, and the view says
        // so instead of drawing an empty list.
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    let rows = v.rows(96, &texts);
    let at = rows
        .iter()
        .position(|r| matches!(r.target, Target::Whole(n) if n == whole.0))
        .unwrap_or_else(|| panic!("`{}` is not a row of this list", whole.1));
    v.clicked(at, &rows);
    assert_eq!(
        v.press(Press::Enter, DECIDING_WIDTH, &texts),
        Errand::Speak(crate::key::GEO_ONLY_IN_PARTS),
        "pressing a region that is published in parts said nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// FAILS IF: the parts of a country cannot be found by the country's name.
///
/// `Sudeste` is a row of five hundred and fifty; what makes it findable is the
/// label carrying `Brazil`, which is also what makes it READABLE. Without it
/// the filter for the country answers nothing and the list answers nothing
/// either — which is the whole of the report.
#[test]
fn a_country_s_parts_are_found_by_the_country_s_name() {
    let (mut v, dir) = view("parts");
    if v.shown().is_empty() {
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    v.took_a_filter("brazil", DECIDING_WIDTH);
    let found: Vec<String> = v.shown().iter().map(|r| r.label.clone()).collect();
    assert!(
        found.len() > 1,
        "filtering by a country found {found:?} — its parts are not reachable by its name"
    );
    assert!(
        found.iter().any(|l| l.contains("Sudeste")),
        "the country's own parts are not among {found:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
