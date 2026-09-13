//! The one boundary with Syncthing.
//!
//! # Event, not clock
//!
//! `GET /rest/events?since=N&timeout=60` is a LONG POLL: it holds the
//! connection open and answers the moment something happens. Measured against
//! Syncthing 2.1.3, which is the version this flake pins: an idle poll blocks
//! for the whole minute and returns an empty list, a real change comes back in
//! **17 ms**, and the blocked client spends **zero** CPU ticks over 25 seconds.
//!
//! So there is no new timer anywhere in this system, and nothing here is asked
//! "what is it now?" on a schedule. A thread parked in a `read` is the whole
//! mechanism.
//!
//! # Why a change re-reads everything instead of merging the event
//!
//! Because the merge is where the bugs live. The sound view learned this the
//! expensive way: PipeWire's `metadata` array arrives PARTIAL, replacing it
//! whole silently deleted three entries, and the only way anybody noticed was
//! comparing two photographs of the same screen taken minutes apart.
//!
//! Syncthing's events carry the same hazard in more shapes — twenty event
//! types, some with the whole object and some with one field. Re-reading is
//! ten calls of a few milliseconds each on the loopback, and it happens only
//! when something actually changed. The correctness is worth far more than the
//! milliseconds, and [`FLOOR`] keeps a busy transfer from turning it into a
//! loop.
//!
//! # What this crate does NOT keep
//!
//! Any of Syncthing's configuration. Syncthing persists all of it, and a second
//! copy would be a second truth. [`prefs`] holds exactly one thing, which is
//! ours and not its: whether the modeline draws a mark.

pub mod options;
pub mod prefs;
pub mod rest;
pub mod unit;

use std::time::{Duration, Instant};

/// The least time between two full re-reads.
///
/// During a transfer Syncthing emits `DownloadProgress` about once a second,
/// and each one is a change. Without a floor a big sync would be a re-read per
/// event; with it, the screen is at most this far behind and the loop cannot
/// run away. Half a second is under the eye's patience for a progress figure.
pub const FLOOR: Duration = Duration::from_millis(500);

/// How long to wait before trying again after the service refused to talk.
///
/// It climbs to [`MOST_REST`]. A service that is down stays down until somebody
/// starts it, and hammering a socket nobody is listening on is the shape of a
/// daemon this project would not ship.
pub const FIRST_REST: Duration = Duration::from_secs(1);

/// The longest the watching thread ever sleeps between attempts.
pub const MOST_REST: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// What the screen is shown
// ---------------------------------------------------------------------------

/// Whether Syncthing is there at all, and what it says about itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Service {
    /// It did not answer. The unit may be stopped, or starting.
    ///
    /// AND IT IS THE DEFAULT, which is the honest starting point: a snapshot
    /// nobody has filled in is a service nobody has heard from.
    #[default]
    Down,
    /// It answered.
    Up(Box<Running>),
}

impl Service {
    /// Whether it answered.
    #[must_use]
    pub const fn is_up(&self) -> bool {
        matches!(self, Self::Up(_))
    }

    /// What it said, when it said anything.
    #[must_use]
    pub const fn running(&self) -> Option<&Running> {
        match self {
            Self::Up(r) => Some(r),
            Self::Down => None,
        }
    }
}

/// What a running Syncthing says about itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Running {
    /// `v2.1.3`.
    pub version: String,
    /// This machine's own device id.
    pub id: String,
    /// Seconds since it started.
    pub uptime: u64,
    /// Where it is listening for other machines.
    pub listeners: Vec<String>,
    /// Whatever it is currently complaining about.
    pub errors: Vec<String>,
    /// Where the web interface is, for whoever goes to open it.
    pub address: String,
}

/// What a folder is doing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum State {
    /// Up to date, nothing to do.
    #[default]
    Idle,
    /// Looking at the disk.
    Scanning,
    /// Moving files.
    Syncing,
    /// Working out what to move.
    Preparing,
    /// Tidying old versions away.
    Cleaning,
    /// Stopped by the person.
    Paused,
    /// Something is wrong, and the folder says what.
    Failing,
    /// Syncthing named a state this build does not know.
    ///
    /// NOT an error and NOT a blank: a release that adds a state should take
    /// one word off the screen, never the row.
    Unknown,
}

