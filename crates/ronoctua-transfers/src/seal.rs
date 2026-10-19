//! The seal engine: `ronoctua-seal` as a child — encrypt anything.
//!
//! # Why a child
//!
//! The belongings' reason ([`crate::belongings`]): sealing links `age` and
//! the X-Wing pieces, and this daemon — up for as long as the machine is —
//! links only the job's wire (`ronoctua-pq` without features,
//! `transfers-never-link-age`). The queue starts the program, hands it the
//! job on its standard input as ONE line (one `write_all`: a reader that
//! stopped for a moment must not get half of it), reads its lines
//! ([`ronoctua_pq::Line`]) and kills its group when told to.
//!
//! # Where the program is
//!
//! `RONOCTUA_SEAL`, which the unit sets (`nix/modules/seal.nix`) — by store
//! path, never a name on a unit's minimal `PATH`. The development loop has no
//! unit: there the program is the one BESIDE this daemon, which cargo built
//! into the same `bin/`.
//!
//! # Nothing secret stays, nothing of the person's goes
//!
//! - The job's secrets (a passphrase's `0600` file) are removed when it
//!   ends, however it ends — the program reads them and never deletes them,
//!   and [`crate::secrets::forget`] removes only files of the queue's own
//!   folder, so the belongings' `key.age` a job opens with is never one.
//! - The output is removed when the run is cut short — but ONLY once the
//!   program said it made it (its first `bytes` line comes right after
//!   `create_new`): before that line the name may be a file somebody else
//!   made, which the program refused to write over.
//!
//! # One at a time
//!
//! A passphrase is stretched by scrypt, and age picks its work factor by
//! timing the machine it runs on: about a second, which costs 256 MiB of
//! memory at the least (`2^18 × 8 × 128` bytes) and a gibibyte on a fast
//! machine. Forty files sealed at once were forty programs holding that
//! together — the memory killer's, and each timing its scrypt against the
//! other thirty-nine, so each picked a weaker factor than a lone seal would.
//! So the queue runs ONE seal at a time ([`Turn`]), in the order they were
//! queued; the rest wait on their rows (weighing, at nought) and a stop
//! reaches one that is waiting too. Opening is held the same way: the factor
//! of a file someone else sealed is whatever they chose.
//!
//! # What it does not honour
//!
//! Pause ([`crate::job::Engine::pauses`]): a stream held half way holds a
//! passphrase in the runtime folder for as long as somebody forgot the row.

use std::collections::{BTreeSet, VecDeque};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use ronoctua_files::watch::{Control, Flow};
use ronoctua_pq::{Line, Why};

use crate::engine::{POLL, Progress};
use crate::job::{Stage, Work};
use crate::ledger::Shared;

/// How long a cancelled child has to leave before its group is killed.
pub const GRACE: Duration = Duration::from_secs(2);

/// How many of the child's other lines are kept for the trouble.
const TAIL: usize = 12;

/// The variable naming the program's store path.
pub const VARIABLE: &str = "RONOCTUA_SEAL";

/// The program's name, beside this daemon in the development loop.
const BESIDE: &str = "ronoctua-seal";

/// Where the program is: the unit's variable, else beside this daemon (the
/// development loop). Never a bare name.
fn program() -> Option<PathBuf> {
    std::env::var_os(VARIABLE)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            let me = std::env::current_exe().ok()?;
            let it = me.parent()?.join(BESIDE);
            it.exists().then_some(it)
        })
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ending {
    /// `done`.
    Done,
    /// The program said why (`refused <why>`): a closed set with a sentence
    /// each on screen ([`crate::job::Job::seal`]).
    Refused(Why),
    /// It died, or could not be started — its own words.
    Failed(String),
    /// The switch said stop.
    Cancelled,
}

/// What the program said so far.
#[derive(Debug, Default)]
struct Heard {
    /// It made the output (its first `bytes` line).
    made: bool,
    done: bool,
    refused: Option<Why>,
    tail: VecDeque<String>,
}

