//! The belongings' words: which text key each closed set of the crate is
//! drawn with — the refusals, the address checks, the decisions, the access,
//! the orders and the reasons of what is kept.
//!
//! # Why a file of its own
//!
//! Because both doors draw these (`belongings.rs` builds the rows either door
//! shows), the file manager says two of them (`files-belongings`), and a word
//! chosen in two places is two words the day one of them moves. And because
//! every set here is CLOSED in `ronoctua-belongings`: a `match` over it is a
//! compile error when the crate grows a case, and the census test below reads
//! every case in every language — a variant with no sentence would draw its
//! raw key, which is the shape invariant 8 exists to refuse.

use crate::key;
use ronoctua_belongings::Why;
use ronoctua_belongings::inventory::State;
use ronoctua_belongings::order::Op;
use ronoctua_belongings::plain::HeldWhy;
use ronoctua_belongings::plan::Decision;
use ronoctua_belongings::prefs::{Auth, Invalid};
use ronoctua_person::tree::TreeWhy;

/// The sentence of a refusal of a sync.
#[must_use]
pub const fn refusal_key(why: Why) -> &'static str {
    match why {
        Why::NoRepository => key::BELONGINGS_REFUSED_NO_REPOSITORY,
        Why::NoPassphrase => key::BELONGINGS_REFUSED_NO_PASSPHRASE,
        Why::WrongPassphrase => key::BELONGINGS_REFUSED_WRONG_PASSPHRASE,
        Why::NeedsAccess => key::BELONGINGS_REFUSED_NEEDS_ACCESS,
        Why::Refused => key::BELONGINGS_REFUSED_REFUSED,
        Why::Unreachable => key::BELONGINGS_REFUSED_UNREACHABLE,
        Why::Raced => key::BELONGINGS_REFUSED_RACED,
        Why::NotOurs => key::BELONGINGS_REFUSED_NOT_OURS,
        Why::Empty => key::BELONGINGS_REFUSED_EMPTY,
        Why::Corrupt => key::BELONGINGS_REFUSED_CORRUPT,
        Why::Undecided => key::BELONGINGS_REFUSED_UNDECIDED,
        Why::NoSession => key::BELONGINGS_REFUSED_NO_SESSION,
        Why::NoTools => key::BELONGINGS_REFUSED_NO_TOOLS,
        Why::Failed => key::BELONGINGS_REFUSED_FAILED,
        Why::Busy => key::BELONGINGS_REFUSED_BUSY,
        Why::NotLet => key::BELONGINGS_REFUSED_NOT_LET,
        Why::Newer => key::BELONGINGS_REFUSED_NEWER,
        Why::PlainTooLarge => key::BELONGINGS_REFUSED_PLAIN_TOO_LARGE,
        Why::NotPostQuantum => key::BELONGINGS_REFUSED_NOT_POST_QUANTUM,
    }
}

/// Why an address was refused.
#[must_use]
pub const fn invalid_key(why: Invalid) -> &'static str {
    match why {
        Invalid::Empty => key::BELONGINGS_INVALID_EMPTY,
        Invalid::CredentialsInAddress => key::BELONGINGS_INVALID_CREDENTIALS_IN_ADDRESS,
        Invalid::PlainHttp => key::BELONGINGS_INVALID_PLAIN_HTTP,
        Invalid::NotRemote => key::BELONGINGS_INVALID_NOT_REMOTE,
        Invalid::UnnamedHost => key::BELONGINGS_INVALID_UNNAMED_HOST,
        Invalid::Unknown => key::BELONGINGS_INVALID_UNKNOWN,
    }
}

/// A decision as a conflict's VALUE: what it is decided to be.
#[must_use]
pub const fn decided_key(d: Decision) -> &'static str {
    match d {
        Decision::KeepLocal => key::BELONGINGS_DECIDED_KEEP_LOCAL,
        Decision::TakeRemote => key::BELONGINGS_DECIDED_TAKE_REMOTE,
        Decision::KeepBoth => key::BELONGINGS_DECIDED_KEEP_BOTH,
        Decision::TakeMergedOurs => key::BELONGINGS_DECIDED_TAKE_MERGED_OURS,
        Decision::TakeMergedTheirs => key::BELONGINGS_DECIDED_TAKE_MERGED_THEIRS,
    }
}