impl State {
    /// The id, which is both what Syncthing says and the stem of the text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Scanning => "scanning",
            Self::Syncing => "syncing",
            Self::Preparing => "preparing",
            Self::Cleaning => "cleaning",
            Self::Paused => "paused",
            Self::Failing => "failing",
            Self::Unknown => "unknown",
        }
    }

    /// What Syncthing's own word means here.
    #[must_use]
    pub fn of(said: &str) -> Self {
        match said {
            "idle" => Self::Idle,
            "scanning" | "scan-waiting" => Self::Scanning,
            "syncing" => Self::Syncing,
            "sync-preparing" | "sync-waiting" => Self::Preparing,
            "cleaning" | "cleaning-waiting" => Self::Cleaning,
            "paused" => Self::Paused,
            "error" => Self::Failing,
            _ => Self::Unknown,
        }
    }
}

/// One folder.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Folder {
    /// Syncthing's id for it, which is what every call names.
    pub id: String,
    /// What the person called it. Empty means the id is the name.
    pub label: String,
    /// Where it is on this disk.
    pub path: String,
    /// `sendreceive`, `sendonly`, `receiveonly`, `receiveencrypted`.
    pub kind: String,
    /// Stopped by the person.
    pub paused: bool,
    /// What it is doing.
    pub state: State,
    /// How much of it is here, from 0 to 100.
    pub completion: u8,
    /// How big the folder is across every machine.
    pub global_bytes: u64,
    /// How much of that is on this one.
    pub local_bytes: u64,
    /// How much is still to come.
    pub need_bytes: u64,
    /// How many items are still to come.
    pub need_items: u64,
    /// How many things it could not do.
    pub errors: u64,
    /// The devices it is shared with.
    pub devices: Vec<String>,
    /// The configuration object, whole, for the option sheet.
    ///
    /// The sheet draws ninety-odd options off a declared table, and a struct
    /// with a field per option would be that table written a second time — in
    /// a place where nothing could check the two agree.
    pub raw: serde_json::Value,
}

impl Folder {
    /// What to call it on screen.
    #[must_use]
    pub fn name(&self) -> &str {
        if self.label.is_empty() {
            &self.id
        } else {
            &self.label
        }
    }
}

/// One machine, this one included.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Device {
    /// The 63-character id.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// Stopped by the person, or by a whole-system pause.
    pub paused: bool,
    /// Talking to us right now.
    pub connected: bool,
    /// Where from, when connected.
    pub address: String,
    /// What it is running, when it says.
    pub client: String,
    /// On the same network as this one.
    pub local: bool,
    /// Bytes we sent it since this Syncthing started.
    pub sent: u64,
    /// Bytes it sent us.
    pub received: u64,
    /// Whether this row IS this machine.
    pub me: bool,
    /// The folders shared with it.
    pub folders: Vec<String>,
    /// The configuration object, whole, for the option sheet.
    pub raw: serde_json::Value,
}

impl Device {
    /// What to call it on screen.
    #[must_use]
    pub fn label(&self) -> &str {
        if self.name.is_empty() {
            &self.id
        } else {
            &self.name
        }
    }
}

/// Somebody asking to be let in.
///
/// This is the one thing that, until now, ONLY the web page could show — and
/// it is the one thing that cannot wait, because the other end is a person
/// standing at another machine wondering whether it worked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invitation {
    /// An unknown machine tried to connect.
    Device {
        /// Its id.
        id: String,
        /// What it calls itself.
        name: String,
        /// Where it called from.
        address: String,
    },
    /// A known machine offered a folder we do not have.
    Folder {
        /// The folder's id.
        folder: String,
        /// What they call it.
        label: String,
        /// Which machine offered it.
        from: String,
        /// That machine's name, when we know one.
        who: String,
    },
}

/// Bytes in and out since the service started.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Traffic {
    /// Sent.
    pub sent: u64,
    /// Received.
    pub received: u64,
}

/// Everything the screen is drawn from, in one value.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// Whether it is there.
    pub service: Service,
    /// The folders, in the order Syncthing keeps them.
    pub folders: Vec<Folder>,
    /// The machines, this one first when it is known.
    pub devices: Vec<Device>,
    /// Who is asking to be let in.
    pub invitations: Vec<Invitation>,
    /// `/rest/config/options`, whole, for the option sheet.
    pub options: serde_json::Value,
    /// `/rest/config/gui`, whole — minus the key, which never comes here.
    pub gui: serde_json::Value,
    /// `/rest/config/defaults/folder`, for the sheet and for new folders.
    pub folder_defaults: serde_json::Value,
    /// `/rest/config/defaults/device`.
    pub device_defaults: serde_json::Value,
    /// How much of everything is here, from 0 to 100.
    pub completion: u8,
    /// Bytes both ways.
    pub traffic: Traffic,
    /// Whether the systemd unit is up, when we could tell.
    pub unit: unit::Standing,
}

