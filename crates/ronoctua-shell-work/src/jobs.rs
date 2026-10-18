//! A JOB OF THE QUEUE, IN WORDS — what ended between two lists, the report
//! a job ends as, the sentence under the pointer, and the queue as a buffer
//! is handed it.
//!
//! # Why a module of its own
//!
//! Hearing the queue is a door's (`crate::queue`); SAYING what a job is
//! needs the words of everything a job can be — an archive, a seal, a
//! program, the belongings — and the reports a view hears. That is the work
//! half: the queue's view, the file manager, the security centre and the
//! queue's buffer all say a job the same way because they say it here.

use ronoctua_transfers::{Job, Measure, Stage};

use crate::queue::opens;

/// What the row says under the pointer, and what a finished job leaves behind.
///
/// A failure's own words, when there are any: a row that says only "failed" is
/// a row that sends somebody to the journal.
#[must_use]
pub fn under_the_pointer(job: &Job, texts: &ronoctua_i18n::Texts) -> String {
    // A PROGRAM'S REFUSAL IS A SENTENCE, and its word (`declared-in-the-flake`)
    // is the CLI's name for it. The program's own words, when there are more
    // than the word, follow it.
    if let Some(why) = job.refusal {
        let said = texts.t(crate::software_words::refusal_key(why));
        return match job.trouble.as_deref().map(str::trim) {
            Some(t) if !t.is_empty() && t != why.word() => format!("{said} · {t}"),
            _ => said,
        };
    }
    // AN ARCHIVE'S REFUSAL IS A SENTENCE too, naming the archive: the job
    // carries its word (`Job::archive`) and no prose.
    if let Some(why) = job.archive {
        let e = crate::report::refusal(
            why,
            crate::report::subject_of(&job.work).unwrap_or(std::path::Path::new("")),
        );
        return texts.t_with(e.key, &e.borrowed());
    }
    // A SEAL'S REFUSAL IS A SENTENCE too (`seal-refused-<id>`), naming the
    // file it was about.
    if let (Some(why), ronoctua_transfers::Work::Seal { job: sealed, .. }) = (job.seal, &job.work) {
        return texts.t_with(
            crate::seal::why_key(why),
            &[("name", &crate::seal::name_of(&sealed.from))],
        );
    }
    if let Some(trouble) = &job.trouble {
        return trouble.clone();
    }
    let stage = texts.t(&format!("transfers-stage-{}", job.stage.word()));
    let engine = texts.t(&format!("transfers-engine-{}", job.engine.word()));
    format!("{stage} · {engine}")
}

/// WHAT ENDED BETWEEN THE FOOTER'S TWO LISTS, as reports ([`what_ended`]) —
/// the news the footer tells the open regions of its host, which hear the
/// queue through it: ONE comparison, against the footer's own last list, so
/// a region that was closed while a job ended is not told it ended when it
/// opens again, and a job that failed with no region open is still said
/// (by the footer, which says every failure it hears).
#[derive(Debug, Clone, PartialEq)]
pub struct Ended(pub Vec<crate::report::Report>);

impl Ended {
    /// What ended ([`what_ended`]), split by who hears it: the failures'
    /// own words, which the footer says itself whichever view is open, and
    /// the rest, for the open regions.
    #[must_use]
    pub fn split(reports: Vec<crate::report::Report>) -> (Vec<String>, Self) {
        let mut failed = Vec::new();
        let mut told = Vec::new();
        for report in reports {
            match report {
                crate::report::Report::Failed(why) => failed.push(why),
                other => told.push(other),
            }
        }
        (failed, Self(told))
    }
}