/// A decision as a ROW the person picks — the verb.
#[must_use]
pub const fn decide_key(d: Decision) -> &'static str {
    match d {
        Decision::KeepLocal => key::BELONGINGS_DECIDE_KEEP_LOCAL,
        Decision::TakeRemote => key::BELONGINGS_DECIDE_TAKE_REMOTE,
        Decision::KeepBoth => key::BELONGINGS_DECIDE_KEEP_BOTH,
        Decision::TakeMergedOurs => key::BELONGINGS_DECIDE_TAKE_MERGED_OURS,
        Decision::TakeMergedTheirs => key::BELONGINGS_DECIDE_TAKE_MERGED_THEIRS,
    }
}

/// The command a decision's key runs (`C-l`, `C-r`, `C-b`), whose `cmd-`
/// sentence is also what its row says under the pointer.
#[must_use]
pub const fn decide_command(d: Decision) -> &'static str {
    match d {
        Decision::KeepLocal => "belongings-keep-local",
        Decision::TakeRemote => "belongings-take-remote",
        Decision::KeepBoth => "belongings-keep-both",
        Decision::TakeMergedOurs => "belongings-take-merged-ours",
        Decision::TakeMergedTheirs => "belongings-take-merged-theirs",
    }
}

/// How the repository is reached, as a value.
#[must_use]
pub const fn auth_key(a: Auth) -> &'static str {
    match a {
        Auth::Ssh => key::BELONGINGS_AUTH_SSH,
        Auth::Token => key::BELONGINGS_AUTH_TOKEN,
    }
}

/// What an order the queue took is called, in the sentence that says it was
/// taken.
#[must_use]
pub const fn op_key(op: Op) -> &'static str {
    match op {
        Op::Prepare => key::BELONGINGS_OP_PREPARE,
        Op::Apply => key::BELONGINGS_OP_APPLY,
        Op::Abandon => key::BELONGINGS_OP_ABANDON,
        Op::Keygen => key::BELONGINGS_OP_KEYGEN,
        Op::Rekey => key::BELONGINGS_OP_REKEY,
        Op::Compact => key::BELONGINGS_OP_COMPACT,
        Op::SyncPlain => key::BELONGINGS_OP_SYNC_PLAIN,
        Op::Forget => key::BELONGINGS_OP_FORGET,
        Op::Restore => key::BELONGINGS_OP_RESTORE,
        Op::Probe => key::BELONGINGS_OP_PROBE,
        Op::Init => key::BELONGINGS_OP_INIT,
        Op::VerifyPassphrase => key::BELONGINGS_OP_VERIFY_PASSPHRASE,
        Op::Sign => key::BELONGINGS_OP_SIGN,
    }
}

/// Why a change of the repository's to a signed file was held, as the value
/// of its row.
#[must_use]
pub const fn held_key(why: HeldWhy) -> &'static str {
    match why {
        HeldWhy::Unsigned => key::BELONGINGS_HELD_UNSIGNED,
        HeldWhy::NoKey => key::BELONGINGS_HELD_NO_KEY,
        HeldWhy::BadSignature => key::BELONGINGS_HELD_BAD_SIGNATURE,
        HeldWhy::RolledBack => key::BELONGINGS_HELD_ROLLED_BACK,
        HeldWhy::NotAsSigned => key::BELONGINGS_HELD_NOT_AS_SIGNED,
    }
}

/// Why root refused a staging for its signature, as a sentence; `None` for
/// a reason that is not the signature's.
#[must_use]
pub const fn root_refused_key(why: TreeWhy) -> Option<&'static str> {
    Some(match why {
        TreeWhy::NotSigned => key::BELONGINGS_ROOT_REFUSED_NOT_SIGNED,
        TreeWhy::BadSignature => key::BELONGINGS_ROOT_REFUSED_BAD_SIGNATURE,
        TreeWhy::RolledBack => key::BELONGINGS_ROOT_REFUSED_ROLLED_BACK,
        _ => return None,
    })
}

/// Why root kept a file back (its own copy kept), as the value of its row;
/// `None` for a reason that does not keep one back.
#[must_use]
pub const fn kept_back_key(why: TreeWhy) -> Option<&'static str> {
    Some(match why {
        TreeWhy::NotAsSigned => key::BELONGINGS_KEPT_BACK_NOT_AS_SIGNED,
        TreeWhy::NoKeyPinned => key::BELONGINGS_KEPT_BACK_NO_KEY_PINNED,
        _ => return None,
    })
}

/// The sentence of a reason of what is kept, by its id — the table's
/// (`assets/belongings.nix`), so the key is assembled; the census test below
/// reads every reason the table gives in every language.
#[must_use]
pub fn reason_key(reason: &str) -> String {
    format!("belongings-reason-{reason}")
}