impl Snapshot {
    /// Whether anything can travel right now.
    ///
    /// This is the whole meaning of the modeline mark, and the owner chose it:
    /// paused and stopped are, from the other side of the screen, the same
    /// fact — nothing is going anywhere. The view is what says which.
    ///
    /// A machine with nothing set up yet is NOT "not syncing". It has nothing
    /// to sync, and a mark that came on at first boot and stayed on until the
    /// person did something nobody asked them to do would be a mark that means
    /// nothing by the second day.
    #[must_use]
    pub fn syncing(&self) -> bool {
        if !self.service.is_up() {
            return false;
        }
        if self.folders.is_empty() && self.others().count() == 0 {
            return true;
        }
        let every_device_paused = {
            let mut others = self.others().peekable();
            others.peek().is_some() && self.others().all(|d| d.paused)
        };
        let every_folder_paused = !self.folders.is_empty() && self.folders.iter().all(|f| f.paused);
        !(every_device_paused || every_folder_paused)
    }

    /// Every machine but this one.
    ///
    /// This machine is in Syncthing's device list, and counting it would make
    /// "no other machines yet" read as "one device, and it is fine".
    pub fn others(&self) -> impl Iterator<Item = &Device> {
        self.devices.iter().filter(|d| !d.me)
    }

    /// How many other machines are talking to us.
    #[must_use]
    pub fn connected(&self) -> usize {
        self.others().filter(|d| d.connected).count()
    }

    /// Whether every other machine is paused.
    #[must_use]
    pub fn paused(&self) -> bool {
        let mut others = self.others().peekable();
        others.peek().is_some() && self.others().all(|d| d.paused)
    }

    /// The configuration object one scope points at.
    #[must_use]
    pub fn value_at(&self, scope: options::Scope, at: Option<&str>) -> Option<&serde_json::Value> {
        match scope {
            options::Scope::Global => Some(&self.options),
            options::Scope::Interface => Some(&self.gui),
            options::Scope::FolderDefaults => Some(&self.folder_defaults),
            options::Scope::DeviceDefaults => Some(&self.device_defaults),
            options::Scope::Folder => {
                let at = at?;
                self.folders.iter().find(|f| f.id == at).map(|f| &f.raw)
            }
            options::Scope::Device => {
                let at = at?;
                self.devices.iter().find(|d| d.id == at).map(|d| &d.raw)
            }
        }
    }

    /// One option's current value.
    ///
    /// THROUGH `Setting::read`, which is what honours `within`. This used to
    /// call `.get(key)` on the object itself, and for a nested option that
    /// reads the WRONG FIELD — `versioning.type` and a folder's own `type` are
    /// both spelt `type`, so the row for "keep old versions" showed "send and
    /// receive", and turning it would have computed the next value from the
    /// folder's kind. Nothing asserted it; it came off a printed screen.
    #[must_use]
    pub fn setting(
        &self,
        setting: &options::Setting,
        at: Option<&str>,
    ) -> Option<&serde_json::Value> {
        setting.read(self.value_at(setting.scope, at)?)
    }
}

// ---------------------------------------------------------------------------
// What the screen asks for
// ---------------------------------------------------------------------------

/// Something to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    /// Stop talking to one machine, or to all of them when `None`.
    Pause(Option<String>),
    /// Start again.
    Resume(Option<String>),
    /// Stop a folder.
    PauseFolder(String),
    /// Start it again.
    ResumeFolder(String),
    /// Look at the disk now instead of at the next interval.
    Rescan(String),
    /// Send this machine's copy out as the truth. Send-only folders.
    Override(String),
    /// Throw this machine's changes away. Receive-only folders.
    Revert(String),
    /// Let a machine in.
    AcceptDevice {
        /// Its id.
        id: String,
        /// What to call it.
        name: String,
    },
    /// Say no, and forget it asked.
    RejectDevice(String),
    /// Take a folder somebody offered.
    AcceptFolder {
        /// The folder's id.
        folder: String,
        /// Who offered it.
        from: String,
        /// What to call it.
        label: String,
        /// Where to put it.
        path: String,
    },
    /// Say no to an offered folder.
    RejectFolder {
        /// The folder's id.
        folder: String,
        /// Who offered it.
        from: String,
    },
    /// Remove a machine.
    ForgetDevice(String),
    /// Remove a folder. The files stay on the disk; only the sharing goes.
    ForgetFolder(String),
    /// Add a machine by hand.
    AddDevice {
        /// The 63-character id.
        id: String,
        /// What to call it.
        name: String,
    },
    /// Add a folder by hand.
    AddFolder {
        /// The id, which the person may leave to Syncthing.
        id: String,
        /// What to call it.
        label: String,
        /// Where it is.
        path: String,
    },
    /// Share a folder with a machine, or stop sharing it.
    Share {
        /// The folder.
        folder: String,
        /// The machine.
        device: String,
        /// Whether it should be shared after this.
        with: bool,
    },
    /// Write one option.
    ///
    /// The BODY is already shaped, by `Setting::written` — which is the one
    /// place that knows whether a key sits inside a nested object. Passing the
    /// key and the value separately would put that knowledge here as well, and
    /// two places that both know how to nest is one place that will get it
    /// wrong.
    Set {
        /// The option's id, so that a refusal can name it on screen.
        what: String,
        /// Which configuration object.
        scope: options::Scope,
        /// Which folder or device, when the scope needs one.
        at: Option<String>,
        /// The PATCH body, whole.
        body: serde_json::Value,
    },
    /// Restart the service. Syncthing does this to itself; no privilege.
    Restart,
    /// Start the systemd unit.
    Start,
    /// Stop the systemd unit.
    Stop,
}

