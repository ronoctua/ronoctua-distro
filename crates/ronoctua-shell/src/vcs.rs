//! The branch of the focused buffer's repository — §4.2's `context_of` segment.
//!
//! # Why this exists
//!
//! The segment was born with the branch icon and the text `milestone 4`: scaffolding that
//! leaked into the interface, and on top of that a string literal in the UI, which
//! invariant 8 forbids. The guards did not catch it because it is assembled with
//! `format!` in the shell, and does not go through `t()` — the same boundary
//! `CLAUDE.md` already records as a silent fallback.
//!
//! Now the segment shows what the icon promises. A branch name is **data**,
//! like the buffer's name, and that is why it has no translation key.
//!
//! # Why read the `.git` by hand, and not a git crate
//!
//! We need one line of one file. A git library would bring
//! object resolution, an index and networking to answer what `.git/HEAD`
//! answers in plain text — and it would land on the footer's startup path, which
//! has a 200 ms budget (§5.2).
//!
//! # Why there is no cache
//!
//! The read happens when the focus changes, not per frame. It is one `openat` and one
//! `read` of a few dozen bytes from a file piping hot in the kernel's cache;
//! a cache here would have to be invalidated by a `checkout`, and a wrong
//! branch on screen is worse than the read.

use std::path::{Path, PathBuf};

/// How many levels to climb looking for the repository.
///
/// A ceiling, and not `while let Some(parent)`: a deep working directory
/// inside a tree with no git would mean one `stat` per level up to the root, on
/// every focus change. Twenty covers any actual path.
const UP_CEILING: usize = 20;

/// The branch, from the content of a `.git/HEAD`.
///
/// A **pure** function: it is what the tests exercise. The alternative would be
/// finding out in a actual repository that a detached `HEAD` has no `ref:`
/// at all, and that the file ends with `\n`.
#[must_use]
pub fn branch_of(head: &str) -> Option<String> {
    let head = head.trim();
    if head.is_empty() {
        return None;
    }
    match head.strip_prefix("ref: ") {
        // `refs/heads/feature/x` — the branch's name has a slash, so one can NOT
        // take the last component. Only the prefix comes off.
        Some(r) => {
            let name = r.strip_prefix("refs/heads/").unwrap_or(r);
            (!name.is_empty()).then(|| name.to_owned())
        }
        // A detached HEAD: the file is the raw sha. Seven digits is what
        // git itself shows, and enough to recognise.
        None => {
            let sha: String = head.chars().take(7).collect();
            sha.chars()
                .all(|c| c.is_ascii_hexdigit())
                .then_some(sha)
                .filter(|s| s.len() == 7)
        }
    }
}

/// The actual `.git` directory, given whatever is in its place.
///
/// In a worktree and a submodule, `.git` is a FILE with `gitdir: <path>`
/// inside. Treating it as a directory makes the read fail silently, and the
/// segment disappears precisely for whoever uses worktrees — which is whoever wants it most.
fn git_directory(access_point_of: &Path) -> Option<PathBuf> {
    if access_point_of.is_dir() {
        return Some(access_point_of.to_path_buf());
    }
    let content = std::fs::read_to_string(access_point_of).ok()?;
    let target = content.trim().strip_prefix("gitdir:")?.trim();
    let target = Path::new(target);
    if target.is_absolute() {
        Some(target.to_path_buf())
    } else {
        // Relative to the directory containing the `.git`, not to the reader's cwd.
        Some(access_point_of.parent()?.join(target))
    }
}

/// The branch of the repository containing `dir`, climbing until it is found.
#[must_use]
pub fn branch_at(dir: &Path) -> Option<String> {
    let mut current = dir;
    for _ in 0..UP_CEILING {
        let git = git_directory(&current.join(".git"));
        if let Some(g) = git
            && let Ok(head) = std::fs::read_to_string(g.join("HEAD"))
            && let Some(r) = branch_of(&head)
        {
            return Some(r);
        }
        current = current.parent()?;
    }
    None
}

/// How many descendant levels to look at.
///
/// Two cover the actual case — terminal → shell — and the case with a launcher in
/// between. Descending without a limit would make a build tree (a `make -j`) cost
/// hundreds of reads on every focus change.
const DOWN_CEILING: usize = 2;

/// How many processes to visit in total. A `make -j32` has many children.
const PROCESS_CEILING: usize = 32;

