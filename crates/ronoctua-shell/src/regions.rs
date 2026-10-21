//! THE REGIONS THE FOOTER HOSTS — one line per feature whose view is a
//! `region::Region` of its own crate.
//!
//! # Why a list in the binary
//!
//! The binary is the one crate that may name every feature: the footer
//! names none, and knows a region by the id it is ranked under in its
//! precedence (`surface::PRECEDENCE`) and by nothing else. A feature that
//! becomes a region adds its `region::Entry` here — sorted, one per line —
//! and its rank keeps the place its view already had; the footer refuses to
//! start on a region registered without a rank, or a rank no region answers
//! (`every_region_is_ranked`).
//!
//! A view the footer still draws with a field of its own
//! (`surface/<view>.rs`) moves here as it becomes a region.

use ronoctua_shell_door::region;

/// Every region the footer hosts, sorted by id.
pub const REGIONS: &[region::Entry] = &[
    ronoctua_shell_agents::agents_region::ENTRY,
    ronoctua_shell_audio::audio_region::ENTRY,
    ronoctua_shell_belongings::belongings_region::ENTRY,
    ronoctua_shell_bluetooth::bluetooth_region::ENTRY,
    ronoctua_shell_calc::calc_region::ENTRY,
    ronoctua_shell_camera::camera_region::ENTRY,
    ronoctua_shell_clipboard::clipboard_region::ENTRY,
    ronoctua_shell_containers::containers_region::ENTRY,
    ronoctua_shell_display::display_region::ENTRY,
    // `LEADER d d` — the search over everything, over cells of its own.
    ronoctua_shell_fast_access::fast_access_region::ENTRY,
    // `LEADER . .` — the file manager, laid out in sections.
    ronoctua_shell_files::files_region::ENTRY,
    ronoctua_shell_locale::geo_region::ENTRY,
    // `C-h` of the file manager — a repository's history, over the manager.
    ronoctua_shell_git::git_region::ENTRY,
    ronoctua_shell_history::history_region::ENTRY,
    ronoctua_shell_icons::icons_region::ENTRY,
    ronoctua_shell_incus::incus_region::ENTRY,
    ronoctua_shell_locale::locale_region::ENTRY,
    ronoctua_shell_manual::manual_region::ENTRY,
    ronoctua_shell_network::network_region::ENTRY,
    ronoctua_shell_notifications::notifications_region::ENTRY,
    // `LEADER p p` — the player, its panel beside its rows.
    ronoctua_shell_player::player_region::ENTRY,
    ronoctua_shell_pointers::pointers_region::ENTRY,
    ronoctua_shell_power::power_region::ENTRY,
    ronoctua_shell_security::security_region::ENTRY,
    ronoctua_shell_software::software_region::ENTRY,
    ronoctua_shell_stick::stick_region::ENTRY,
    ronoctua_shell_syncthing::syncthing_region::ENTRY,
    ronoctua_shell_transfers::transfers_region::ENTRY,
    ronoctua_shell_update::update_region::ENTRY,
    // `LEADER v v` — the video catalogue.
    ronoctua_shell_video_metrics::video_metrics_region::ENTRY,
];

/// What a feature keeps in the footer for the session, its region open or
/// closed (`region::Resident`) — beside its region's entry, sorted by id.
pub const RESIDENTS: &[region::Residing] = &[
    // `LEADER / c`'s question, kept until its answer.
    ronoctua_shell_git::git_region::RESIDING,
    ronoctua_shell_player::player_region::RESIDING,
    ronoctua_shell_video_metrics::video_metrics_region::RESIDING,
];

#[cfg(test)]
mod tests {
    use ronoctua_shell_door::region;

    /// FAILS IF: a region is registered without a rank in the footer's
    /// precedence, a rank names a region nothing answers, or an id is taken
    /// twice — the footer would refuse to start, and this says why first.
    #[test]
    fn every_region_is_ranked() {
        let slots = region::Slots::of(super::REGIONS).expect("one slot per id");
        ronoctua_shell_footer::surface::ranks_are_whole(&slots).expect("the precedence is whole");
    }