/// A file's state in the listing of what goes to the repository.
#[must_use]
pub const fn state_key(state: State) -> &'static str {
    match state {
        State::Synced => key::BELONGINGS_STATE_SYNCED,
        State::Changed => key::BELONGINGS_STATE_CHANGED,
        State::New => key::BELONGINGS_STATE_NEW,
        State::Conflict => key::BELONGINGS_STATE_CONFLICT,
        State::Unreadable => key::BELONGINGS_STATE_UNREADABLE,
        State::Repaired => key::BELONGINGS_STATE_REPAIRED,
        State::Deleted => key::BELONGINGS_STATE_DELETED,
    }
}

/// A predefined target's name, by its id — the table's, so the key is
/// assembled; the census test below reads every target in every language.
#[must_use]
pub fn target_key(id: &str) -> String {
    format!("belongings-target-{id}")
}

/// A period, as a value: "every 7 days", "never".
#[must_use]
pub fn period_text(days: Option<u16>, texts: &ronoctua_i18n::Texts) -> String {
    match days {
        Some(n) => texts.t_args(
            key::BELONGINGS_PERIOD_DAYS,
            &[("n", ronoctua_i18n::Arg::Number(i64::from(n)))],
        ),
        None => texts.t(key::BELONGINGS_PERIOD_NEVER),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a case of a closed set of the belongings — a refusal, an
    /// address check, a decision, an access, an order, a period, a reason the
    /// table gives — has no sentence in a declared language (it would draw its
    /// raw key), or two decisions read alike.
    #[test]
    fn every_word_has_its_sentence_in_every_language() {
        for language in ronoctua_i18n::LANGUAGES {
            let texts = ronoctua_i18n::Texts::fresh(language).expect("a declared language");
            let has = |k: &str| {
                assert!(texts.has(k), "{language}: `{k}` has no sentence");
            };
            for why in Why::ALL {
                has(refusal_key(why));
            }
            for why in [
                Invalid::Empty,
                Invalid::CredentialsInAddress,
                Invalid::PlainHttp,
                Invalid::NotRemote,
                Invalid::UnnamedHost,
                Invalid::Unknown,
            ] {
                has(invalid_key(why));
            }
            let mut said = std::collections::BTreeSet::new();
            for d in Decision::ALL {
                has(decided_key(d));
                has(decide_key(d));
                has(&format!("cmd-{}", decide_command(d)));
                assert!(
                    said.insert(texts.t(decide_key(d))),
                    "{language}: two decisions alike"
                );
            }
            for a in [Auth::Ssh, Auth::Token] {
                has(auth_key(a));
            }
            for op in Op::ALL {
                has(op_key(op));
            }
            // THE SIGNATURE'S: every reason a file is held, every reason
            // root refused or kept back — and none of those silently missing.
            for why in HeldWhy::ALL {
                has(held_key(why));
            }
            let mut refusing = 0;
            let mut keeping = 0;
            for why in TreeWhy::ALL {
                if let Some(k) = root_refused_key(why) {
                    has(k);
                    refusing += 1;
                }
                if let Some(k) = kept_back_key(why) {
                    has(k);
                    keeping += 1;
                }
                assert_eq!(root_refused_key(why).is_some(), why.refuses_the_signature());
                assert_eq!(kept_back_key(why).is_some(), why.keeps_back());
            }
            assert_eq!((refusing, keeping), (3, 2));
            for reason in ronoctua_belongings::kept::REASONS {
                has(&reason_key(reason));
            }
            // AND THE WALK'S OWN: what a file IS (a link into the system's
            // store, the access key under another name).
            for reason in ronoctua_belongings::sift::REASONS {
                has(&reason_key(reason));
            }
            // AND THE REPAIR LEDGER'S, by the id each reason is written with.
            for why in ronoctua_store::repair::Why::ALL {
                has(&format!("belongings-repair-why-{}", why.id()));
            }
            // AND THE LISTING'S: what a target skips (each by its own
            // reason, among the table's above — and asked of every target
            // here, so none falls through), a plain file past the line,
            // every state, every target by its id.
            for target in ronoctua_belongings::scope::targets() {
                for skip in target.skip {
                    has(&reason_key(skip.reason));
                }
            }
            for reason in ronoctua_belongings::inventory::REASONS {
                has(&reason_key(reason));
            }
            for state in State::ALL {
                has(state_key(state));
            }
            for target in ronoctua_belongings::scope::targets() {
                has(&target_key(target.id));
            }
            for days in ronoctua_belongings::prefs::PERIODS {
                let text = period_text(days, &texts);
                assert!(!text.starts_with('['), "{language}: {days:?} is {text}");
            }
        }
    }
}
