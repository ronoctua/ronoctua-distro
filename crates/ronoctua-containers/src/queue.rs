//! What goes on the transfer queue: pulling, building and compose.
//!
//! # Why the queue and not a thread of the door
//!
//! A pull is minutes of network and a build is minutes of processor, and
//! either outlives the window that asked for it. The queue
//! (`ronoctua-transfers`, `Work::Container`) owns the child, its group and
//! its cancel; this crate owns what the child IS — its arguments, by the one
//! podman ([`crate::Podman`]) — and what its lines mean ([`step_of`]).
//!
//! # Progress, from what podman prints
//!
//! Nothing here invents a percentage. A build says where it is
//! (`STEP 3/7: RUN …`), so its row counts steps; a pull says each layer it
//! copies (`Copying blob sha256:… done`), so its row counts layers with no
//! total; compose says the commands it runs, so its row says the last one.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// One piece of work on the person's containers, as the queue carries it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "kebab-case")]
pub enum Job {
    /// Pull an image by its name (`docker.io/library/nginx:latest`,
    /// `nginx`).
    Pull {
        /// The name, as the person wrote it.
        image: String,
    },
    /// Build an image from a folder.
    Build {
        /// The folder the build sees.
        context: PathBuf,
        /// The Containerfile, when it is not the folder's own.
        file: Option<PathBuf>,
        /// The name the image gets.
        tag: String,
    },
    /// `compose up --detach`: make and start a project's containers.
    ComposeUp {
        /// The project's name, as its containers' labels say it.
        project: String,
        /// Its compose files, in compose's order (`ComposeProject::config_files`).
        files: Vec<PathBuf>,
    },
    /// `compose down`: stop and REMOVE a project's containers — after a
    /// question with "no" first, the door's.
    ComposeDown {
        /// The project's name.
        project: String,
        /// Its compose files.
        files: Vec<PathBuf>,
    },
    /// `compose pull`: fetch the images a project names.
    ComposePull {
        /// The project's name.
        project: String,
        /// Its compose files.
        files: Vec<PathBuf>,
    },
}

impl Job {
    /// Every kind of job, by its word — for the census of texts.
    pub const WORDS: [&'static str; 5] = [
        "pull",
        "build",
        "compose-up",
        "compose-down",
        "compose-pull",
    ];

    /// Its word: the suffix of its text key.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Pull { .. } => "pull",
            Self::Build { .. } => "build",
            Self::ComposeUp { .. } => "compose-up",
            Self::ComposeDown { .. } => "compose-down",
            Self::ComposePull { .. } => "compose-pull",
        }
    }

    /// podman's arguments for it. Pure. A name or a path the person gave
    /// always comes after `--`, or as the value of a flag, so none is read
    /// as an option.
    #[must_use]
    pub fn arguments(&self) -> Vec<String> {
        let owned = |all: &[&str]| all.iter().map(|&s| s.to_owned()).collect::<Vec<_>>();
        let path = |p: &PathBuf| p.to_string_lossy().into_owned();
        match self {
            Self::Pull { image } => {
                let mut args = owned(&["pull", "--"]);
                args.push(image.clone());
                args
            }
            Self::Build { context, file, tag } => {
                let mut args = owned(&["build", "--tag"]);
                args.push(tag.clone());
                if let Some(file) = file {
                    args.push("--file".to_owned());
                    args.push(path(file));
                }
                args.push("--".to_owned());
                args.push(path(context));
                args
            }
            // `podman compose` hands everything after it to the provider
            // (`podman-compose`), which reads its options before the verb.
            // EVERY FILE, and THE PROJECT'S OWN NAME: a project made with an
            // override, or under `--project-name`, is another project when
            // given only its first file or the folder's name.
            Self::ComposeUp { project, files } => {
                let mut args = compose(project, files);
                args.extend(owned(&["up", "--detach"]));
                args
            }
            Self::ComposeDown { project, files } => {
                let mut args = compose(project, files);
                args.push("down".to_owned());
                args
            }
            Self::ComposePull { project, files } => {
                let mut args = compose(project, files);
                args.push("pull".to_owned());
                args
            }
        }
    }

    /// The folder it runs in: the first compose file's own (compose resolves
    /// the paths in it from there), a build's context, and none for a pull.
    /// Never an empty path — `current_dir("")` fails the job before podman
    /// starts.
    #[must_use]
    pub fn working_dir(&self) -> Option<PathBuf> {
        match self {
            Self::Pull { .. } => None,
            Self::Build { context, .. } => Some(context.clone()),
            Self::ComposeUp { files, .. }
            | Self::ComposeDown { files, .. }
            | Self::ComposePull { files, .. } => files
                .first()
                .and_then(|f| f.parent())
                .filter(|d| !d.as_os_str().is_empty())
                .map(std::path::Path::to_path_buf),
        }
    }
}

/// `compose`, the project's name and each of its files, as options.
fn compose(project: &str, files: &[PathBuf]) -> Vec<String> {
    let mut args = vec![
        "compose".to_owned(),
        "--project-name".to_owned(),
        project.to_owned(),
    ];
    for file in files {
        args.push("--file".to_owned());
        args.push(file.to_string_lossy().into_owned());
    }
    args
}