/// One HTTP call, worked out from an [`Order`] with nothing running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Errand {
    /// The verb.
    pub method: &'static str,
    /// The path, query string included.
    pub path: String,
    /// The body, when there is one.
    pub body: Option<String>,
}

/// The configuration an order must read BEFORE it can be worked out.
///
/// Adding a device or a folder means writing a whole object, and the object it
/// should start from is Syncthing's own defaults — which
/// `nix/modules/syncthing.nix` has already tuned (`numConnections`,
/// `copyRangeMethod`, `pullerMaxPendingKiB`). Building one from scratch here
/// would quietly throw those measurements away on every folder the person adds.
#[must_use]
pub const fn seed_of(order: &Order) -> Option<options::Scope> {
    match order {
        Order::AddDevice { .. } | Order::AcceptDevice { .. } => {
            Some(options::Scope::DeviceDefaults)
        }
        Order::AddFolder { .. } | Order::AcceptFolder { .. } => {
            Some(options::Scope::FolderDefaults)
        }
        _ => None,
    }
}

/// Percent-encode what goes in a query string.
///
/// A folder id may hold anything the person typed, and a label with a space or
/// an ampersand in it would otherwise cut the query in half — which Syncthing
/// answers with a refusal about a folder that does not exist, naming a folder
/// that is only the first half of the real one.
#[must_use]
pub fn escaped(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// What an order comes down to, given whatever seed it asked for.
///
/// Pure — no socket, no service, no clock. This is where every path in this
/// crate is decided, and it is the whole of what a test needs to hold them
/// down.
#[must_use]
pub fn errand_of(order: &Order, seed: Option<&serde_json::Value>) -> Errand {
    let post = |path: String| Errand {
        method: "POST",
        path,
        body: None,
    };
    let patch = |path: String, body: serde_json::Value| Errand {
        method: "PATCH",
        path,
        body: Some(body.to_string()),
    };
    match order {
        Order::Pause(who) => post(match who {
            Some(id) => rest::pause_one(id),
            None => rest::PAUSE.to_owned(),
        }),
        Order::Resume(who) => post(match who {
            Some(id) => rest::resume_one(id),
            None => rest::RESUME.to_owned(),
        }),
        // A FOLDER'S PAUSE IS CONFIGURATION, and a device's is not.
        //
        // `/rest/system/pause` is runtime and forgotten on restart; a folder
        // has a `paused` field that persists. Two different mechanisms wearing
        // one word on the screen, and getting them the wrong way round makes a
        // paused folder come back by itself after a reboot.
        Order::PauseFolder(id) => patch(rest::a_folder(id), serde_json::json!({ "paused": true })),
        Order::ResumeFolder(id) => {
            patch(rest::a_folder(id), serde_json::json!({ "paused": false }))
        }
        Order::Rescan(id) => post(rest::scan(id)),
        Order::Override(id) => post(rest::override_it(id)),
        Order::Revert(id) => post(rest::revert(id)),
        Order::AcceptDevice { id, name } | Order::AddDevice { id, name } => {
            let mut object = seed.cloned().unwrap_or_else(|| serde_json::json!({}));
            if let Some(map) = object.as_object_mut() {
                map.insert("deviceID".to_owned(), serde_json::json!(id));
                map.insert("name".to_owned(), serde_json::json!(name));
            }
            Errand {
                method: "PUT",
                path: rest::a_device(id),
                body: Some(object.to_string()),
            }
        }
        Order::RejectDevice(id) => Errand {
            method: "DELETE",
            path: rest::pending_device(id),
            body: None,
        },
        Order::AcceptFolder {
            folder,
            from,
            label,
            path,
        } => {
            let mut object = seed.cloned().unwrap_or_else(|| serde_json::json!({}));
            if let Some(map) = object.as_object_mut() {
                map.insert("id".to_owned(), serde_json::json!(folder));
                map.insert("label".to_owned(), serde_json::json!(label));
                map.insert("path".to_owned(), serde_json::json!(path));
                // AND THE MACHINE THAT OFFERED IT IS IN IT FROM THE START.
                // Accepting a folder and then not sharing it back is a folder
                // that exists on both machines and never moves a byte.
                map.insert(
                    "devices".to_owned(),
                    serde_json::json!([{ "deviceID": from }]),
                );
            }
            Errand {
                method: "PUT",
                path: rest::a_folder(folder),
                body: Some(object.to_string()),
            }
        }
        Order::RejectFolder { folder, from } => Errand {
            method: "DELETE",
            path: rest::pending_folder(folder, from),
            body: None,
        },
        Order::AddFolder { id, label, path } => {
            let mut object = seed.cloned().unwrap_or_else(|| serde_json::json!({}));
            if let Some(map) = object.as_object_mut() {
                map.insert("id".to_owned(), serde_json::json!(id));
                map.insert("label".to_owned(), serde_json::json!(label));
                map.insert("path".to_owned(), serde_json::json!(path));
            }
            Errand {
                method: "PUT",
                path: rest::a_folder(id),
                body: Some(object.to_string()),
            }
        }
        Order::ForgetDevice(id) => Errand {
            method: "DELETE",
            path: rest::a_device(id),
            body: None,
        },
        Order::ForgetFolder(id) => Errand {
            method: "DELETE",
            path: rest::a_folder(id),
            body: None,
        },
        Order::Share {
            folder,
            device,
            with,
        } => {
            // THE WHOLE LIST GOES BACK, because `devices` is an array and a
            // PATCH of an array REPLACES it. The seed here is the folder's own
            // current object; without it this would empty the sharing list and
            // the change would read as "it unshared everything".
            let mut list: Vec<serde_json::Value> = seed
                .and_then(|f| f.get("devices"))
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
            list.retain(|d| d.get("deviceID").and_then(serde_json::Value::as_str) != Some(device));
            if *with {
                list.push(serde_json::json!({ "deviceID": device }));
            }
            patch(
                rest::a_folder(folder),
                serde_json::json!({ "devices": list }),
            )
        }
        Order::Set {
            scope, at, body, ..
        } => patch(scope.path(at.as_deref()), body.clone()),
        Order::Restart => post(rest::RESTART.to_owned()),
        // THE UNIT IS NOT THE API'S. These two never become an HTTP call, and
        // `carry_out` sends them to `unit.rs` before it gets here; the errand
        // exists so that the match is total and a new order cannot be added
        // without deciding what it does.
        Order::Start | Order::Stop => Errand {
            method: "GET",
            path: rest::PING.to_owned(),
            body: None,
        },
    }
}

/// Why an order did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// Nothing was listening: the service is not running.
    NotRunning,
    /// The service answered, and said no.
    Refused,
    /// The unit could not be started or stopped.
    NotAllowed,
    /// Something else, which the log carries.
    Unknown,
}