/// The branch of a process's working directory **or of its children's**.
///
/// # Why the children matter
///
/// A Wayland client's `cwd` is where it was LAUNCHED, and in a terminal whoever
/// navigates is the shell inside it. Looking only at the client, the segment gets the
/// rare case right (an app opened already inside the repository) and the main one wrong —
/// which on a keyboard desktop is precisely the terminal.
///
/// Measured in the VM test: `foot` launched with `cwd = /`, the shell inside it
/// in the repository. The client alone would give `None`.
///
/// A pid that has already died returns `None`: the modeline degrades, it does not invent.
#[must_use]
pub fn branch_of_pid(pid: i32) -> Option<String> {
    branch_in_procfs(Path::new("/proc"), pid)
}

/// [`branch_of_pid`], with the `procfs` root made explicit.
///
/// The root is a parameter only so the test can build a fake `/proc` —
/// the same reason as the `estrangeiro` module's `origin_at`.
#[must_use]
pub fn branch_in_procfs(procfs: &Path, pid: i32) -> Option<String> {
    // Breadth first: the process, then its children, then its grandchildren.
    // The SHALLOWEST wins — with a `git` running inside the terminal, the shell is
    // the one with the cwd that matters, and it is closer.
    let mut fila = vec![pid];
    let mut seen = 0usize;
    for _ in 0..=DOWN_CEILING {
        let mut proxima = Vec::new();
        for p in fila {
            seen += 1;
            if seen > PROCESS_CEILING {
                return None;
            }
            if let Ok(cwd) = std::fs::read_link(procfs.join(p.to_string()).join("cwd"))
                && let Some(r) = branch_at(&cwd)
            {
                return Some(r);
            }
            proxima.extend(children_of(procfs, p));
        }
        if proxima.is_empty() {
            return None;
        }
        fila = proxima;
    }
    None
}