impl Heard {
    /// One line; progress comes back to be shown (`0` total: not known).
    fn line(&mut self, text: &str, from_stdout: bool) -> Option<(u64, u64)> {
        if from_stdout && let Some(line) = Line::parse(text) {
            match line {
                Line::Bytes { done, total } => {
                    self.made = true;
                    return Some((done, total.unwrap_or(0)));
                }
                Line::Done { .. } => self.done = true,
                Line::Refused(why) => self.refused = Some(why),
            }
            return None;
        }
        let text = text.trim_end();
        if !text.is_empty() {
            self.tail.push_back(text.to_owned());
            if self.tail.len() > TAIL {
                self.tail.pop_front();
            }
        }
        None
    }

    /// How it ended, given the exit: a verdict line wins, then the exit.
    fn ending(self, success: bool, status: &str) -> Ending {
        if let Some(why) = self.refused {
            return Ending::Refused(why);
        }
        if success && self.done {
            return Ending::Done;
        }
        Ending::Failed(if self.tail.is_empty() {
            format!("the sealing program exited with {status} and no verdict")
        } else {
            Vec::from(self.tail).join("\n")
        })
    }
}

/// Whose turn it is to run the sealing program: one job at a time, the one
/// queued first among those waiting (a job's id grows with each job taken).
struct Turn {
    state: Mutex<Waiting>,
    moved: Condvar,
}

/// The job running, and the ones waiting for it.
#[derive(Default)]
struct Waiting {
    running: Option<u64>,
    queued: BTreeSet<u64>,
}

/// A turn held: the next job waiting starts when it is dropped.
struct Held<'a> {
    turn: &'a Turn,
}

impl Turn {
    /// Nobody running, nobody waiting.
    const fn new() -> Self {
        Self {
            state: Mutex::new(Waiting {
                running: None,
                queued: BTreeSet::new(),
            }),
            moved: Condvar::new(),
        }
    }

    /// The queue's one turn.
    fn of_the_queue() -> &'static Self {
        static ONE: Turn = Turn::new();
        &ONE
    }

    /// Waits until it is job `id`'s turn — `None` once `switch` says stop,
    /// and then it is no longer waiting. The switch is looked at every
    /// [`POLL`] while it waits: nothing signals a stop but the switch itself.
    fn wait(&self, id: u64, switch: &Control) -> Option<Held<'_>> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.queued.insert(id);
        loop {
            if switch.flow() == Flow::Stop {
                state.queued.remove(&id);
                // THE NEXT ONE may be the first now.
                self.moved.notify_all();
                return None;
            }
            if state.running.is_none() && state.queued.first() == Some(&id) {
                state.queued.remove(&id);
                state.running = Some(id);
                return Some(Held { turn: self });
            }
            state = self
                .moved
                .wait_timeout(state, POLL)
                .map_or_else(|e| e.into_inner().0, |(s, _)| s);
        }
    }
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        let mut state = self
            .turn
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        state.running = None;
        self.turn.moved.notify_all();
    }
}

/// Runs one job to its end, or to what the switch says. The job's thread.
pub fn run(shared: &Arc<Shared>, id: u64, switch: &Arc<Control>, work: Work) {
    let Work::Seal { job, .. } = work else {
        tracing::error!("the seal engine got a job that is not its own");
        shared.touch(id, |j| j.stage = Stage::Failed);
        return;
    };
    // ITS TURN FIRST: one sealing program at a time (the module's header).
    // Stopped while waiting, it ran nothing and made nothing.
    let held = Turn::of_the_queue().wait(id, switch);
    let ending = match (held.is_some(), program()) {
        (false, _) => Ending::Cancelled,
        (true, Some(program)) => carry(shared, id, switch, &program, &job, GRACE),
        (true, None) => Ending::Failed(format!(
            "this installation does not have the sealing program ({VARIABLE} is not set)"
        )),
    };
    drop(held);
    // NO SECRET OUTLIVES A JOB, a refused or a cancelled one included.
    for secret in job.secrets() {
        crate::secrets::forget(secret);
    }
    shared.touch(id, |j| match ending {
        Ending::Done => {
            j.stage = Stage::Done;
            if j.total == 0 || j.done > j.total {
                j.total = j.done;
            } else {
                j.done = j.total;
            }
        }
        Ending::Refused(why) => {
            j.stage = Stage::Failed;
            j.seal = Some(why);
        }
        Ending::Failed(trouble) => {
            j.stage = Stage::Failed;
            j.trouble = Some(trouble);
        }
        Ending::Cancelled => j.stage = Stage::Cancelled,
    });
}

