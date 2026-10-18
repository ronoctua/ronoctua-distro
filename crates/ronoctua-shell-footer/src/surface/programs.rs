//! THE FOOTER'S OWN DEALINGS WITH THE PERSON'S PROGRAMS, view or no view —
//! the modeline's two software marks, the ends of the queue's software jobs,
//! and the event that opens what was just installed.
//!
//! The view itself is a region of its own crate (`ronoctua-shell-software`),
//! hosted by its id like any other; what is here lives while the footer does,
//! because a trial runs and a job ends with nobody looking at the view. It
//! says the person's programs in the words the centre says them in
//! (the work's `programs_words`, under both), as the modeline draws every mark.

use super::*;

/// The software centre's id, as the footer ranks it (`PRECEDENCE`) — the
/// region the binary registers under it, hosted by its id alone.
const SOFTWARE: &str = "software";

impl Footer {
    /// What a notification's "open it" opens: the entry the last finished
    /// install made — or the entry NAMED, the event's own: the launcher's
    /// path (the catalogue, the scope, the terminal for a program that needs
    /// one, the sweep for one installed a moment ago) lives with the
    /// catalogue, here. The software view, when it is up, gives the keyboard
    /// back first.
    pub(super) fn software_launch_latest(&mut self, named: Option<&str>) {
        let named = named.map(str::trim).filter(|n| !n.is_empty());
        match named
            .map(str::to_owned)
            .or_else(|| self.software_latest.clone())
        {
            Some(id) => {
                if self.regions.is_open(SOFTWARE) {
                    self.close_region(SOFTWARE);
                }
                self.launch_desktop(&id);
            }
            None => {
                let said = self.texts.t(crate::key::SOFTWARE_NOTHING_TO_OPEN);
                self.echo_message(said);
            }
        }
        self.draw_and_paint();
    }

    /// Software jobs of the queue ENDED: the launcher reads again, the marks
    /// are asked again, and some ends are a notification. (The view, when it
    /// is up, hears the same list and reads the machine again itself.)
    pub(super) fn software_jobs_ended(&mut self, ended: &[ronoctua_transfers::Job]) {
        if ended.is_empty() {
            return;
        }
        // THE LAUNCHER FIRST: an install made an entry, a removal took one.
        self.rescan_if_changed();
        // A TRIAL KEPT OR STOPPED, Android started or stopped.
        self.software_read_running();
        let mut said = Vec::new();
        for job in ended {
            if let Some(id) = crate::queue::opens(job) {
                self.software_latest = Some(id.to_owned());
            }
            let Some(end) = crate::programs_words::ended_of(job) else {
                continue;
            };
            let app = match &job.work {
                ronoctua_transfers::Work::Software { label, .. } => label.clone(),
                _ => String::new(),
            };
            // A PROGRAM PUT IN THE LAUNCHER IS AN EVENT, said on the
            // minibuffer's line with the entry's one action at its end —
            // opening what was just installed (`software-launch-latest`).
            // Every other end stays the notification it always was.
            if matches!(
                end,
                crate::programs_words::Ended::Installed | crate::programs_words::Ended::WebCreated
            ) {
                // WITH WHAT ITS ACTION OPENS: this job's own desktop id, taken
                // now, and not `software_latest`, which the next job in the
                // batch overwrites.
                said.push(crate::events::Event::installed(
                    software_tag(end),
                    &app,
                    crate::queue::opens(job),
                ));
                continue;
            }
            let why = if end == crate::programs_words::Ended::Failed {
                crate::programs_words::failure_said(job, &self.texts)
            } else {
                String::new()
            };
            self.software_notice(
                software_tag(end),
                crate::programs_words::ended_keys(end),
                &app,
                &why,
            );
        }
        self.said_now(said);
    }

    /// One of the software centre's notifications, in words.
    pub(super) fn software_notice(
        &mut self,
        tag: &'static str,
        (summary, body): (&'static str, &'static str),
        app: &str,
        why: &str,
    ) {
        let args = [("app", app), ("why", why)];
        let mut n = ronoctua_notify::Notification::fresh_one(
            0,
            self.texts.t(crate::key::NOTIF_SOFTWARE_APP),
            None,
            self.texts.t_with(summary, &args),
            self.texts.t_with(body, &args),
            ronoctua_notify::Urgency::Normal,
        );
        n.tags = vec![tag.to_owned()];
        self.arrived(n, std::time::Instant::now());
    }

    /// Ask what the two marks are drawn from, on a thread — one reading at a
    /// time, and one more when asked meanwhile.
    pub(super) fn software_read_running(&mut self) {
        let (Some(ctx), Some(sender)) = (
            self.software_context.clone(),
            self.software_running_sender.clone(),
        ) else {
            return;
        };
        if self.software_running_reading {
            self.software_running_again = true;
            return;
        }
        self.software_running_reading = true;
        crate::programs_words::read_running(ctx, move |r| {
            let _ = sender.send(r);
        });
    }

    /// A reading of the marks came back.
    pub(super) fn software_running_arrived(&mut self, running: crate::programs_words::Running) {
        self.software_running_reading = false;
        if std::mem::take(&mut self.software_running_again) {
            self.software_read_running();
        }
        self.software_running_now(running);
    }

    /// What the marks are drawn from, NOW: a trial that closed is said, and
    /// the modeline drawn when anything changed. THE MARKS FOLLOW the reading
    /// the view made too — it tells the footer (`Doorway::news`).
    pub(super) fn software_running_now(&mut self, running: crate::programs_words::Running) {
        if running == self.software_running {
            return;
        }
        for name in running.ended_since(&self.software_running) {
            self.software_notice(
                SOFTWARE_TRIAL_ENDED,
                (
                    crate::key::NOTIF_SOFTWARE_TRIAL_ENDED_SUMMARY,
                    crate::key::NOTIF_SOFTWARE_TRIAL_ENDED_BODY,
                ),
                &name,
                "",
            );
        }
        self.software_running = running;
        self.draw_and_paint();
    }

    /// A window closed: when a mark is up it may have been a trial's, so the
    /// marks are asked again. With none up, nothing is asked.
    pub(super) fn software_window_closed(&mut self) {
        if self.software_running.any() {
            self.software_read_running();
        }
    }
}