/// What ENDED between two lists, as the file manager's own reports.
///
/// # Why this exists at all
///
/// The window door is a second process with no panel and no modeline: all it
/// needs from the queue is "something you asked for is over, read the directory
/// again" and "that folder turned out to be this big". Handing it the whole
/// job model would make it draw one, and there is nothing there to draw it in.
///
/// The comparison is against the PREVIOUS list, not against a stage: a finished
/// job stays in the history for a long time, and reacting to `Done` on every
/// beat would re-read the directory sixty times a minute for a copy that ended
/// an hour ago.
///
/// `before` is `None` until the door has heard a list at all — see [`ended`].
#[must_use]
pub fn what_ended(before: Option<&[Job]>, now: &[Job]) -> Vec<crate::report::Report> {
    ended(before, now)
        // A PROGRAM'S ORDER IS NOT THE FILE MANAGER'S: its end is its own
        // notification, with the refusal as a sentence (`software::ended_of`),
        // and a report here would echo the refusal's bare word.
        .filter(|j| !matches!(j.work, ronoctua_transfers::Work::Software { .. }))
        .flat_map(|j| {
            let report = report_of(j);
            // AN ARCHIVE'S JOB THAT FAILED WITHOUT A WORD for it — a source
            // that could not be weighed, a program that died — still ended:
            // its failure is said with its own words, and the view that sent
            // it lets go of the password it kept for asking again.
            let let_go = (j.stage == Stage::Failed
                && j.archive.is_none()
                && crate::report::subject_of(&j.work).is_some())
            .then(|| {
                crate::report::Report::Archive(Box::new(crate::report::ArchiveReport::Ended(
                    Box::new(j.work.clone()),
                )))
            });
            // A SEAL THAT ENDED WITHOUT A REFUSAL — done, stopped, a program
            // that died with no verdict (said above, as any job's failure) —
            // is let go of by the view that sent it, so a later refusal of
            // the same file from another door is not taken for its own.
            let sealed = match &j.work {
                ronoctua_transfers::Work::Seal { job, .. } if j.seal.is_none() => Some(
                    crate::report::Report::Seal(Box::new(crate::report::SealReport::Ended {
                        from: job.from.clone(),
                        opens: crate::seal::opening_of(job).is_some(),
                    })),
                ),
                _ => None,
            };
            std::iter::once(report).chain(let_go).chain(sealed)
        })
        .collect()
}

/// The one report a job that ended makes — see [`what_ended`].
fn report_of(j: &Job) -> crate::report::Report {
    match (&j.work, j.answer, j.stage) {
        (ronoctua_transfers::Work::Measure { path }, Some(bytes), _) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            crate::report::Report::Measured(name, bytes)
        }
        // AN ARCHIVE'S REFUSAL goes to the view that sent it: a password
        // wanted is a question there, anything else a sentence.
        (work, _, Stage::Failed) if j.archive.is_some() => {
            crate::report::Report::Archive(Box::new(crate::report::ArchiveReport::JobRefused {
                work: Box::new(work.clone()),
                why: j.archive.unwrap_or(ronoctua_archive::Why::Failed),
            }))
        }
        // A SEAL'S REFUSAL goes to the view that sent it: a wrong passphrase
        // is asked again there, anything else a sentence (`crate::seal`).
        (ronoctua_transfers::Work::Seal { job, .. }, _, Stage::Failed) if j.seal.is_some() => {
            crate::report::Report::Seal(Box::new(crate::report::SealReport::Refused {
                from: job.from.clone(),
                opening: crate::seal::opening_of(job),
                why: j.seal.unwrap_or(ronoctua_pq::Why::Failed),
            }))
        }
        // AN ARCHIVE THE QUEUE CHANGED is listed again by a view standing
        // in it.
        (ronoctua_transfers::Work::ArchiveEdit { archive, .. }, _, Stage::Done) => {
            crate::report::Report::Archive(Box::new(crate::report::ArchiveReport::Changed(
                archive.clone(),
            )))
        }
        // ANY OTHER END OF AN ARCHIVE'S JOB, so the view that sent it
        // lets go of the password it kept for asking again — and reads
        // the directory again, as `Done` does.
        (work, _, stage) if stage != Stage::Failed && crate::report::subject_of(work).is_some() => {
            crate::report::Report::Archive(Box::new(crate::report::ArchiveReport::Ended(Box::new(
                work.clone(),
            ))))
        }
        // A FAILURE CARRIES ITS OWN WORDS. `copiar /x/y` alone does not say
        // `No space left on device`, and the second half is the whole
        // diagnosis.
        (_, _, Stage::Failed) => crate::report::Report::Failed(
            j.trouble
                .clone()
                .unwrap_or_else(|| j.stage.word().to_owned()),
        ),
        _ => crate::report::Report::Done,
    }
}

