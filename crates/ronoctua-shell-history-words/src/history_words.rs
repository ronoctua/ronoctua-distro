//! What the shell's history says when Atuin answered an order — said by the
//! footer, whose thread carries every order of its own and of the history's
//! region, and by the view's window, whose thread carries its own.
//!
//! Below both because the footer says it with no view open (`LEADER s h s`
//! syncs), and it names no feature: what both speak lives below both — in a
//! crate of its own, and not in the kit, so a line of the Atuin crate never
//! recompiles the foundation.

/// What a finished order says on the door's line, if anything — one answer
/// for both doors.
///
/// A REFUSAL IS ALWAYS A SENTENCE. Of the orders that went through, the two
/// whose effect the rows do not show say so: a sync and a log-out. A setting,
/// the mark and a login redraw their own rows. The footer used to say "the
/// history is synced" after EVERY order — a ring turned, a mark moved — which
/// was a sentence about something that had not happened, and the window said
/// nothing at all.
#[must_use]
pub fn said_when_done(
    order: &ronoctua_atuin::Order,
    refused: Option<&str>,
    texts: &ronoctua_i18n::Texts,
) -> Option<String> {
    use ronoctua_atuin::Order;
    if let Some(reason) = refused {
        return Some(texts.t_with(crate::key::ECHO_ATUIN_REFUSED, &[("reason", reason)]));
    }
    match order {
        Order::Sync => Some(texts.t(crate::key::ECHO_ATUIN_SYNCED)),
        Order::LogOut => Some(texts.t(crate::key::ECHO_ATUIN_LOGGED_OUT)),
        Order::LogIn { .. } | Order::Set { .. } | Order::Mark(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    /// WHAT A FINISHED ORDER SAYS, one answer for both doors: a refusal always,
    /// a sync and a log-out by name, and nothing over rows that show it.
    #[test]
    fn a_finished_order_says_what_it_did_and_only_that() {
        use ronoctua_atuin::Order;
        let t = texts();
        assert_eq!(
            said_when_done(&Order::Sync, None, &t),
            Some(t.t(crate::key::ECHO_ATUIN_SYNCED))
        );
        assert_eq!(
            said_when_done(&Order::LogOut, None, &t),
            Some(t.t(crate::key::ECHO_ATUIN_LOGGED_OUT))
        );
        assert_eq!(
            said_when_done(&Order::Mark(ronoctua_atuin::prefs::Mark::Never), None, &t),
            None
        );
        let refused = said_when_done(
            &Order::Mark(ronoctua_atuin::prefs::Mark::Never),
            Some("no room"),
            &t,
        )
        .expect("a refusal is a sentence");
        assert!(refused.contains("no room"), "{refused}");
    }
}