impl Refusal {
    /// The id, which is the stem of the text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotRunning => "not-running",
            Self::Refused => "refused",
            Self::NotAllowed => "not-allowed",
            Self::Unknown => "unknown",
        }
    }
}

/// What went wrong, read out of the error the call came back with.
///
/// The shape `ronoctua_power::refusal` set: the words are the daemon's, the
/// classification is ours, and it is done in ONE place so that two screens
/// cannot disagree about what a failure was.
#[must_use]
pub fn refusal(trouble: &anyhow::Error) -> Refusal {
    let said = trouble.to_string().to_ascii_lowercase();
    if said.contains("connection refused")
        || said.contains(rest::NOT_READABLE)
        || said.contains("no address")
        || said.contains("no route")
    {
        return Refusal::NotRunning;
    }
    if said.contains("not authorized") || said.contains("access denied") || said.contains("polkit")
    {
        return Refusal::NotAllowed;
    }
    if said.contains("the daemon answered") {
        return Refusal::Refused;
    }
    Refusal::Unknown
}

/// Something that happened.
#[derive(Debug, Clone)]
pub enum Event {
    /// The whole picture, again.
    ///
    /// Boxed because it is large and the other variant is not: an unboxed
    /// enum is as big as its biggest arm, in every channel it travels through.
    Snapshot(Box<Snapshot>),
    /// An order finished, and whether it was refused.
    Done {
        /// Which one.
        order: Box<Order>,
        /// `None` when it went through.
        refused: Option<Refusal>,
    },
}