/// The name a build gets when the person names only the folder: the
/// folder's own name, lower-case, under `localhost/` — podman's own
/// namespace for an image nobody pushed. `None` for a folder with no name a
/// tag may carry.
#[must_use]
pub fn tag_of_folder(context: &std::path::Path) -> Option<String> {
    let name = context.file_name()?.to_str()?;
    let tag: String = name
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let tag = tag.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    (!tag.is_empty()).then(|| format!("localhost/{tag}:latest"))
}

/// The Containerfile a build of `context` reads: `Containerfile`, else
/// `Dockerfile` — podman's own order. `None` when the folder has neither,
/// which the door says instead of queueing a build that cannot start.
#[must_use]
pub fn containerfile_in(context: &std::path::Path) -> Option<PathBuf> {
    ["Containerfile", "Dockerfile"]
        .iter()
        .map(|n| context.join(n))
        .find(|p| p.is_file())
}

/// What one line of a job's output says of its progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A build's step `done` of `total`.
    Of {
        /// The step it is on.
        done: u64,
        /// How many there are.
        total: u64,
    },
    /// A layer being copied — counted, with no total.
    Layer,
}

/// A line of `pull`, `build` or compose, read for progress. Pure.
#[must_use]
pub fn step_of(line: &str) -> Option<Step> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("STEP ") {
        let (at, _) = rest.split_once(':')?;
        let (done, total) = at.split_once('/')?;
        let (done, total) = (done.trim().parse().ok()?, total.trim().parse().ok()?);
        return (done <= total && total > 0).then_some(Step::Of { done, total });
    }
    line.starts_with("Copying blob ").then_some(Step::Layer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_job_is_its_arguments_and_a_name_is_never_an_option() {
        let pull = Job::Pull {
            image: "-x".to_owned(),
        };
        assert_eq!(pull.arguments(), ["pull", "--", "-x"]);
        let build = Job::Build {
            context: PathBuf::from("/home/someone/app"),
            file: Some(PathBuf::from("/home/someone/app/Containerfile.dev")),
            tag: "localhost/app:latest".to_owned(),
        };
        assert_eq!(
            build.arguments(),
            [
                "build",
                "--tag",
                "localhost/app:latest",
                "--file",
                "/home/someone/app/Containerfile.dev",
                "--",
                "/home/someone/app"
            ]
        );
        let files = vec![
            PathBuf::from("/home/someone/site/compose.yaml"),
            PathBuf::from("/home/someone/site/compose.override.yaml"),
        ];
        let up = Job::ComposeUp {
            project: "site".to_owned(),
            files: files.clone(),
        };
        assert_eq!(
            up.arguments(),
            [
                "compose",
                "--project-name",
                "site",
                "--file",
                "/home/someone/site/compose.yaml",
                "--file",
                "/home/someone/site/compose.override.yaml",
                "up",
                "--detach"
            ]
        );
        let down = Job::ComposeDown {
            project: "site".to_owned(),
            files: files.clone(),
        };
        assert_eq!(
            down.working_dir(),
            Some(PathBuf::from("/home/someone/site"))
        );
        // A BARE NAME has no folder to run in, and is never given one that
        // is an empty path.
        assert_eq!(
            Job::ComposePull {
                project: "site".to_owned(),
                files: vec![PathBuf::from("compose.yaml")],
            }
            .working_dir(),
            None
        );
        // EVERY WORD IS LISTED, once.
        let words: std::collections::BTreeSet<&str> = [
            pull.word(),
            build.word(),
            up.word(),
            down.word(),
            Job::ComposePull {
                project: "site".to_owned(),
                files,
            }
            .word(),
        ]
        .into_iter()
        .collect();
        assert_eq!(words.len(), Job::WORDS.len());
        assert!(words.iter().all(|w| Job::WORDS.contains(w)));
    }

    #[test]
    fn a_job_crosses_the_socket_whole() {
        let job = Job::Build {
            context: PathBuf::from("/home/someone/app"),
            file: None,
            tag: "localhost/app:latest".to_owned(),
        };
        let wire = serde_json::to_string(&job).unwrap();
        assert!(wire.contains(r#""do":"build""#), "{wire}");
        assert_eq!(serde_json::from_str::<Job>(&wire).unwrap(), job);
    }

    #[test]
    fn a_line_says_how_far_and_nothing_is_invented() {
        assert_eq!(
            step_of("STEP 3/7: RUN apk add curl"),
            Some(Step::Of { done: 3, total: 7 })
        );
        assert_eq!(
            step_of(
                "Copying blob sha256:6e771e15690e2fabf2332d3a3b744495411d6e0b00b2aea64419b58b0066cf81"
            ),
            Some(Step::Layer)
        );
        for nothing in ["STEP 9/7: nonsense", "STEP x/y:", "Writing manifest", ""] {
            assert_eq!(step_of(nothing), None, "{nothing}");
        }
    }

    #[test]
    fn a_folder_names_its_image() {
        assert_eq!(
            tag_of_folder(std::path::Path::new("/home/someone/My App")).as_deref(),
            Some("localhost/my-app:latest")
        );
        assert_eq!(tag_of_folder(std::path::Path::new("/")), None);
        assert_eq!(
            tag_of_folder(std::path::Path::new("/home/someone/---")),
            None
        );
    }
}