    /// FAILS IF: two regions answer one command while closed — the host asks
    /// the closed regions in turn and takes the first that answers
    /// (`Slots::closed_command`), so which view a command opened or acted on
    /// would be the order of this list, and a command is ONE view's.
    ///
    /// Asked of every command of the registry, through a photograph's door:
    /// nothing of the machine is reached on the way.
    #[test]
    fn no_two_regions_answer_one_command() {
        let (_, registry) = crate::keymap::build().expect("the keymap builds");
        let texts = ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language");
        let mut way = region::Recorder::new(
            std::rc::Rc::new(texts),
            80,
            true,
            ronoctua_shell_door::sudo::Gate::window(),
            ronoctua_shell_kit::wake::Wake::new(|| {}),
        );
        let mut answered = 0;
        for c in registry.all_ones() {
            let takers: Vec<&str> = super::REGIONS
                .iter()
                .filter(|e| (e.closed)(c.id, None, &mut way) != region::Closed::NotMine)
                .map(|e| e.id)
                .collect();
            assert!(takers.len() <= 1, "`{}` is answered by {takers:?}", c.id);
            answered += takers.len();
        }
        // AND THEY ANSWER SOMETHING: a closed function that answered nothing
        // would pass the line above for every command.
        assert!(
            answered >= 40,
            "only {answered} commands are the regions' — a closed function stopped answering"
        );
    }

    /// FAILS IF: a command of the crate of a region its closed entry answers
    /// whole is answered by nothing with the region closed — its entry says
    /// `NotMine`, and the key or `LEADER :` that runs it falls to "does
    /// nothing yet" — a command the footer answers itself is claimed by the
    /// region too, or the list of such regions names one nothing registers.
    ///
    /// The code as it RUNS: every command of the crate's contribution is
    /// handed to the closed entry, on a photograph's doorway, which reaches
    /// nothing of the machine (`run-command-handles-every-key` lets these
    /// commands leave its gaps on the strength of this test, and reads the
    /// other regions' commands in their own sources).
    #[test]
    fn every_hosted_command_reaches_its_region() {
        // THE REGIONS WHOSE CLOSED ENTRY ANSWERS EVERY COMMAND OF THEIR
        // CRATE — the camera's remote keys and its questions, the manuals,
        // the manager, the panel — whose commands no arm spells where the
        // check reads. A region with keys of its view alone (`locale-up`,
        // only open) or commands the footer's own line answers (the radio's
        // `bluetooth-scan`) is not one of them: its commands are read in its
        // sources.
        const ANSWERED_CLOSED: &[&str] = &["camera", "manual", "notifications", "transfers"];
        // THE BAND'S OWN, which the footer answers: the notification on the
        // band read and taken off it — the band is the footer's.
        const THE_FOOTERS: &[&str] = &["notif-dismiss"];
        let texts = std::rc::Rc::new(
            ronoctua_i18n::Texts::fresh(ronoctua_i18n::DEFAULT).expect("the base language"),
        );
        for id in ANSWERED_CLOSED {
            assert!(
                super::REGIONS.iter().any(|e| e.id == *id),
                "`{id}` is said to be answered closed, and no region is registered as it"
            );
        }
        let mut asked = 0;
        for entry in super::REGIONS
            .iter()
            .filter(|e| ANSWERED_CLOSED.contains(&e.id))
        {
            let keys = crate::keymap::CONTRIBUTIONS
                .iter()
                .find(|c| c.owner == entry.id)
                .unwrap_or_else(|| panic!("no contribution is the region `{}`'s", entry.id));
            for command in keys.commands {
                let mut way = region::Recorder::new(
                    std::rc::Rc::clone(&texts),
                    120,
                    true,
                    ronoctua_shell_door::sudo::Gate::window(),
                    ronoctua_shell_kit::wake::Wake::new(|| {}),
                );
                let answer = (entry.closed)(command.id, None, &mut way);
                if THE_FOOTERS.contains(&command.id) {
                    assert_eq!(
                        answer,
                        region::Closed::NotMine,
                        "`{}` is the footer's, and the region `{}` takes it",
                        command.id,
                        entry.id
                    );
                    continue;
                }
                assert_ne!(
                    answer,
                    region::Closed::NotMine,
                    "`{}` is the region `{}`'s, and its closed entry answers nothing",
                    command.id,
                    entry.id
                );
                asked += 1;
            }
        }
        assert!(asked >= ANSWERED_CLOSED.len(), "{asked} commands asked");
    }

    /// FAILS IF: the list stops being sorted, so the line a new region adds
    /// is not the one place a reader looks for it.
    #[test]
    fn the_regions_are_sorted() {
        let ids: Vec<&str> = super::REGIONS.iter().map(|e| e.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    /// FAILS IF: a resident is registered for a region that is not, or two
    /// for one region — the footer would refuse to start — or the list stops
    /// being sorted.
    #[test]
    fn every_resident_stands_beside_its_region() {
        let slots = region::Slots::of(super::REGIONS)
            .and_then(|s| s.with_residents(super::RESIDENTS))
            .expect("one resident beside each region that keeps one");
        let ids: Vec<&str> = slots.resident_ids().collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }
}