/// The jobs that ENDED between two lists: afoot in the first, not in the
/// second. The one definition of "this job is over", for the file manager's
/// reports and the software centre's notifications alike.
///
/// # And one that was BORN and ended between them
///
/// The queue sends the list as it is when its reader wakes, not once per
/// change: a job refused in the instant it starts — a password file that does
/// not open, an archive that is not one — can be added and failed between two
/// lists, and was never afoot in any list anybody saw. Such a job is newer
/// than every job of the list before (ids only grow), and it counts as ended.
///
/// # `None` is "no list heard yet", and an EMPTY list is a list
///
/// The first list a door receives is the whole history, and none of it ended
/// just now: `before` is `None` for it. After that an empty list is an answer
/// like any other — and it is the COMMON one: the queue keeps its jobs in
/// memory and numbers them from one each time it starts, so the first job of a
/// session follows an empty list, and a refusal in its first instant was
/// missed while "a list that had jobs" was the rule.
pub fn ended<'a>(before: Option<&'a [Job]>, now: &'a [Job]) -> impl Iterator<Item = &'a Job> + 'a {
    let newest = before.map(|b| b.iter().map(|j| j.id).max().unwrap_or(0));
    now.iter().filter(|j| !j.afoot()).filter(move |j| {
        before.is_some_and(|b| b.iter().any(|old| old.id == j.id && old.afoot()))
            || newest.is_some_and(|newest| j.id > newest)
    })
}

/// Listens to the queue and turns each list into the file manager's reports.
///
/// What the WINDOW door uses — see [`what_ended`]. `false` when there is
/// nowhere to put a socket, and that is not fatal: the window goes on working,
/// and its listing refreshes on the next key instead of by itself.
pub fn listen_as_reports(
    channel: smithay_client_toolkit::reexports::calloop::channel::Sender<crate::report::Report>,
) -> bool {
    if ronoctua_transfers::wire::socket_path().is_err() {
        return false;
    }
    let built = std::thread::Builder::new()
        .name("transfers".into())
        .spawn(move || {
            // NONE until the first list: that one is the whole history.
            let mut before: Option<Vec<Job>> = None;
            let _ = ronoctua_transfers::client::watch(|jobs| {
                for report in what_ended(before.as_deref(), &jobs) {
                    if channel.send(report).is_err() {
                        return true;
                    }
                }
                before = Some(jobs);
                false
            });
        });
    built.is_ok()
}