/// The way to ask for something.
#[derive(Debug, Clone)]
pub struct Handle {
    orders: std::sync::mpsc::Sender<Order>,
}

impl Handle {
    /// Asks for one thing. Nothing waits.
    ///
    /// A dropped channel means the thread is gone, which happens when the
    /// process is closing; there is nobody to tell, so it goes to the log.
    pub fn send(&self, order: Order) {
        if let Err(e) = self.orders.send(order) {
            tracing::warn!(error = %e, "the syncthing thread is not there any more");
        }
    }
}

/// Raises the two threads and starts watching.
///
/// `wake` is called from THEM, not from the caller's thread — every door in
/// this system turns that into a channel its event loop already owns.
#[must_use]
pub fn watch(wake: impl Fn(Event) + Send + Clone + 'static) -> Handle {
    let (orders, taking) = std::sync::mpsc::channel::<Order>();

    let listening = wake.clone();
    std::thread::Builder::new()
        .name("syncthing-events".to_owned())
        .spawn(move || follow(&listening))
        .map_or_else(
            |e| tracing::error!(error = %e, "could not raise the syncthing watcher"),
            drop,
        );

    std::thread::Builder::new()
        .name("syncthing-orders".to_owned())
        .spawn(move || {
            // AND IT DOES NOT RE-READ AFTERWARDS. The long poll on the other
            // thread reports the change in 17 ms by itself; a read here would
            // mean holding an arrow key was one round trip per frame.
            while let Ok(order) = taking.recv() {
                let refused = carry_out(&order).err().map(|e| {
                    tracing::warn!(error = %e, ?order, "syncthing refused");
                    refusal(&e)
                });
                wake(Event::Done {
                    order: Box::new(order),
                    refused,
                });
            }
        })
        .map_or_else(
            |e| tracing::error!(error = %e, "could not raise the syncthing orders thread"),
            drop,
        );

    Handle { orders }
}

/// Does one thing.
///
/// # Errors
///
/// The service refused, or is not there.
pub fn carry_out(order: &Order) -> anyhow::Result<()> {
    match order {
        Order::Start => return unit::start(),
        Order::Stop => return unit::stop(),
        _ => {}
    }
    let seed = match seed_of(order) {
        Some(scope) => Some(serde_json::from_str(&rest::defaults(scope)?)?),
        None => None,
    };
    // THE SHARE ORDER'S SEED IS THE FOLDER ITSELF, not a defaults object: the
    // `devices` array is replaced whole by a PATCH, so the current list has to
    // be in hand before one entry can be added to or taken out of it.
    let seed = match order {
        Order::Share { folder, .. } => Some(serde_json::from_str(&rest::one_folder(folder)?)?),
        _ => seed,
    };
    let errand = errand_of(order, seed.as_ref());
    rest::call(errand.method, &errand.path, errand.body.as_deref())?;
    Ok(())
}

/// The watching thread: poll, read, tell, repeat.
fn follow(wake: &impl Fn(Event)) {
    let mut since = 0_u64;
    let mut rest_for = FIRST_REST;
    let mut last_read: Option<Instant> = None;
    let mut last_told: Option<Snapshot> = None;

    loop {
        match look() {
            Ok(now) => {
                rest_for = FIRST_REST;
                if last_told.as_ref() != Some(&now) {
                    last_told = Some(now.clone());
                    wake(Event::Snapshot(Box::new(now)));
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "syncthing did not answer");
                // AND IT ASKS SYSTEMD WHY. `Snapshot::default()` alone says
                // `Standing::Unknown`, which is "we could not find out" — so a
                // machine that never installed Syncthing and a machine whose
                // service is merely stopped reached the screen as one word,
                // and the modeline drew a mark on both. Asking here is one
                // `systemctl show` on a path that is already sleeping with a
                // backoff, so it costs nothing that matters.
                let down = Snapshot {
                    unit: unit::standing(),
                    ..Snapshot::default()
                };
                if last_told.as_ref() != Some(&down) {
                    last_told = Some(down.clone());
                    wake(Event::Snapshot(Box::new(down)));
                }
                std::thread::sleep(rest_for);
                rest_for = (rest_for * 2).min(MOST_REST);
                // AND THE COUNTER GOES BACK TO ZERO. Syncthing's event ids
                // start at 1 every time it starts, so a `since` from the last
                // run is a number the new one will never reach — and the poll
                // would then block for ever, on a service that is up.
                since = 0;
                continue;
            }
        }

        if let Some(when) = last_read {
            let waited = when.elapsed();
            if waited < FLOOR {
                std::thread::sleep(FLOOR - waited);
            }
        }
        last_read = Some(Instant::now());

        match rest::poll(since) {
            Ok(body) => {
                if let Some(last) = newest(&body) {
                    since = last;
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "the syncthing event stream stopped");
                std::thread::sleep(rest_for);
                rest_for = (rest_for * 2).min(MOST_REST);
                since = 0;
            }
        }
    }
}