/// The job as the program reads it: one line, with its end.
fn line_of(job: &ronoctua_pq::Job) -> Option<Vec<u8>> {
    let mut text = serde_json::to_vec(job).ok()?;
    text.push(b'\n');
    Some(text)
}

/// Removes what a run cut short had written — only what it said it made.
fn unmake(made: bool, into: &Path) {
    if !made {
        return;
    }
    match std::fs::remove_file(into) {
        Ok(()) => tracing::info!("the half of a seal that was stopped is gone"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!(error = %e, "the half of a stopped seal could not be removed"),
    }
}

/// One line, and which stream it came from.
struct Said {
    text: String,
    from_stdout: bool,
}

/// Starts `program` in a group of its own, hands it `job` and follows it to
/// the end.
fn carry(
    shared: &Arc<Shared>,
    id: u64,
    switch: &Arc<Control>,
    program: &Path,
    job: &ronoctua_pq::Job,
    grace: Duration,
) -> Ending {
    if switch.flow() == Flow::Stop {
        return Ending::Cancelled;
    }
    let Some(line) = line_of(job) else {
        return Ending::Failed("the job could not be written for the sealing program".to_owned());
    };
    let mut command = Command::new(program);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => return Ending::Failed(format!("could not run {}: {e}", program.display())),
    };
    let pid = child.id();
    // ONE `write_all`, and the pipe closed after it: the program reads one
    // line and nothing more from here.
    let handed = child
        .stdin
        .take()
        .map_or(Ok(()), |mut stdin| stdin.write_all(&line));
    if let Err(e) = handed {
        crate::software::stop(&mut child, pid, grace);
        return Ending::Failed(format!(
            "could not hand the job to the sealing program: {e}"
        ));
    }
    shared.under_way(id, |_| {});
    let (lines, arriving) = mpsc::channel::<Said>();
    reader(child.stdout.take(), lines.clone(), true);
    reader(child.stderr.take(), lines, false);
    let mut progress = Progress::new(shared, id);
    let mut heard = Heard::default();
    loop {
        if switch.flow() == Flow::Stop {
            progress.flush();
            crate::software::stop(&mut child, pid, grace);
            unmake(heard.made, &job.into);
            return Ending::Cancelled;
        }
        match arriving.recv_timeout(POLL) {
            Ok(said) => {
                if let Some((done, total)) = heard.line(&said.text, said.from_stdout) {
                    progress.set(done, total, "");
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    progress.flush();
    let made = heard.made;
    let ending = match child.wait() {
        Ok(status) => heard.ending(status.success(), &status.to_string()),
        Err(e) => Ending::Failed(format!("could not wait for the sealing program: {e}")),
    };
    // A PROGRAM THAT DIED WITHOUT A VERDICT (a signal, the memory killer) did
    // not reach the line that removes its half: it goes here. One that said
    // its verdict dealt with its output itself.
    if matches!(ending, Ending::Failed(_)) {
        unmake(made, &job.into);
    }
    ending
}

/// Pumps a stream into the channel, line by line, until it ends.
fn reader<R: std::io::Read + Send + 'static>(
    from: Option<R>,
    into: mpsc::Sender<Said>,
    from_stdout: bool,
) {
    let Some(from) = from else {
        return;
    };
    let built = std::thread::Builder::new()
        .name(if from_stdout { "seal-out" } else { "seal-err" }.into())
        .spawn(move || {
            use std::io::BufRead as _;
            let mut from = std::io::BufReader::new(from);
            let mut bytes = Vec::new();
            loop {
                bytes.clear();
                match from.read_until(b'\n', &mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let text = String::from_utf8_lossy(&bytes).into_owned();
                        if into.send(Said { text, from_stdout }).is_err() {
                            break;
                        }
                    }
                }
            }
        });
    if let Err(e) = built {
        tracing::warn!(error = %e, "could not start the seal's reading thread");
    }
}

// ── anchor: L22 ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::Engine;
    use ronoctua_pq::Action;

    fn place(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-transfers-seal-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A script in the program's place: what it read on stdin goes to
    /// `job.json`, then it says `body`.
    fn stand_in(dir: &Path, body: &str) -> PathBuf {
        let script = dir.join("seal.sh");
        crate::write_executable(
            &script,
            &format!(
                "#!/bin/sh\nhead -n 1 > '{}/job.json'\n{body}\n",
                dir.display()
            ),
        );
        script
    }

    fn sealing(dir: &Path, secret: PathBuf) -> ronoctua_pq::Job {
        ronoctua_pq::Job {
            action: Action::SealPassphrase {
                passphrase_file: secret,
                work: None,
            },
            from: dir.join("note.txt"),
            into: dir.join("note.txt.age"),
        }
    }

    #[test]
    fn a_verdict_wins_and_chatter_is_the_trouble() {
        let mut heard = Heard::default();
        assert_eq!(heard.line("bsdtar: a warning\n", false), None);
        assert_eq!(heard.line("bytes 0/9\n", true), Some((0, 9)));
        assert!(heard.made);
        assert_eq!(heard.line("bytes 4/-\n", true), Some((4, 0)));
        assert_eq!(heard.line("refused wrong-passphrase\n", true), None);
        assert_eq!(
            heard.ending(false, "exit status: 1"),
            Ending::Refused(Why::WrongPassphrase)
        );
        let mut heard = Heard::default();
        heard.line("thread 'main' panicked", false);
        assert_eq!(
            heard.ending(false, "signal: 9"),
            Ending::Failed("thread 'main' panicked".to_owned())
        );
        // SUCCESS WITHOUT `done` is not done: the verdict is what says so.
        assert!(matches!(
            Heard::default().ending(true, "exit status: 0"),
            Ending::Failed(_)
        ));
        let mut heard = Heard::default();
        heard.line("done 9", true);
        assert_eq!(heard.ending(true, ""), Ending::Done);
    }

    /// FAILS IF: the job does not reach the program whole, on one line, or
    /// its progress and its verdict are not what the row shows — or the
    /// passphrase itself crosses instead of its file's path.
    #[test]
    fn the_job_crosses_on_stdin_whole_and_its_secret_never_does() {
        let dir = place("crosses");
        let secrets = dir.join("secrets");
        let secret = crate::secrets::keep_in(&secrets, "horse battery").unwrap();
        let script = stand_in(&dir, "printf 'bytes 0/8\\nbytes 8/8\\ndone 8\\n'");
        let job = sealing(&dir, secret.clone());
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = shared.take(
            Engine::Seal,
            Work::Seal {
                job: Box::new(job.clone()),
                label: "note.txt".to_owned(),
            },
        );
        assert_eq!(
            carry(&shared, id, &switch, &script, &job, GRACE),
            Ending::Done
        );
        let read = std::fs::read_to_string(dir.join("job.json")).unwrap();
        assert_eq!(
            serde_json::from_str::<ronoctua_pq::Job>(read.trim()).unwrap(),
            job
        );
        assert!(!read.contains("horse"), "the passphrase crossed: {read}");
        let (jobs, _) = shared.snapshot();
        assert_eq!((jobs[0].done, jobs[0].total), (8, 8));
        // THE PROGRAM NEVER REMOVES IT: that is the job's end (`run`), through
        // `secrets::forget`, whose own tests prove what it removes.
        assert!(secret.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a refusal is not carried as its word (the row's sentence),
    /// or a program that dies with no verdict leaves its half behind — or,
    /// worse, a file somebody else made at that name is removed because the
    /// program refused to write over it.
    #[test]
    fn only_what_the_program_made_is_ever_removed() {
        let dir = place("removed");
        let job = sealing(&dir, dir.join("pw"));
        // REFUSED: its word, and the output (somebody's file) untouched.
        std::fs::write(&job.into, b"somebody's").unwrap();
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = shared.take(
            Engine::Seal,
            Work::Seal {
                job: Box::new(job.clone()),
                label: String::new(),
            },
        );
        let refuses = stand_in(&dir, "echo 'refused failed'; exit 1");
        assert_eq!(
            carry(&shared, id, &switch, &refuses, &job, GRACE),
            Ending::Refused(Why::Failed)
        );
        assert_eq!(std::fs::read(&job.into).unwrap(), b"somebody's");
        // DIED BEFORE IT MADE ANYTHING: the file there is not its own.
        let dies_early = stand_in(&dir, "kill -9 $$");
        assert!(matches!(
            carry(&shared, id, &switch, &dies_early, &job, GRACE),
            Ending::Failed(_)
        ));
        assert_eq!(std::fs::read(&job.into).unwrap(), b"somebody's");
        // DIED AFTER IT SAID IT MADE THE OUTPUT: the half goes.
        std::fs::remove_file(&job.into).unwrap();
        let dies_late = stand_in(
            &dir,
            &format!(
                "printf 'half' > '{}'; echo 'bytes 0/9'; kill -9 $$",
                job.into.display()
            ),
        );
        assert!(matches!(
            carry(&shared, id, &switch, &dies_late, &job, GRACE),
            Ending::Failed(_)
        ));
        assert!(!job.into.exists(), "the half of a dead seal was left");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: two sealing programs run at once — forty files sealed with a
    /// passphrase were forty scrypts of up to a gibibyte each, together —
    /// or the waiting ones start out of the order they were queued, or a
    /// stop does not reach a job that is still waiting (it would wait for
    /// the whole queue before it could be told).
    #[test]
    fn one_seal_runs_at_a_time_in_the_order_queued() {
        let turn = Arc::new(Turn::new());
        let first = turn
            .wait(1, Control::running())
            .expect("the first waits for nobody");
        let (started, order) = mpsc::channel();
        let mut waiting = Vec::new();
        for id in [3_u64, 2] {
            let (turn, started) = (Arc::clone(&turn), started.clone());
            waiting.push(std::thread::spawn(move || {
                let held = turn.wait(id, Control::running());
                started.send(id).unwrap();
                // HELD A MOMENT: the other must not start meanwhile.
                std::thread::sleep(Duration::from_millis(300));
                drop(held);
            }));
            // QUEUED IN THIS ORDER: 3 then 2 — the ids, not the arrival,
            // say who was taken first.
            std::thread::sleep(Duration::from_millis(100));
        }
        let stopped = Arc::new(Control::new());
        let (turn_of_four, stop) = (Arc::clone(&turn), Arc::clone(&stopped));
        let four = std::thread::spawn(move || turn_of_four.wait(4, &stop).is_none());
        assert!(
            order.recv_timeout(Duration::from_millis(600)).is_err(),
            "a seal started while another ran"
        );
        stopped.set(Flow::Stop);
        assert!(four.join().unwrap(), "a stop did not reach a waiting seal");
        drop(first);
        let ran: Vec<u64> = (0..2)
            .map(|_| order.recv_timeout(Duration::from_secs(10)).unwrap())
            .collect();
        assert_eq!(ran, [2, 3], "the waiting seals ran out of order");
        for w in waiting {
            w.join().unwrap();
        }
        assert!(
            turn.wait(5, Control::running()).is_some(),
            "the turn was not given back"
        );
    }

    /// FAILS IF: a stop does not stop the program, or leaves its half.
    #[test]
    fn a_stop_takes_the_program_and_its_half() {
        let dir = place("stop");
        let job = sealing(&dir, dir.join("pw"));
        let shared = Arc::new(Shared::fresh_one());
        let (id, switch) = shared.take(
            Engine::Seal,
            Work::Seal {
                job: Box::new(job.clone()),
                label: String::new(),
            },
        );
        let slow = stand_in(
            &dir,
            &format!(
                "printf 'half' > '{}'; echo 'bytes 0/9'; sleep 30",
                job.into.display()
            ),
        );
        let stopper = Arc::clone(&switch);
        let at = std::time::Instant::now();
        let stopping = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            stopper.set(Flow::Stop);
        });
        assert_eq!(
            carry(
                &shared,
                id,
                &switch,
                &slow,
                &job,
                Duration::from_millis(500)
            ),
            Ending::Cancelled
        );
        stopping.join().unwrap();
        assert!(at.elapsed() < Duration::from_secs(10), "{:?}", at.elapsed());
        assert!(!job.into.exists(), "the half of a stopped seal was left");
        let _ = std::fs::remove_dir_all(dir);
    }
}