/// The queue's own model, as [`ronoctua_protocol::Errand`].
///
/// An exhaustive `match` and not `Work::word()` parsed back: a work variant
/// added without an arm here does not compile, where a string lookup would
/// simply draw the wrong word.
const fn errand_of(work: &ronoctua_transfers::Work) -> ronoctua_protocol::Errand {
    use ronoctua_protocol::Errand as E;
    use ronoctua_transfers::Work as W;
    match work {
        W::Paste { moves: true, .. } => E::MoveIt,
        W::Paste { moves: false, .. } => E::Copy,
        W::Delete { .. } => E::Delete,
        W::Shred { .. } => E::Shred,
        W::Trash { .. } => E::Trash,
        W::Restore { .. } => E::Restore,
        W::Compress { .. } => E::Compress,
        W::Extract { .. } => E::Extract,
        // AN ARCHIVE'S OTHER WORK, each by its own word — the ones the
        // footer's door reads out of `Work::word`, so the two doors of one
        // queue name a job alike. An edit is an ADD when it adds anything,
        // which is `Work::word`'s own rule.
        W::ArchiveEdit { add, .. } if !add.is_empty() => E::ArchiveAdd,
        W::ArchiveEdit { .. } => E::ArchiveRemove,
        W::ArchiveTest { .. } => E::ArchiveTest,
        W::ArchiveConvert { .. } => E::ArchiveConvert,
        W::Measure { .. } => E::Measure,
        W::Fetch { .. } => E::Fetch,
        W::Media {
            want: ronoctua_transfers::Want::Video,
            ..
        } => E::Media,
        W::Media {
            want: ronoctua_transfers::Want::Audio,
            ..
        } => E::Audio,
        W::Media {
            want: ronoctua_transfers::Want::Subtitles,
            ..
        } => E::Subtitles,
        W::Torrent { .. } => E::Torrent,
        // A REGION IS A DOWNLOAD, to the window that draws the queue.
        //
        // `ronoctua_protocol::Errand` crosses the WIT into a wasip2 buffer, so
        // a variant of its own would be a WIT change, a regenerated binding
        // and a rebuild of every buffer — for a word. What this job does from
        // the queue's side IS a fetch, and the row says WHICH: its subject is
        // the region's name. The FOOTER's door, which is not a buffer, reads
        // `Work::word` and says "install a region's streets" in full.
        W::Atlas { .. } => E::Fetch,
        // A PROGRAM'S INSTALL, REMOVAL OR UPDATE, for the same reason: no WIT
        // change for a word. The buffer's row says what it is by its subject
        // (the program's name); the footer's door names the order in full.
        W::Software { .. } => E::Fetch,
        // A FILE SENT TO VIRUSTOTAL, for the same reason: an upload is a
        // transfer, and the row's subject names the file.
        W::Scan { .. } => E::Fetch,
        // A PULL, A BUILD OR COMPOSE, for the same reason: no WIT change for
        // a word — the row's subject says what it is.
        W::Container { .. } => E::Fetch,
        // THE BELONGINGS' SYNC, for the same reason: its row's subject is the
        // order's word, and the footer's door names it in full.
        W::Belongings { .. } => E::Fetch,
        // A SEAL, AN INCUS JOB, THE INSTALLER WRITTEN TO A STICK: each its
        // own word in the window too, because the same word the others take
        // would say "download" of a file encrypted here — a sentence that is
        // false, not short (the footer names them by `Work::word`). The
        // installer's DOWNLOAD is a download.
        W::Seal { job, .. } if job.seals() => E::Seal,
        W::Seal { .. } => E::Unseal,
        W::Incus { .. } => E::Incus,
        W::Stick { write: None, .. } => E::Fetch,
        W::Stick { write: Some(_), .. } => E::StickWrite,
    }
}

/// And the engine.
const fn engine_of(engine: ronoctua_transfers::Engine) -> ronoctua_protocol::Engine {
    use ronoctua_protocol::Engine as E;
    match engine {
        // ONE WORD EACH, the queue's own: the buffer names the engine the
        // footer names, with the same `transfers-engine-<word>` key. It knew
        // four, and a region, a program, a scan, a container and a sync all
        // said "link" there while the footer said what they were.
        ronoctua_transfers::Engine::Local => E::Local,
        ronoctua_transfers::Engine::Atlas => E::Atlas,
        ronoctua_transfers::Engine::Fetch => E::Fetch,
        ronoctua_transfers::Engine::Media => E::Media,
        ronoctua_transfers::Engine::Torrent => E::Torrent,
        ronoctua_transfers::Engine::Software => E::Software,
        ronoctua_transfers::Engine::Scan => E::Scan,
        ronoctua_transfers::Engine::Container => E::Container,
        ronoctua_transfers::Engine::Belongings => E::Belongings,
        ronoctua_transfers::Engine::Seal => E::Seal,
        ronoctua_transfers::Engine::Incus => E::Incus,
        ronoctua_transfers::Engine::Stick => E::Stick,
    }
}