/// The id of the last event in a batch.
///
/// `None` for an empty batch, which is what an idle minute answers — and
/// keeping the old `since` is exactly right there.
#[must_use]
pub fn newest(body: &str) -> Option<u64> {
    let events: Vec<serde_json::Value> = serde_json::from_str(body).ok()?;
    events.last()?.get("id").and_then(serde_json::Value::as_u64)
}

/// Reads everything, once.
///
/// # Errors
///
/// The service did not answer.
pub fn look() -> anyhow::Result<Snapshot> {
    let status: serde_json::Value = serde_json::from_str(&rest::status()?)?;
    let version: serde_json::Value = serde_json::from_str(&rest::version()?)?;
    let config: serde_json::Value = serde_json::from_str(&rest::config()?)?;
    let connections: serde_json::Value = serde_json::from_str(&rest::connections()?)?;
    let completion: serde_json::Value = serde_json::from_str(&rest::completion()?)?;
    // AN ERROR LIST THAT WILL NOT PARSE IS AN EMPTY ONE, and never a failure:
    // this is the least important call of the ten, and letting it take the
    // whole screen down would be the tail wagging the dog.
    let errors: serde_json::Value = rest::errors()
        .ok()
        .and_then(|b| serde_json::from_str(&b).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let waiting_devices: serde_json::Value = rest::pending_devices()
        .ok()
        .and_then(|b| serde_json::from_str(&b).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let waiting_folders: serde_json::Value = rest::pending_folders()
        .ok()
        .and_then(|b| serde_json::from_str(&b).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let mut folders = Vec::new();
    if let Some(list) = config.get("folders").and_then(serde_json::Value::as_array) {
        for raw in list {
            let id = text(raw, "id");
            let each: serde_json::Value = rest::folder_status(&id)
                .ok()
                .and_then(|b| serde_json::from_str(&b).ok())
                .unwrap_or_else(|| serde_json::json!({}));
            folders.push(folder_of(raw, &each));
        }
    }

    let me = text(&status, "myID");
    let devices = config
        .get("devices")
        .and_then(serde_json::Value::as_array)
        .map(|list| {
            list.iter()
                .map(|raw| device_of(raw, &connections, &me, &folders))
                .collect()
        })
        .unwrap_or_default();

    Ok(Snapshot {
        service: Service::Up(Box::new(Running {
            version: text(&version, "version"),
            id: me,
            uptime: number(&status, "uptime"),
            listeners: listeners_of(&status),
            errors: errors_of(&errors),
            address: rest::address().unwrap_or_default(),
        })),
        folders,
        devices,
        invitations: invitations_of(&waiting_devices, &waiting_folders, &config),
        options: config
            .get("options")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
        gui: without_secrets(config.get("gui")),
        folder_defaults: config
            .pointer("/defaults/folder")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
        device_defaults: config
            .pointer("/defaults/device")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
        completion: completion
            .get("completion")
            .and_then(serde_json::Value::as_f64)
            .map_or(0, |c| c.round().clamp(0.0, 100.0) as u8),
        traffic: Traffic {
            sent: connections
                .pointer("/total/outBytesTotal")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
            received: connections
                .pointer("/total/inBytesTotal")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0),
        },
        unit: unit::standing(),
    })
}

/// The interface's settings, minus the two that are secrets.
///
/// The API key and the password are read to SIGN a request and for nothing
/// else. Putting them in the snapshot would put them one careless row away
/// from the screen, and this crate's whole promise about them is that they
/// never cross.
fn without_secrets(gui: Option<&serde_json::Value>) -> serde_json::Value {
    let mut out = gui.cloned().unwrap_or_else(|| serde_json::json!({}));
    if let Some(map) = out.as_object_mut() {
        map.remove("apiKey");
        map.remove("apikey");
        map.remove("password");
    }
    out
}

/// A string field, or empty.
fn text(from: &serde_json::Value, key: &str) -> String {
    from.get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// A number field, or zero.
fn number(from: &serde_json::Value, key: &str) -> u64 {
    from.get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

/// A boolean field, or false.
fn flag(from: &serde_json::Value, key: &str) -> bool {
    from.get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// One folder, out of its configuration and its status.
#[must_use]
pub fn folder_of(raw: &serde_json::Value, status: &serde_json::Value) -> Folder {
    let global = number(status, "globalBytes");
    let need = number(status, "needBytes");
    let paused = flag(raw, "paused");
    Folder {
        id: text(raw, "id"),
        label: text(raw, "label"),
        path: text(raw, "path"),
        kind: text(raw, "type"),
        paused,
        state: if paused {
            State::Paused
        } else {
            State::of(&text(status, "state"))
        },
        // COMPLETION IS DERIVED, because `/rest/db/status` does not carry one
        // and asking `/rest/db/completion` per folder is another call per
        // folder per change. A folder with nothing in it anywhere is DONE, not
        // zero per cent: an empty folder drawn at 0% reads as broken.
        completion: if global == 0 {
            100
        } else {
            let done = global.saturating_sub(need);
            u8::try_from(done.saturating_mul(100) / global).unwrap_or(100)
        },
        global_bytes: global,
        local_bytes: number(status, "localBytes"),
        need_bytes: need,
        need_items: number(status, "needTotalItems"),
        errors: number(status, "errors") + number(status, "pullErrors"),
        devices: raw
            .get("devices")
            .and_then(serde_json::Value::as_array)
            .map(|list| list.iter().map(|d| text(d, "deviceID")).collect())
            .unwrap_or_default(),
        raw: raw.clone(),
    }
}

/// One device, out of its configuration and the live connection table.
#[must_use]
pub fn device_of(
    raw: &serde_json::Value,
    connections: &serde_json::Value,
    me: &str,
    folders: &[Folder],
) -> Device {
    let id = text(raw, "deviceID");
    let live = connections.pointer(&format!("/connections/{id}"));
    Device {
        // THE RUNTIME PAUSE WINS OVER THE CONFIGURED ONE.
        //
        // `/rest/system/pause` does not touch the configuration — measured:
        // after it, every `paused` in `config.xml` was still false and the
        // events were `DevicePaused`. Reading the config alone would draw a
        // paused machine as running, which is the one thing the mark exists to
        // say.
        paused: live.map_or_else(|| flag(raw, "paused"), |l| flag(l, "paused")),
        connected: live.is_some_and(|l| flag(l, "connected")),
        address: live.map(|l| text(l, "address")).unwrap_or_default(),
        client: live.map(|l| text(l, "clientVersion")).unwrap_or_default(),
        local: live.is_some_and(|l| flag(l, "isLocal")),
        sent: live.map_or(0, |l| number(l, "outBytesTotal")),
        received: live.map_or(0, |l| number(l, "inBytesTotal")),
        me: id == me,
        folders: folders
            .iter()
            .filter(|f| f.devices.iter().any(|d| d == &id))
            .map(|f| f.id.clone())
            .collect(),
        name: text(raw, "name"),
        id,
        raw: raw.clone(),
    }
}

/// Where the service is listening, out of its connection service status.
fn listeners_of(status: &serde_json::Value) -> Vec<String> {
    status
        .get("connectionServiceStatus")
        .and_then(serde_json::Value::as_object)
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default()
}

/// The service's complaints, in the order it made them.
fn errors_of(errors: &serde_json::Value) -> Vec<String> {
    errors
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .map(|list| list.iter().map(|e| text(e, "message")).collect())
        .unwrap_or_default()
}

/// Everyone waiting to be let in, machines before folders.
#[must_use]
pub fn invitations_of(
    devices: &serde_json::Value,
    folders: &serde_json::Value,
    config: &serde_json::Value,
) -> Vec<Invitation> {
    let mut out = Vec::new();
    if let Some(map) = devices.as_object() {
        for (id, about) in map {
            out.push(Invitation::Device {
                id: id.clone(),
                name: text(about, "name"),
                address: text(about, "address"),
            });
        }
    }
    let named = |id: &str| -> String {
        config
            .get("devices")
            .and_then(serde_json::Value::as_array)
            .and_then(|list| list.iter().find(|d| text(d, "deviceID") == id).cloned())
            .map(|d| text(&d, "name"))
            .unwrap_or_default()
    };
    if let Some(map) = folders.as_object() {
        for (folder, about) in map {
            // A FOLDER IS OFFERED BY ONE OR MORE MACHINES, and the offer is
            // keyed by machine inside it. Drawing one row per folder would
            // lose which of them to answer.
            let Some(from) = about
                .get("offeredBy")
                .and_then(serde_json::Value::as_object)
            else {
                continue;
            };
            for (who, what) in from {
                out.push(Invitation::Folder {
                    folder: folder.clone(),
                    label: text(what, "label"),
                    from: who.clone(),
                    who: named(who),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