/// A process's direct children, through `procfs`.
///
/// `task/<tid>/children` and not walking the whole of `/proc` looking for `PPid`: it is
/// two small files against thousands. The file may not exist (the
/// kernel only exposes it with `CONFIG_PROC_CHILDREN`), and then the list is empty — the
/// segment disappears, which is the right degradation.
fn children_of(procfs: &Path, pid: i32) -> Vec<i32> {
    let tarefas = procfs.join(pid.to_string()).join("task");
    let Ok(inputs) = std::fs::read_dir(&tarefas) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in inputs.flatten() {
        let Ok(txt) = std::fs::read_to_string(e.path().join("children")) else {
            continue;
        };
        out.extend(
            txt.split_ascii_whitespace()
                .filter_map(|t| t.parse::<i32>().ok()),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{branch_at, branch_in_procfs, branch_of};

    #[test]
    fn reads_the_branch_of_a_normal_head() {
        assert_eq!(branch_of("ref: refs/heads/main\n"), Some("main".to_owned()));
    }

    #[test]
    fn a_branch_with_a_slash_is_not_cut() {
        // `rsplit('/')` would give `x`. The branch's name is everything after
        // `refs/heads/`, and a truncated `feature/x` would be wrong without looking it.
        assert_eq!(
            branch_of("ref: refs/heads/feature/algo\n"),
            Some("feature/algo".to_owned())
        );
    }

    #[test]
    fn a_detached_head_becomes_a_short_sha() {
        // With no `ref:` the file is the raw sha. Returning `None` would make the segment
        // disappear during a rebase or a bisect — exactly when knowing
        // where one is matters most.
        assert_eq!(
            branch_of("9f3a2b1c4d5e6f708192a3b4c5d6e7f809a1b2c3\n"),
            Some("9f3a2b1".to_owned())
        );
    }

    #[test]
    fn junk_does_not_become_a_branch() {
        assert_eq!(branch_of(""), None);
        assert_eq!(branch_of("   \n"), None);
        assert_eq!(branch_of("ref: "), None);
        // Too short to be a sha, and it is not a `ref:`.
        assert_eq!(branch_of("abc"), None);
        // It has the length, but it is not hexadecimal.
        assert_eq!(branch_of("zzzzzzz"), None);
    }

    /// Builds a fake repository: only `.git/HEAD`, which is all that gets read.
    fn repo(root: &std::path::Path, head: &str) {
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git").join("HEAD"), head).unwrap();
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ronoctua-vcs-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn finds_the_repository_by_walking_up() {
        let root = tmp("subindo");
        repo(&root, "ref: refs/heads/trabalho\n");
        let background = root.join("a").join("b").join("c");
        std::fs::create_dir_all(&background).unwrap();
        assert_eq!(branch_at(&background), Some("trabalho".to_owned()));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn outside_a_repository_there_is_no_branch() {
        // The segment simply does not appear. Inventing "no git" would be putting
        // a string literal in the UI — the defect this module fixes.
        let root = tmp("sem-git");
        assert_eq!(branch_at(&root), None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_worktree_has_git_as_a_file() {
        // In a worktree and a submodule the `.git` is a file, with `gitdir:` inside.
        // Treating it as a directory makes the read fail silently, and the
        // segment disappears for whoever wants it most.
        let root = tmp("worktree");
        let actual = root.join("git-de-verdade");
        std::fs::create_dir_all(&actual).unwrap();
        std::fs::write(actual.join("HEAD"), "ref: refs/heads/outro\n").unwrap();

        let tree = root.join("arvore");
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(tree.join(".git"), format!("gitdir: {}\n", actual.display())).unwrap();

        assert_eq!(branch_at(&tree), Some("outro".to_owned()));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_relative_gitdir_resolves_against_the_dot_and_not_the_cwd() {
        // A `gitdir: ../.git/worktrees/x` resolved against the reader's cwd
        // would point anywhere — and the footer runs from a cwd that is not
        // the buffer's.
        let root = tmp("gitdir-relativo");
        let actual = root.join("interno");
        std::fs::create_dir_all(&actual).unwrap();
        std::fs::write(actual.join("HEAD"), "ref: refs/heads/relativo\n").unwrap();

        let tree = root.join("arvore");
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::write(tree.join(".git"), "gitdir: ../interno\n").unwrap();

        assert_eq!(branch_at(&tree), Some("relativo".to_owned()));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_branch_comes_from_the_process_cwd() {
        let root = tmp("procfs");
        let work = root.join("trabalho");
        std::fs::create_dir_all(&work).unwrap();
        repo(&work, "ref: refs/heads/do-processo\n");

        let procfs = root.join("proc");
        std::fs::create_dir_all(procfs.join("4242")).unwrap();
        std::os::unix::fs::symlink(&work, procfs.join("4242").join("cwd")).unwrap();

        assert_eq!(
            branch_in_procfs(&procfs, 4242),
            Some("do-processo".to_owned())
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_branch_comes_from_the_child_when_the_client_is_outside() {
        // The main case: the terminal was launched in `/`, and whoever navigated
        // to the repository was the shell inside it. Looking only at the client, the
        // segment would never appear in a terminal — which on a keyboard
        // desktop is precisely the most-used buffer.
        let root = tmp("filho");
        let work = root.join("trabalho");
        std::fs::create_dir_all(&work).unwrap();
        repo(&work, "ref: refs/heads/do-filho\n");
        let fora_de_repo = root.join("neutro");
        std::fs::create_dir_all(&fora_de_repo).unwrap();

        let procfs = root.join("proc");
        // The client: a cwd outside a repository, and one child.
        let parent = procfs.join("100");
        std::fs::create_dir_all(parent.join("task").join("100")).unwrap();
        std::os::unix::fs::symlink(&fora_de_repo, parent.join("cwd")).unwrap();
        std::fs::write(parent.join("task").join("100").join("children"), "200\n").unwrap();
        // The shell: a cwd inside the repository.
        let child = procfs.join("200");
        std::fs::create_dir_all(&child).unwrap();
        std::os::unix::fs::symlink(&work, child.join("cwd")).unwrap();

        assert_eq!(branch_in_procfs(&procfs, 100), Some("do-filho".to_owned()));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_client_beats_the_child() {
        // The SHALLOWEST wins: a `git` running inside the terminal must not
        // change what the modeline shows.
        let root = tmp("raso");
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        repo(&a, "ref: refs/heads/do-cliente\n");
        repo(&b, "ref: refs/heads/do-filho\n");

        let procfs = root.join("proc");
        let parent = procfs.join("100");
        std::fs::create_dir_all(parent.join("task").join("100")).unwrap();
        std::os::unix::fs::symlink(&a, parent.join("cwd")).unwrap();
        std::fs::write(parent.join("task").join("100").join("children"), "200\n").unwrap();
        let child = procfs.join("200");
        std::fs::create_dir_all(&child).unwrap();
        std::os::unix::fs::symlink(&b, child.join("cwd")).unwrap();

        assert_eq!(
            branch_in_procfs(&procfs, 100),
            Some("do-cliente".to_owned())
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_pid_that_died_brings_nothing_down() {
        // It happens every time a window closes between the event and the read.
        let empty = std::env::temp_dir().join("ronoctua-vcs-procfs-vazio");
        assert_eq!(branch_in_procfs(&empty, 999_999), None);
    }
}