const fn stage_of(stage: Stage) -> ronoctua_protocol::Stage {
    use ronoctua_protocol::Stage as S;
    match stage {
        Stage::Weighing => S::Weighing,
        Stage::Running => S::Running,
        Stage::Paused => S::Paused,
        Stage::Done => S::Done,
        Stage::Failed => S::Failed,
        Stage::Cancelled => S::Cancelled,
    }
}

const fn amount_of(measure: Measure) -> ronoctua_protocol::Amount {
    match measure {
        Measure::Bytes => ronoctua_protocol::Amount::Bytes,
        Measure::Items => ronoctua_protocol::Amount::Items,
    }
}

/// What the row says it is working on.
///
/// The name it is on RIGHT NOW beats the set's first name, for the same reason
/// the footer's sentence prefers it: while twelve files are copying, "which one
/// is it on" is the question. Already cut to a file name — a component drawing
/// a whole path would push everything else off the row.
///
/// The belongings' order is not a name but a word of ours (`prepare`,
/// `apply`…), drawn in the person's language
/// (`belongings_words::op_key`), never as the CLI writes it.
pub fn subject_of(job: &Job, texts: &ronoctua_i18n::Texts) -> String {
    if let ronoctua_transfers::Work::Belongings { op } = &job.work {
        return texts.t(crate::belongings_words::op_key(*op));
    }
    if !job.now.is_empty() {
        return job.now.clone();
    }
    job.work.subjects().first()
}

/// Whether the work went out as the person.
///
/// It comes out of the WORK and not out of a setting: the request already
/// carried who was asking, and a screen that read a setting would say what is
/// true NOW rather than what was true when the job started.
pub const fn identified(work: &ronoctua_transfers::Work) -> bool {
    use ronoctua_transfers::Identity;
    use ronoctua_transfers::Work as W;
    matches!(
        work,
        W::Fetch {
            identity: Identity::Mine,
            ..
        } | W::Media {
            identity: Identity::Mine,
            ..
        }
    )
}

/// The queue as the buffer receives it.
#[must_use]
pub fn queue_of(jobs: &[Job], texts: &ronoctua_i18n::Texts) -> ronoctua_protocol::Queue {
    ronoctua_protocol::Queue {
        jobs: jobs
            .iter()
            .map(|j| ronoctua_protocol::Job {
                id: j.id,
                engine: engine_of(j.engine),
                errand: errand_of(&j.work),
                stage: stage_of(j.stage),
                amount: amount_of(j.measure),
                done: j.done,
                total: j.total,
                subject: subject_of(j, texts),
                // The failure is DATA, and it crosses as it is: it was written
                // by whoever failed, and a buffer that received a key here
                // would need the host to have a key for every `io::Error`
                // there will ever be.
                //
                // EXCEPT AN ARCHIVE'S REFUSAL, which is a closed set with a
                // sentence each: it crosses as that sentence, in the host's
                // language — the same one the footer's door says under the
                // pointer — and never as its bare word.
                trouble: if j.archive.is_some() || j.seal.is_some() {
                    under_the_pointer(j, texts)
                } else {
                    j.trouble.clone().unwrap_or_default()
                },
                answer: j.answer.unwrap_or(0),
                identified: identified(&j.work),
                pauses: j.engine.pauses(),
                opens: opens(j).is_some(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Report;

    /// FAILS IF: a failure is told to the regions instead of being kept for
    /// the footer to say — it would be said twice with the manager open and
    /// never with it closed — or anything else that ended is not told, in
    /// the order it ended.
    #[test]
    fn what_ended_is_split_by_who_hears_it() {
        let (failed, told) = Ended::split(vec![
            Report::Done,
            Report::Failed("No space left on device".to_owned()),
            Report::Measured("a".to_owned(), 3),
            Report::Failed("Permission denied".to_owned()),
        ]);
        assert_eq!(failed, ["No space left on device", "Permission denied"]);
        assert_eq!(
            told,
            Ended(vec![Report::Done, Report::Measured("a".to_owned(), 3)])
        );
        assert_eq!(Ended::split(Vec::new()), (Vec::new(), Ended(Vec::new())));
    }
}
