//! THE STUDIO'S STATE, HEARD BY THE FOOTER: the modeline's camera and
//! recording marks, and the photos and recordings the studio says it wrote —
//! the footer's own, for the whole session, with the camera's views open or
//! closed.
//!
//! The camera's settings (`ronoctua-shell-camera`) read the same state: it is
//! lent to them and told to them as it changes (`hosted_heard`), and they
//! keep no copy of their own between two readings.
//!
//! It is no view: nothing here is ranked, and no dispatcher calls it.

use super::*;

impl Footer {
    /// The studio's new state: the marks, the notifications, the settings.
    pub(super) fn studio_arrived(&mut self, s: Option<ronoctua_studio::wire::State>) {
        // WHAT THE FOOTER HEARD, told to every region of the slot host first.
        self.hosted_heard::<Option<ronoctua_studio::wire::State>>(&s);
        // A PHOTO OR A RECORDING WAS WRITTEN: said once, by the catalogue —
        // and never for the first state, which reports what was written
        // before this footer started.
        let mut written: Vec<(String, bool)> = Vec::new();
        if let (Some(old), Some(new)) = (&self.studio, &s) {
            if let Some(path) = new
                .last_photo
                .clone()
                .filter(|_| new.last_photo != old.last_photo)
            {
                written.push((path, true));
            }
            if let Some(path) = new
                .last_recording
                .clone()
                .filter(|_| new.last_recording != old.last_recording)
            {
                written.push((path, false));
            }
        }
        for (path, photo) in written {
            let n = self.studio_notification(&path, photo);
            self.arrived(n, std::time::Instant::now());
        }
        // AND ITS RECORDING AND STREAM STARTING OR ENDING are events — the
        // file written stays the notification above, with its actions.
        let said = self.events.studio(s.as_ref());
        self.studio = s;
        self.camera_mark_allowed =
            ronoctua_studio::prefs::store().read().mark == ronoctua_studio::prefs::Mark::WhenInUse;
        self.draw_and_paint();
        self.said_now(said);
    }

    /// A photo or a recording, as a notification the catalogue recognises by
    /// its tag — the screenshot's shape, the path in the icon field.
    pub(super) fn studio_notification(
        &self,
        path: &str,
        photo: bool,
    ) -> ronoctua_notify::Notification {
        use crate::key;
        let (summary, body, tag) = if photo {
            (
                key::NOTIF_STUDIO_PHOTO_SUMMARY,
                key::NOTIF_STUDIO_PHOTO_BODY,
                "studio-photo",
            )
        } else {
            (
                key::NOTIF_STUDIO_RECORDING_SUMMARY,
                key::NOTIF_STUDIO_RECORDING_BODY,
                "studio-recording",
            )
        };
        let mut n = ronoctua_notify::Notification::fresh_one(
            0,
            self.texts.t(key::NOTIF_STUDIO_APP),
            Some(format!("file://{path}")),
            self.texts.t(summary),
            self.texts.t(body),
            ronoctua_notify::Urgency::Low,
        );
        n.tags = vec![tag.to_owned()];
        n
    }

    /// Whether the modeline draws the camera mark.
    pub(super) fn camera_in_use(&self) -> bool {
        self.camera_mark_allowed && !self.camera_holders.is_empty()
    }

    /// Whether the studio is recording or streaming.
    pub(super) fn studio_rolling(&self) -> bool {
        self.studio
            .as_ref()
            .is_some_and(|s| s.recording.is_some() || s.streaming.is_some())
    }
}
