//! Native buffer runtime — sections 5.2, 7.8 and 7.9.
//!
//! # The 5 ms budget is not a goal, it is a configuration
//!
//! Instantiating a component with 1.5 MB of data (the old glyph browser's
//! `glyphdb`) copies those 1.5 MB per instance — unless the memory image is mapped
//! copy-on-write. `memory_init_cow` is the difference between meeting and not meeting
//! section 5.2, and there is no later tuning that makes up for having it off.
//!
//! That is why [`Runtime::fresh`] accepts no configuration: the decisions that
//! matter are **fixed**, and a constructor that let one choose would allow
//! a session to run outside the budget with nobody noticing.
//!
//! # Why AOT
//!
//! Compiling in `Module::new` puts Cranelift's cost on the path of opening a
//! buffer. The components are pre-compiled into `.cwasm` in the Nix build, and
//! here they are only deserialised.

// `deny`, not `forbid`: deserialising a `.cwasm` is an `unsafe fn` in wasmtime,
// and `forbid` accepts no exception even with a written contract. `deny` requires
// an explicit, visible `#[allow]` — the same criterion as `ronoctua-shell`.
#![deny(unsafe_code)]
#![warn(missing_docs)]

use anyhow::{Context as _, Result};
// Since wasmtime-wasi 38 the types are at the crate's ROOT, and `WasiView::ctx`
// returns `ctx` and `table` together in a `WasiCtxView` — the separate `IoView`
// from earlier versions no longer exists.
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

/// What every driven test of a buffer needs. See the module's own head.
pub mod driven;

/// Instantiation ceiling, from section 5.2.
///
/// It is not a "performance goal": it is the value the `instance-fast` check
/// compares against, and CI fails above it. With no gate, the regression comes back silently
/// on the first `cargo update` that touches wasmtime.
pub const CEILING_MS: f64 = 5.0;

/// What each instance carries.
///
/// WASI only. There is no Wayland handle, no token table, and no i18n
/// bundle — invariant 11 is not a rule the host remembers to obey, it is
/// what **does not exist** in this struct.
pub struct State {
    wasi: WasiCtx,
    table: wasmtime::component::ResourceTable,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// The engine, configured once.
pub struct Runtime {
    engine: wasmtime::Engine,
    linker: wasmtime::component::Linker<State>,
}

impl Runtime {
    /// Builds the engine with the configuration section 5.2 requires.
    ///
    /// # Errors
    ///
    /// wasmtime refuses the configuration — in practice, a platform with no support
    /// for the pooling allocator.
    pub fn fresh() -> Result<Self> {
        Self::with_cow(true)
    }

    /// The same engine, with `memory_init_cow` TURNED OFF.
    ///
    /// It exists so the benchmark can measure the difference instead of repeating it
    /// by ear. Do not use it in production: it is the configuration that blew section
    /// 5.2's budget with the old glyph browser's embedded table.
    ///
    /// # Errors
    ///
    /// The same as [`Runtime::fresh`].
    pub fn without_cow() -> Result<Self> {
        Self::with_cow(false)
    }

    fn with_cow(cow: bool) -> Result<Self> {
        let mut c = wasmtime::Config::new();

        // THE LINE THAT MATTERS. Without it, every instance memcpys the whole
        // memory image; with it, the mapping is copy-on-write and
        // instantiation becomes O(pages touched) instead of O(data).
        c.memory_init_cow(cow);

        // Pooling: it reuses memory slots instead of asking the OS on every
        // instance. `mmap`/`munmap` per buffer opened would on their own be
        // milliseconds.
        let mut pool = wasmtime::PoolingAllocationConfig::default();
        pool.total_component_instances(64);
        pool.total_memories(64);
        pool.total_tables(64);
        c.allocation_strategy(wasmtime::InstanceAllocationStrategy::Pooling(pool));

        // Components, not modules: it is invariant 12's shape.
        c.wasm_component_model(true);

        // Node debug info in the production binary: it grows the `.cwasm` and the
        // size counts against the deserialisation time.
        c.debug_info(false);

        let engine =
            wasmtime::Engine::new(&c).context("the configuration was refused by wasmtime")?;

        // The linker is built ONCE and reused. Building it per instance would put
        // the resolution of hundreds of WASI imports inside the 5 ms
        // budget — and it is work that does not change between instances.
        let mut linker = wasmtime::component::Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .context("could not wire WASI into the linker")?;

        Ok(Self { engine, linker })
    }

    /// The engine, for whoever needs to compile or deserialise.
    #[must_use]
    pub fn engine(&self) -> &wasmtime::Engine {
        &self.engine
    }

    /// Pre-compiles a component into `.cwasm`.
    ///
    /// It runs at BUILD time, never at runtime: Cranelift compiling at the moment of
    /// opening a buffer would put hundreds of milliseconds on the path.
    ///
    /// # Errors
    ///
    /// The file is not a valid component.
    pub fn precompile(&self, wasm: &[u8]) -> Result<Vec<u8>> {
        self.engine
            .precompile_component(wasm)
            .context("could not pre-compile the component")
    }

    /// Loads an already-compiled `.cwasm`.
    ///
    /// # Errors
    ///
    /// Bytes that are not a `.cwasm` of this wasmtime version. The version
    /// check is wasmtime's own and is desirable: a `.cwasm` from another
    /// version would be accepted and would fail much later.
    ///
    /// # Safety
    ///
    /// Deserialisation trusts the bytes. They come from the Nix store, written
    /// by our own build — the same trust one places in the binary.
    pub fn load(&self, cwasm: &[u8]) -> Result<wasmtime::component::Component> {
        // SAFETY: see the note above — the bytes come from the Nix store, written
        // by our own build.
        #[allow(unsafe_code)]
        unsafe { wasmtime::component::Component::deserialize(&self.engine, cwasm) }
            .context("the .cwasm does not match this version of wasmtime")
    }

    /// Instantiates an already-loaded component.
    ///
    /// THIS is the operation section 5.2 times: opening a buffer.
    ///
    /// # Errors
    ///
    /// The component asks for an import the linker does not offer.
    pub fn instantiate(
        &self,
        component: &wasmtime::component::Component,
    ) -> Result<wasmtime::Store<State>> {
        // A MINIMAL WASI context: no `inherit_stdio`, no `preopened_dir`,
        // no `inherit_env`. A buffer that needed a file would be
        // doing what the host should do for it — and the smaller surface
        // also instantiates faster.
        let state = State {
            wasi: WasiCtxBuilder::new().build(),
            table: wasmtime::component::ResourceTable::new(),
        };
        let mut store = wasmtime::Store::new(&self.engine, state);
        self.linker
            .instantiate(&mut store, component)
            .context("could not instantiate the component")?;
        Ok(store)
    }
}

/// A LIVE buffer: an instantiated component, with state between calls.
///
/// # Why the instance persists
///
/// The component holds state — the glyph browser has a search, a cursor and a
/// window top. Instantiating again on every call would clear all of it: typing
/// a letter and redrawing would show the whole list again, with no error
/// at all and no way to notice by looking at the buffer's code.
///
/// That is why `event` and `render` are methods **here**, and not of [`Runtime`]:
/// the type carries the one thing that cannot be recreated, which is the `Store`.
pub struct Instance {
    store: wasmtime::Store<State>,
    instance: wasmtime::component::Instance,
    init: wasmtime::component::ComponentExportIndex,
    render: wasmtime::component::ComponentExportIndex,
    event: wasmtime::component::ComponentExportIndex,
}

impl Instance {
    /// Calls `init`, returning the descriptor.
    ///
    /// Called ONCE, at load. This is where the buffer's keymap arrives as
    /// data — and it is why the host can refuse whoever binds outside
    /// `SPC m` **before** the buffer runs anything at all (section 7.9).
    ///
    /// # Errors
    ///
    /// `init` traps.
    pub fn init(&mut self) -> Result<ronoctua_protocol::Descriptor> {
        use wasmtime::component::Val;

        let func = self
            .instance
            .get_func(&mut self.store, self.init)
            .context("`init` is not a function")?;
        let mut out = [Val::Bool(false)];
        func.call(&mut self.store, &[], &mut out)
            .context("the component's `init` failed")?;
        func.post_return(&mut self.store)?;
        descriptor_of(&out[0])
    }

    /// Calls `render`, returning the display list.
    ///
    /// # Errors
    ///
    /// `render` traps.
    pub fn render(
        &mut self,
        columns: u16,
        lines: u16,
        heights: ronoctua_protocol::Heights,
    ) -> Result<ronoctua_protocol::DisplayList> {
        use wasmtime::component::Val;

        let func = self
            .instance
            .get_func(&mut self.store, self.render)
            .context("`render` is not a function")?;
        // Dynamic `Val` and not a typed `bindgen!`: the host has to load
        // ANY component that fulfils the world, including one written by a
        // third party and compiled elsewhere. Binding the types at compile time
        // would require recompiling the shell for every new buffer.
        let ctx = Val::Record(vec![
            ("columns".into(), Val::U16(columns)),
            ("lines".into(), Val::U16(lines)),
            (
                "heights".into(),
                Val::Record(vec![
                    ("small".into(), Val::U16(heights.small)),
                    ("base".into(), Val::U16(heights.base)),
                    ("normal".into(), Val::U16(heights.normal)),
                    ("large".into(), Val::U16(heights.large)),
                    ("huge".into(), Val::U16(heights.huge)),
                ]),
            ),
        ]);
        let mut out = [Val::Bool(false)];
        func.call(&mut self.store, &[ctx], &mut out)
            .context("the component's `render` failed")?;
        func.post_return(&mut self.store)?;
        display_list_from(&out[0])
    }

    /// Delivers an input to the buffer and returns what it ASKS for.
    ///
    /// The host executes the effects; the buffer touches neither clipboard,
    /// file nor surface (invariant 11).
    ///
    /// # Errors
    ///
    /// `event` traps.
    pub fn event(
        &mut self,
        e: &ronoctua_protocol::Input,
    ) -> Result<Vec<ronoctua_protocol::Effect>> {
        use wasmtime::component::Val;

        let func = self
            .instance
            .get_func(&mut self.store, self.event)
            .context("`event` is not a function")?;
        let mut out = [Val::Bool(false)];
        func.call(&mut self.store, &[val_of_input(e)], &mut out)
            .context("the component's `event` failed")?;
        func.post_return(&mut self.store)?;
        Ok(effects_from(&out[0]))
    }
}

impl Runtime {
    /// Instantiates a component and resolves the world's three exports.
    ///
    /// # Errors
    ///
    /// The component does not export `ronoctua:buffer/view@0.1.0` with the three
    /// functions — which is a contract refusal, and not a detail: a buffer with no
    /// `event` would accept keys and never react.
    pub fn open(&self, component: &wasmtime::component::Component) -> Result<Instance> {
        let mut store = self.instantiate_with(component)?;
        let instance = self
            .linker
            .instantiate(&mut store, component)
            .context("could not instantiate the component")?;

        let vision = instance
            .get_export_index(&mut store, None, "ronoctua:buffer/view@0.1.0")
            .context("the component does not export `ronoctua:buffer/view@0.1.0`")?;
        let export = |store: &mut wasmtime::Store<State>, name: &str| {
            instance
                .get_export_index(store, Some(&vision), name)
                .with_context(|| format!("the `view` interface does not export `{name}`"))
        };
        let init = export(&mut store, "init")?;
        let render = export(&mut store, "render")?;
        let event = export(&mut store, "event")?;

        Ok(Instance {
            store,
            instance,
            init,
            render,
            event,
        })
    }

    /// Opens and calls `init` in one go, for whoever only wants the descriptor.
    ///
    /// # Errors
    ///
    /// The same as [`Runtime::open`] and [`Instance::init`].
    pub fn init(
        &self,
        component: &wasmtime::component::Component,
    ) -> Result<ronoctua_protocol::Descriptor> {
        self.open(component)?.init()
    }

    /// Opens and calls `render` in one go.
    ///
    /// It serves whoever draws a single frame. Whoever processes input has to
    /// keep the [`Instance`] — here the state dies with it.
    ///
    /// # Errors
    ///
    /// The same as [`Runtime::open`] and [`Instance::render`].
    pub fn render(
        &self,
        component: &wasmtime::component::Component,
        columns: u16,
        lines: u16,
        heights: ronoctua_protocol::Heights,
    ) -> Result<ronoctua_protocol::DisplayList> {
        self.open(component)?.render(columns, lines, heights)
    }

    fn instantiate_with(
        &self,
        _component: &wasmtime::component::Component,
    ) -> Result<wasmtime::Store<State>> {
        let state = State {
            wasi: WasiCtxBuilder::new().build(),
            table: wasmtime::component::ResourceTable::new(),
        };
        Ok(wasmtime::Store::new(&self.engine, state))
    }
}

/// The host's input in WIT's form.
fn val_of_input(e: &ronoctua_protocol::Input) -> wasmtime::component::Val {
    use ronoctua_protocol::Input;
    use wasmtime::component::Val;

    let (case, payload) = match e {
        Input::Command(id) => ("command", Some(Val::String(id.clone()))),
        Input::EnterState(id) => ("enter-state", Some(Val::String(id.clone()))),
        // `character`, and not `char`: see the comment in the `.wit`.
        Input::Char(c) => ("character", Some(Val::Char(*c))),
        Input::Up => ("up", None),
        Input::Down => ("down", None),
        Input::Left => ("left", None),
        Input::Right => ("right", None),
        Input::Confirm => ("confirm", None),
        Input::Cancel => ("cancel", None),
        Input::Delete => ("delete", None),
        Input::Toggle => ("toggle", None),
        Input::Copied(ok) => ("copied", Some(Val::Bool(*ok))),
        Input::Paste(t) => ("paste", Some(Val::String(t.clone()))),
        Input::Phrases(p) => ("phrases", Some(val_of_phrases(p))),
        Input::Click { column, line } => (
            "click",
            Some(Val::Record(vec![
                ("column".into(), Val::U16(*column)),
                ("line".into(), Val::U16(*line)),
            ])),
        ),
        // THE POINTER AT REST, in the same `position` record the click uses —
        // it is the same question asked without a button, and giving it a
        // shape of its own would be two answers to "where is the mouse".
        // AND WHICH POINTER: `pointed` is the position NESTED with the
        // cursor's number beside it, the shape the WIT declares.
        Input::Hover { column, line, seat } => (
            "hover",
            Some(Val::Record(vec![
                (
                    "at".into(),
                    Val::Record(vec![
                        ("column".into(), Val::U16(*column)),
                        ("line".into(), Val::U16(*line)),
                    ]),
                ),
                ("seat".into(), Val::U16(*seat)),
            ])),
        ),
        Input::Unhover { seat } => ("unhover", Some(Val::U16(*seat))),
        Input::KeyboardSeat { seat } => ("keyboard-seat", Some(Val::U16(*seat))),
        Input::KeyboardLeft { seat } => ("keyboard-left", Some(Val::U16(*seat))),
        // THE WHEEL, and it carries a `position` RECORD the way `click` does —
        // a nested record and not two more fields, because that is the shape
        // the WIT declares and `wasmtime` lowers a record POSITIONALLY. Two
        // `u16` and an `s16` flattened here would lower cleanly into a
        // `{ position, s16 }` and put the line number where the position is.
        Input::Wheel {
            column,
            line,
            lines,
        } => (
            "wheel",
            Some(Val::Record(vec![
                (
                    "at".into(),
                    Val::Record(vec![
                        ("column".into(), Val::U16(*column)),
                        ("line".into(), Val::U16(*line)),
                    ]),
                ),
                ("lines".into(), Val::S16(*lines)),
            ])),
        ),
        Input::Scrolled {
            column,
            line,
            first,
        } => (
            "scrolled",
            Some(Val::Record(vec![
                (
                    "at".into(),
                    Val::Record(vec![
                        ("column".into(), Val::U16(*column)),
                        ("line".into(), Val::U16(*line)),
                    ]),
                ),
                ("first".into(), Val::U32(*first)),
            ])),
        ),
        Input::Survey(s) => ("survey", Some(val_of_survey(s))),
        Input::Queue(q) => ("queue", Some(val_of_queue(q))),
        Input::Rack(r) => ("rack", Some(val_of_rack(r))),
        Input::Keyboards(b) => ("keyboards", Some(val_of_boards(b))),
        Input::Mind(m) => ("mind", Some(val_of_mind(m))),
        Input::Log(lines) => ("log", Some(val_of_log(lines))),
        Input::ModuleScreen(screen) => ("module-screen", Some(val_of_screen(screen))),
        Input::Formatted(o) => (
            "formatted",
            Some(wasmtime::component::Val::Record(vec![
                ("done".into(), wasmtime::component::Val::Bool(o.done)),
                (
                    "trouble".into(),
                    wasmtime::component::Val::String(o.trouble.clone()),
                ),
            ])),
        ),
    };
    Val::Variant(case.to_owned(), payload.map(Box::new))
}

/// The name a `gauge` has in the WIT.
///
/// A `match` and not a table on purpose: it is EXHAUSTIVE, so a gauge added to
/// the protocol without a name here does not compile. A table would let it
/// through and the reading would simply never arrive.
const fn gauge_name(g: ronoctua_protocol::Gauge) -> &'static str {
    use ronoctua_protocol::Gauge as G;
    match g {
        G::RamTotal => "ram-total",
        G::RamAvailable => "ram-available",
        G::RamFree => "ram-free",
        G::RamCached => "ram-cached",
        G::RamBuffers => "ram-buffers",
        G::RamShmem => "ram-shmem",
        G::RamSlabReclaimable => "ram-slab-reclaimable",
        G::RamDirty => "ram-dirty",
        G::RamAnonymous => "ram-anonymous",
        G::RamKernel => "ram-kernel",
        G::SwapTotal => "swap-total",
        G::SwapFree => "swap-free",
        G::ZramOriginal => "zram-original",
        G::ZramCompressed => "zram-compressed",
        G::ZramLimit => "zram-limit",
        G::DesktopMin => "desktop-min",
        G::DesktopCurrent => "desktop-current",
        G::AppCurrent => "app-current",
        G::AppCeiling => "app-ceiling",
        G::SystemCurrent => "system-current",
        G::VramTotal => "vram-total",
        G::VramUsed => "vram-used",
        G::DiskTotal => "disk-total",
        G::DiskFree => "disk-free",
    }
}

/// The same, for what kind of thing a consumer is.
const fn consumer_kind_name(k: ronoctua_protocol::ConsumerKind) -> &'static str {
    use ronoctua_protocol::ConsumerKind as K;
    match k {
        K::Process => "process",
        K::Scope => "scope",
        K::Directory => "directory",
        K::Filesystem => "filesystem",
        K::Generation => "generation",
    }
}

/// And for which policy is in force.
const fn guard_mode_name(m: ronoctua_protocol::GuardMode) -> &'static str {
    use ronoctua_protocol::GuardMode as M;
    match m {
        M::Unknown => "unknown",
        M::Off => "off",
        M::Strict => "strict",
        M::Tolerant => "tolerant",
    }
}

/// The name an `engine` has in the WIT.
const fn engine_name(e: ronoctua_protocol::Engine) -> &'static str {
    use ronoctua_protocol::Engine as E;
    match e {
        E::Local => "local",
        E::Fetch => "fetch",
        E::Media => "media",
        E::Torrent => "torrent",
        E::Atlas => "atlas",
        E::Software => "software",
        E::Scan => "scan",
        E::Container => "container",
        E::Belongings => "belongings",
        E::Seal => "seal",
        E::Incus => "incus",
        E::Stick => "stick",
    }
}

/// The name a `stage` has in the WIT.
const fn stage_name(s: ronoctua_protocol::Stage) -> &'static str {
    use ronoctua_protocol::Stage as S;
    match s {
        S::Weighing => "weighing",
        S::Running => "running",
        S::Paused => "paused",
        S::Done => "done",
        S::Failed => "failed",
        S::Cancelled => "cancelled",
    }
}

/// The name an `errand` has in the WIT.
const fn errand_name(e: ronoctua_protocol::Errand) -> &'static str {
    use ronoctua_protocol::Errand as E;
    match e {
        E::Copy => "copy",
        E::MoveIt => "move-it",
        E::Delete => "delete",
        E::Shred => "shred",
        E::Trash => "trash",
        E::Restore => "restore",
        E::Compress => "compress",
        E::Extract => "extract",
        E::Measure => "measure",
        E::Fetch => "fetch",
        E::Media => "media",
        E::Audio => "audio",
        E::Subtitles => "subtitles",
        E::Torrent => "torrent",
        E::ArchiveAdd => "archive-add",
        E::ArchiveRemove => "archive-remove",
        E::ArchiveTest => "archive-test",
        E::ArchiveConvert => "archive-convert",
        E::Seal => "seal",
        E::Unseal => "unseal",
        E::Incus => "incus",
        E::StickWrite => "stick-write",
    }
}

/// And for what the counters count.
const fn amount_name(a: ronoctua_protocol::Amount) -> &'static str {
    use ronoctua_protocol::Amount as A;
    match a {
        A::Bytes => "bytes",
        A::Items => "items",
    }
}

/// The name a `vital` has in the WIT.
///
/// A `match` and not a table, like `gauge_name`: it is EXHAUSTIVE, so a vital
/// added to the protocol without a name here does not compile. A table would
/// let it through and the reading would simply never arrive.
const fn vital_name(v: ronoctua_protocol::Vital) -> &'static str {
    use ronoctua_protocol::Vital as V;
    match v {
        V::RamUsed => "ram-used",
        V::MemoryStall => "memory-stall",
        V::CpuStall => "cpu-stall",
        V::OnBattery => "on-battery",
    }
}

/// The name a `standing` has in the WIT.
const fn standing_name(s: ronoctua_protocol::Standing) -> &'static str {
    use ronoctua_protocol::Standing as S;
    match s {
        S::Idle => "idle",
        S::Queued => "queued",
        S::Running => "running",
    }
}

/// The name a `hold-kind` has in the WIT.
const fn hold_kind_name(h: ronoctua_protocol::HoldKind) -> &'static str {
    use ronoctua_protocol::HoldKind as H;
    match h {
        H::None => "none",
        H::Ram => "ram",
        H::Cpu => "cpu",
        H::Battery => "battery",
        H::Busy => "busy",
        H::Network => "network",
        H::Asleep => "asleep",
    }
}

/// The name an `outcome-kind` has in the WIT.
const fn outcome_kind_name(o: ronoctua_protocol::OutcomeKind) -> &'static str {
    use ronoctua_protocol::OutcomeKind as O;
    match o {
        O::Skipped => "skipped",
        O::Done => "done",
        O::Failed => "failed",
        O::Held => "held",
        O::Noted => "noted",
    }
}

/// One run of the journal, as the WIT's `run-record`.
///
/// A FUNCTION and not a closure, because two places marshal one: a task's own
/// history and the whole machine's activity log. They are the same record, and
/// two copies of nine fields is two chances for `wasmtime` to lower a pair the
/// wrong way round — it zips a record POSITIONALLY and only then compares
/// names, so two strings traded still lower cleanly.
fn run_val(r: &ronoctua_protocol::RunRecord) -> wasmtime::component::Val {
    use wasmtime::component::Val;
    Val::Record(vec![
        ("task".into(), Val::String(r.task.clone())),
        ("task-text".into(), Val::String(r.task_text.clone())),
        ("began".into(), Val::U64(r.began)),
        ("ended".into(), Val::U64(r.ended)),
        ("when-text".into(), Val::String(r.when_text.clone())),
        (
            "outcome".into(),
            Val::Enum(outcome_kind_name(r.outcome).to_owned()),
        ),
        ("at".into(), Val::String(r.at.clone())),
        ("why".into(), Val::String(r.why.clone())),
        ("trouble".into(), Val::String(r.trouble.clone())),
        (
            "steps".into(),
            Val::List(
                r.steps
                    .iter()
                    .map(|o| {
                        Val::Record(vec![
                            ("id".into(), Val::String(o.id.clone())),
                            ("ok".into(), Val::Bool(o.ok)),
                            ("seconds".into(), Val::U32(o.seconds)),
                            ("trouble".into(), Val::String(o.trouble.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        // THE KEYS THE HOST RESOLVED, so the component never builds one out
        // of an id, and what a note says — the key and the sentence made of it.
        ("task-key".into(), Val::String(r.task_key.clone())),
        ("at-key".into(), Val::String(r.at_key.clone())),
        ("why-key".into(), Val::String(r.why_key.clone())),
        ("says".into(), Val::String(r.says.clone())),
        ("says-text".into(), Val::String(r.says_text.clone())),
    ])
}

/// One action of a module, as the WIT's `module-action`.
///
/// A FUNCTION for the reason `run_val` is one: a task's page and a notice's
/// controls both carry the record, and two copies of four fields are two
/// chances to trade the two strings.
fn action_val(a: &ronoctua_protocol::ModuleAction) -> wasmtime::component::Val {
    use wasmtime::component::Val;
    Val::Record(vec![
        ("id".into(), Val::String(a.id.clone())),
        ("label-key".into(), Val::String(a.label_key.clone())),
        ("row".into(), Val::Bool(a.row)),
        ("asks-key".into(), Val::String(a.asks_key.clone())),
    ])
}

/// The name a `notice-level` has in the WIT.
const fn notice_level_name(l: ronoctua_protocol::NoticeLevel) -> &'static str {
    use ronoctua_protocol::NoticeLevel as L;
    match l {
        L::Info => "info",
        L::Warn => "warn",
        L::Danger => "danger",
        L::Alarm => "alarm",
    }
}

/// One notice, as the WIT's `notice`.
fn notice_val(n: &ronoctua_protocol::Notice) -> wasmtime::component::Val {
    use wasmtime::component::Val;
    Val::Record(vec![
        ("module".into(), Val::String(n.module.clone())),
        ("id".into(), Val::String(n.id.clone())),
        ("says".into(), Val::String(n.says.clone())),
        ("says-text".into(), Val::String(n.says_text.clone())),
        (
            "args".into(),
            Val::List(
                n.args
                    .iter()
                    .map(|a| {
                        Val::Record(vec![
                            ("name".into(), Val::String(a.name.clone())),
                            ("text".into(), Val::String(a.text.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "level".into(),
            Val::Enum(notice_level_name(n.level).to_owned()),
        ),
        ("since".into(), Val::U64(n.since)),
        ("since-text".into(), Val::String(n.since_text.clone())),
        (
            "controls".into(),
            Val::List(n.controls.iter().map(action_val).collect()),
        ),
        ("dismissable".into(), Val::Bool(n.dismissable)),
        (
            "snooze".into(),
            Val::List(n.snooze.iter().copied().map(Val::U16).collect()),
        ),
    ])
}

/// One row of the folded activity log, as the WIT's `activity-row`.
fn activity_row_val(a: &ronoctua_protocol::ActivityRow) -> wasmtime::component::Val {
    use wasmtime::component::Val;
    Val::Record(vec![
        ("newest".into(), run_val(&a.newest)),
        ("repeats".into(), Val::U32(a.repeats)),
        ("last-text".into(), Val::String(a.last_text.clone())),
        ("first-text".into(), Val::String(a.first_text.clone())),
        (
            "members".into(),
            Val::List(a.members.iter().map(run_val).collect()),
        ),
    ])
}

/// The name a `brain-tone` has in the WIT.
const fn brain_tone_name(t: ronoctua_protocol::BrainTone) -> &'static str {
    use ronoctua_protocol::BrainTone as T;
    match t {
        T::Quiet => "quiet",
        T::Watching => "watching",
        T::Warning => "warning",
        T::Danger => "danger",
        T::Accent => "accent",
    }
}

/// One cell of a module's row, as the WIT's `screen-cell` variant.
fn cell_val(c: &ronoctua_protocol::ScreenCell) -> wasmtime::component::Val {
    use ronoctua_protocol::ScreenCell as C;
    use wasmtime::component::Val;
    let (case, payload) = match c {
        C::Empty => ("empty", None),
        C::Key(k) => ("key", Some(Val::String(k.clone()))),
        C::Data(d) => ("data", Some(Val::String(d.clone()))),
        C::Moment(t) => (
            "moment",
            Some(Val::Record(vec![
                ("at".into(), Val::U64(t.at)),
                ("text".into(), Val::String(t.text.clone())),
            ])),
        ),
        C::Count(n) => ("count", Some(Val::U64(*n))),
        C::Bytes(m) => (
            "bytes",
            Some(Val::Record(vec![
                ("bytes".into(), Val::U64(m.bytes)),
                ("text".into(), Val::String(m.text.clone())),
            ])),
        ),
    };
    Val::Variant(case.to_owned(), payload.map(Box::new))
}

/// A module's page, as the WIT's `module-screen`.
///
/// The field order is the WIT's, for the reason `val_of_mind` writes down.
fn val_of_screen(s: &ronoctua_protocol::ModuleScreen) -> wasmtime::component::Val {
    use wasmtime::component::Val;
    let sections = s
        .sections
        .iter()
        .map(|section| {
            let rows = section
                .rows
                .iter()
                .map(|r| {
                    Val::Record(vec![
                        ("id".into(), Val::String(r.id.clone())),
                        ("label".into(), cell_val(&r.label)),
                        ("value".into(), cell_val(&r.value)),
                        ("note".into(), cell_val(&r.note)),
                        ("tone".into(), Val::Enum(brain_tone_name(r.tone).to_owned())),
                        ("act".into(), Val::String(r.act.clone())),
                        ("also".into(), Val::String(r.also.clone())),
                    ])
                })
                .collect();
            Val::Record(vec![
                ("heading".into(), Val::String(section.heading.clone())),
                ("rows".into(), Val::List(rows)),
            ])
        })
        .collect();
    Val::Record(vec![
        ("module".into(), Val::String(s.module.clone())),
        ("generation".into(), Val::U64(s.generation)),
        ("sections".into(), Val::List(sections)),
        ("refused".into(), Val::String(s.refused.clone())),
    ])
}

/// The scale a command named, lifted back.
///
/// A FUNCTION because two commands carry one — a run and a paragraph — and the
/// second was written by copying the first, which is how a new scale ends up
/// legible in one of them and gigantic in the other.
///
/// An unknown enum falls back to NORMAL: a buffer compiled against a newer WIT
/// shows up readable rather than three times the size of everything round it.
fn scale_of(v: Option<&wasmtime::component::Val>) -> ronoctua_protocol::Scale {
    use ronoctua_protocol::Scale;
    use wasmtime::component::Val;
    match v {
        Some(Val::Enum(n)) if n == "large" => Scale::Large,
        Some(Val::Enum(n)) if n == "small" => Scale::Small,
        Some(Val::Enum(n)) if n == "base" => Scale::Base,
        Some(Val::Enum(n)) if n == "huge" => Scale::Huge,
        _ => Scale::Normal,
    }
}

/// The name a `task-kind` has in the WIT.
const fn task_kind_name(k: ronoctua_protocol::TaskKind) -> &'static str {
    use ronoctua_protocol::TaskKind as K;
    match k {
        K::Routine => "routine",
        K::Once => "once",
        K::Watch => "watch",
    }
}

/// The name a `day-part` has in the WIT.
const fn day_part_name(d: ronoctua_protocol::DayPart) -> &'static str {
    use ronoctua_protocol::DayPart as D;
    match d {
        D::Any => "any",
        D::Night => "night",
        D::Morning => "morning",
        D::Afternoon => "afternoon",
        D::Evening => "evening",
    }
}

/// The queue, as a `Val` the guest can receive.
///
/// The field order is the WIT's, and the warning above [`val_of_survey`]
/// applies here word for word: `wasmtime` zips positionally and only then
/// compares names. The test at the end of this file walks this shape too.
fn val_of_queue(q: &ronoctua_protocol::Queue) -> wasmtime::component::Val {
    use wasmtime::component::Val;

    let jobs = q
        .jobs
        .iter()
        .map(|j| {
            Val::Record(vec![
                ("id".into(), Val::U64(j.id)),
                ("engine".into(), Val::Enum(engine_name(j.engine).to_owned())),
                ("errand".into(), Val::Enum(errand_name(j.errand).to_owned())),
                ("stage".into(), Val::Enum(stage_name(j.stage).to_owned())),
                ("amount".into(), Val::Enum(amount_name(j.amount).to_owned())),
                ("done".into(), Val::U64(j.done)),
                ("total".into(), Val::U64(j.total)),
                ("subject".into(), Val::String(j.subject.clone())),
                ("trouble".into(), Val::String(j.trouble.clone())),
                ("answer".into(), Val::U64(j.answer)),
                ("identified".into(), Val::Bool(j.identified)),
                ("pauses".into(), Val::Bool(j.pauses)),
                ("opens".into(), Val::Bool(j.opens)),
            ])
        })
        .collect();

    Val::Record(vec![("jobs".into(), Val::List(jobs))])
}

/// The rack, as a `Val` the guest can receive.
///
/// # The order of the fields is the WIT's order, and it is not style
///
/// `wasmtime` lowers a `Val::Record` by zipping it POSITIONALLY against the
/// WIT's fields and only then compares the names. A field moved here without
/// being moved there fails when the function is CALLED, with a message about a
/// name — which is why there is a test walking both shapes field by field.
fn val_of_rack(r: &ronoctua_protocol::Rack) -> wasmtime::component::Val {
    use wasmtime::component::Val;

    let devices = r
        .devices
        .iter()
        .map(|d| {
            Val::Record(vec![
                ("id".into(), Val::String(d.id.clone())),
                ("name".into(), Val::String(d.name.clone())),
                ("node".into(), Val::String(d.node.clone())),
                ("kind".into(), Val::String(d.kind.clone())),
                ("bytes".into(), Val::U64(d.bytes)),
                ("mounted".into(), Val::Bool(d.mounted)),
                ("removable".into(), Val::Bool(d.removable)),
                ("needed".into(), Val::String(d.needed.clone())),
                ("whole".into(), Val::Bool(d.whole)),
            ])
        })
        .collect();

    Val::Record(vec![("devices".into(), Val::List(devices))])
}

/// The keyboards, as a `Val` the guest can receive.
///
/// THE FIELD ORDER IS THE WIT'S, for the reason `val_of_rack` gives above:
/// `wasmtime` zips a record positionally and only then compares names, so a
/// field moved here without being moved there fails when the function is
/// called, with a message about a name.
fn val_of_boards(b: &ronoctua_protocol::Boards) -> wasmtime::component::Val {
    use wasmtime::component::Val;

    let word = |w: &ronoctua_protocol::Word| {
        Val::Record(vec![
            ("keyd".into(), Val::String(w.keyd.clone())),
            ("text".into(), Val::String(w.text.clone())),
            ("holds".into(), Val::Bool(w.holds)),
        ])
    };
    let boards = b
        .boards
        .iter()
        .map(|one| {
            Val::Record(vec![
                ("id".into(), Val::String(one.id.clone())),
                ("name".into(), Val::String(one.name.clone())),
                ("built-in".into(), Val::Bool(one.built_in)),
                (
                    "decided".into(),
                    Val::List(
                        one.decided
                            .iter()
                            .map(|(k, v)| {
                                Val::Tuple(vec![Val::String(k.clone()), Val::String(v.clone())])
                            })
                            .collect(),
                    ),
                ),
            ])
        })
        .collect();

    Val::Record(vec![
        ("keys".into(), Val::List(b.keys.iter().map(word).collect())),
        (
            "choices".into(),
            Val::List(b.choices.iter().map(word).collect()),
        ),
        ("boards".into(), Val::List(boards)),
        ("trouble".into(), Val::String(b.trouble.clone())),
        ("leader".into(), Val::String(b.leader.clone())),
        (
            "mode".into(),
            Val::Enum(keyboard_mode_name(b.mode).to_owned()),
        ),
        (
            "units".into(),
            Val::List(b.units.iter().map(val_of_unit).collect()),
        ),
        (
            "cursors".into(),
            Val::List(b.cursors.iter().map(|c| Val::U16(*c)).collect()),
        ),
        (
            "layouts".into(),
            Val::List(
                b.layouts
                    .iter()
                    .map(|l| {
                        Val::Record(vec![
                            ("code".into(), Val::String(l.code.clone())),
                            ("name".into(), Val::String(l.name.clone())),
                            (
                                "variants".into(),
                                Val::List(
                                    l.variants.iter().map(|v| Val::String(v.clone())).collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

/// The WIT's name for a keyboard mode.
const fn keyboard_mode_name(m: ronoctua_protocol::KeyboardMode) -> &'static str {
    match m {
        ronoctua_protocol::KeyboardMode::Together => "together",
        ronoctua_protocol::KeyboardMode::Independent => "independent",
    }
}

/// One unit, IN THE WIT'S FIELD ORDER — see `val_of_boards`.
fn val_of_unit(u: &ronoctua_protocol::UnitBoard) -> wasmtime::component::Val {
    use wasmtime::component::Val;

    Val::Record(vec![
        ("identity".into(), Val::String(u.identity.clone())),
        ("name".into(), Val::String(u.name.clone())),
        ("model".into(), Val::String(u.model.clone())),
        ("built-in".into(), Val::Bool(u.built_in)),
        ("plugged".into(), Val::Bool(u.plugged)),
        ("cursor".into(), Val::U16(u.cursor)),
        ("layout".into(), Val::String(u.layout.clone())),
        ("layout-variant".into(), Val::String(u.variant.clone())),
        ("leader".into(), Val::String(u.leader.clone())),
        ("repeat-rate".into(), Val::U16(u.repeat_rate)),
        ("repeat-delay".into(), Val::U16(u.repeat_delay)),
        (
            "decided".into(),
            Val::List(
                u.decided
                    .iter()
                    .map(|(k, v)| Val::Tuple(vec![Val::String(k.clone()), Val::String(v.clone())]))
                    .collect(),
            ),
        ),
    ])
}

/// The unit change the guest asked for, read back out of its record.
fn unit_change_from(v: Option<&wasmtime::component::Val>) -> Option<ronoctua_protocol::UnitChange> {
    use ronoctua_protocol::UnitSetting;
    use wasmtime::component::Val;

    let Some(Val::Record(fields)) = v else {
        return None;
    };
    let get = |name: &str| fields.iter().find(|(n, _)| n == name).map(|(_, v)| v);
    let Some(Val::String(unit)) = get("unit") else {
        return None;
    };
    let Some(Val::Variant(case, payload)) = get("setting") else {
        return None;
    };
    let text = || match payload.as_deref() {
        Some(Val::String(s)) => Some(s.clone()),
        _ => None,
    };
    let number = || match payload.as_deref() {
        Some(Val::U16(n)) => Some(*n),
        _ => None,
    };
    let setting = match case.as_str() {
        "cursor" => UnitSetting::Cursor(number()?),
        "layout" => UnitSetting::Layout(text()?),
        "layout-variant" => UnitSetting::Variant(text()?),
        "leader" => UnitSetting::Leader(text()?),
        "repeat-rate" => UnitSetting::RepeatRate(number()?),
        "repeat-delay" => UnitSetting::RepeatDelay(number()?),
        "remap" => match payload.as_deref() {
            Some(Val::Tuple(pair)) => match pair.as_slice() {
                [Val::String(k), Val::String(v)] => UnitSetting::Remap(k.clone(), v.clone()),
                _ => return None,
            },
            _ => return None,
        },
        _ => return None,
    };
    Some(ronoctua_protocol::UnitChange {
        unit: unit.clone(),
        setting,
    })
}

/// The survey, as a `Val` the guest can receive.
///
/// # The order of the fields is the WIT's order, and it is not style
///
/// The keys a buffer declared, with the text the host resolved for each.
///
/// THE FIELD ORDER IS THE WIT'S, for the reason spelled out over
/// `val_of_survey` below, and the shape test at the end of this file walks it.
fn val_of_phrases(p: &[ronoctua_protocol::Phrase]) -> wasmtime::component::Val {
    use wasmtime::component::Val;

    Val::List(
        p.iter()
            .map(|one| {
                Val::Record(vec![
                    ("key".into(), Val::String(one.key.clone())),
                    ("text".into(), Val::String(one.text.clone())),
                ])
            })
            .collect(),
    )
}

/// `wasmtime` lowers a `Val::Record` by zipping it POSITIONALLY against the
/// WIT's fields and only then compares the names (`values.rs`, `lower`). A field
/// moved here without being moved there fails when the function is CALLED, with
/// a message about a name — never at compile time. The test at the end of this
/// file walks the shape so that a reordering fails in `cargo test` instead of on
/// somebody's screen.
fn val_of_survey(s: &ronoctua_protocol::Survey) -> wasmtime::component::Val {
    use wasmtime::component::Val;

    let readings = s
        .readings
        .iter()
        .map(|r| {
            Val::Record(vec![
                ("of".into(), Val::Enum(gauge_name(r.of).to_owned())),
                ("bytes".into(), Val::U64(r.bytes)),
            ])
        })
        .collect();

    let consumers = s
        .consumers
        .iter()
        .map(|c| {
            Val::Record(vec![
                (
                    "kind".into(),
                    Val::Enum(consumer_kind_name(c.kind).to_owned()),
                ),
                ("name".into(), Val::String(c.name.clone())),
                ("bytes".into(), Val::U64(c.bytes)),
                ("hint".into(), Val::U64(c.hint)),
            ])
        })
        .collect();

    Val::Record(vec![
        ("readings".into(), Val::List(readings)),
        (
            "pressure".into(),
            Val::Record(vec![
                ("some10".into(), Val::U16(s.pressure.some10)),
                ("some60".into(), Val::U16(s.pressure.some60)),
                ("some300".into(), Val::U16(s.pressure.some300)),
                ("full10".into(), Val::U16(s.pressure.full10)),
                ("full60".into(), Val::U16(s.pressure.full60)),
            ]),
        ),
        ("consumers".into(), Val::List(consumers)),
        (
            "guard".into(),
            Val::Record(vec![
                (
                    "mode".into(),
                    Val::Enum(guard_mode_name(s.guard.mode).to_owned()),
                ),
                ("pressure-limit".into(), Val::U16(s.guard.pressure_limit)),
                (
                    "pressure-seconds".into(),
                    Val::U32(s.guard.pressure_seconds),
                ),
                ("ceiling".into(), Val::U16(s.guard.ceiling)),
                ("acted".into(), Val::U32(s.guard.acted)),
                ("zram".into(), Val::Bool(s.guard.zram)),
                (
                    "persisted".into(),
                    Val::Enum(guard_mode_name(s.guard.persisted).to_owned()),
                ),
            ]),
        ),
        ("scanning".into(), Val::Bool(s.scanning)),
        ("progress".into(), Val::U16(s.progress)),
    ])
}

/// The brain's log, as a `Val`: a list of `log-line`, in the WIT's field order.
fn val_of_log(lines: &[ronoctua_protocol::LogLine]) -> wasmtime::component::Val {
    use ronoctua_protocol::LogLevel as L;
    use wasmtime::component::Val;
    Val::List(
        lines
            .iter()
            .map(|l| {
                Val::Record(vec![
                    ("seq".into(), Val::U64(l.seq)),
                    ("at-text".into(), Val::String(l.at_text.clone())),
                    (
                        "level".into(),
                        Val::Enum(
                            match l.level {
                                L::Error => "error",
                                L::Warn => "warn",
                                L::Info => "info",
                                L::Debug => "debug",
                                L::Trace => "trace",
                            }
                            .to_owned(),
                        ),
                    ),
                    ("text".into(), Val::String(l.text.clone())),
                ])
            })
            .collect(),
    )
}

/// The mind, as a `Val` the guest can receive.
///
/// # The order of the fields is the WIT's order, and it is not style
///
/// `wasmtime` lowers a `Val::Record` by zipping it POSITIONALLY against the
/// WIT's fields and only then compares the names (`values.rs`, `lower`). A field
/// moved here without being moved there fails when the function is CALLED, with
/// a message about a name — never at compile time. There is a test at the end of
/// this file walking the shape, and it matters more here than anywhere else in
/// this file: `task-state` has three booleans and two strings in a row, and any
/// pair of them traded still lowers CLEANLY.
fn val_of_mind(m: &ronoctua_protocol::Mind) -> wasmtime::component::Val {
    use wasmtime::component::Val;

    let signs = m
        .pulse
        .signs
        .iter()
        .map(|s| {
            Val::Record(vec![
                ("of".into(), Val::Enum(vital_name(s.of).to_owned())),
                ("amount".into(), Val::U16(s.amount)),
            ])
        })
        .collect();

    let tasks = m
        .tasks
        .iter()
        .map(|t| {
            let steps = t
                .steps
                .iter()
                .map(|s| {
                    Val::Record(vec![
                        ("id".into(), Val::String(s.id.clone())),
                        ("command".into(), Val::String(s.command.clone())),
                        ("needs-root".into(), Val::Bool(s.needs_root)),
                        ("on".into(), Val::Bool(s.on)),
                        ("on-by-default".into(), Val::Bool(s.on_by_default)),
                        // THE STEP'S OWN KEYS, as its module declared them.
                        ("name-key".into(), Val::String(s.name_key.clone())),
                        ("does-key".into(), Val::String(s.does_key.clone())),
                    ])
                })
                .collect();

            let history: Vec<Val> = t.history.iter().map(&run_val).collect();
            Val::Record(vec![
                ("id".into(), Val::String(t.id.clone())),
                // THE NAME AS A WORD, beside the id. The component draws the
                // KEY and measures THIS — see `TaskState::name_text` — and the
                // order is the WIT's, because `wasmtime` zips a record
                // positionally and only then compares names.
                ("name-text".into(), Val::String(t.name_text.clone())),
                ("kind".into(), Val::Enum(task_kind_name(t.kind).to_owned())),
                (
                    "standing".into(),
                    Val::Enum(standing_name(t.standing).to_owned()),
                ),
                ("enabled".into(), Val::Bool(t.enabled)),
                ("interval-days".into(), Val::U16(t.interval_days)),
                (
                    "part-of-day".into(),
                    Val::Enum(day_part_name(t.part_of_day).to_owned()),
                ),
                ("next".into(), Val::U64(t.next)),
                ("next-text".into(), Val::String(t.next_text.clone())),
                ("hold".into(), Val::Enum(hold_kind_name(t.hold).to_owned())),
                ("why-not".into(), Val::String(t.why_not.clone())),
                ("ran".into(), Val::Bool(t.ran)),
                (
                    // `run_val` AND NOT A COPY. It was a copy — eight fields
                    // written out here and the same eight in the closure that
                    // built the history — and the copy fell a field behind the
                    // day the record grew one, which `wasmtime` reports as
                    // "expected 9 fields, got 8" about a record it will not
                    // name. One function, one shape.
                    "last".into(),
                    run_val(&t.last),
                ),
                ("history".into(), Val::List(history)),
                ("steps".into(), Val::List(steps)),
                // THE MODULES' HALF OF THE ROW: whether it keeps a calendar,
                // its keys, the generation of its page, and what it offers.
                ("calendar".into(), Val::Bool(t.calendar)),
                ("name-key".into(), Val::String(t.name_key.clone())),
                ("does-key".into(), Val::String(t.does_key.clone())),
                ("why-key".into(), Val::String(t.why_key.clone())),
                ("screen-generation".into(), Val::U64(t.screen_generation)),
                (
                    "actions".into(),
                    Val::List(t.actions.iter().map(action_val).collect()),
                ),
            ])
        })
        .collect();

    Val::Record(vec![
        ("now".into(), Val::U64(m.now)),
        (
            "pulse".into(),
            Val::Record(vec![
                ("at".into(), Val::U64(m.pulse.at)),
                ("signs".into(), Val::List(signs)),
            ]),
        ),
        ("tasks".into(), Val::List(tasks)),
        (
            "queue".into(),
            Val::List(m.queue.iter().map(|id| Val::String(id.clone())).collect()),
        ),
        ("running".into(), Val::String(m.running.clone())),
        ("rehearsing".into(), Val::String(m.rehearsing.clone())),
        (
            "announcement".into(),
            Val::Record(vec![
                ("task".into(), Val::String(m.announcement.task.clone())),
                (
                    "total-seconds".into(),
                    Val::U16(m.announcement.total_seconds),
                ),
                ("deadline".into(), Val::U64(m.announcement.deadline)),
                ("why".into(), Val::String(m.announcement.why.clone())),
            ]),
        ),
        (
            "gate".into(),
            Val::Record(vec![
                ("ram-ceiling-pct".into(), Val::U16(m.gate.ram_ceiling_pct)),
                (
                    "cpu-stall-per-mille".into(),
                    Val::U16(m.gate.cpu_stall_per_mille),
                ),
                (
                    "abort-on-battery".into(),
                    Val::Bool(m.gate.abort_on_battery),
                ),
            ]),
        ),
        (
            "cadence".into(),
            Val::List(m.cadence.iter().copied().map(Val::U16).collect()),
        ),
        // EVERYTHING THE BRAIN HAS DONE, folded, in the same record the
        // per-task history uses — one shape, so the screen that draws one
        // draws the other.
        (
            "activity-rows".into(),
            Val::List(m.activity_rows.iter().map(activity_row_val).collect()),
        ),
        // HOW MANY THAT LIST KEEPS. It crosses for the reason `cadence` does:
        // it is the daemon's policy and the screen's title names it.
        ("remembers".into(), Val::U32(m.remembers)),
        ("recent".into(), Val::U32(m.recent)),
        // EVERY NOTICE, in the order the strip ranks them.
        (
            "notices".into(),
            Val::List(m.notices.iter().map(notice_val).collect()),
        ),
    ])
}

/// A `demand` coming back from the guest.
///
/// Unknown request = `None`, like every other unknown case in this file: a
/// component built against a newer WIT is still useful for what the host
/// understands, and a request it cannot perform is better refused than guessed.
/// The plan behind `effect::format`.
///
/// # A field missing here is a refusal, never a default
///
/// Every other lift in this file tolerates a gap: a missing label draws an
/// empty string and nobody is hurt. This one writes a filesystem over somebody's
/// disk, so an unreadable field returns `None` and the effect never happens —
/// a `whole-disk` that failed to lift would otherwise read as `false` and
/// silently format the wrong thing.
fn plan_from(v: Option<&wasmtime::component::Val>) -> Option<ronoctua_protocol::Plan> {
    use wasmtime::component::Val;

    let Some(Val::Record(fields)) = v else {
        return None;
    };
    let get = |name: &str| fields.iter().find(|(n, _)| n == name).map(|(_, v)| v);

    let Some(Val::String(device)) = get("device") else {
        return None;
    };
    let Some(Val::Enum(recipe)) = get("recipe") else {
        return None;
    };
    let Some(recipe) = ronoctua_protocol::Recipe::of_wit_name(recipe) else {
        // A NAME THIS BUILD CANNOT WRITE. The WIT gained a case and the host
        // did not learn it, which on a disk is the one place to stop rather
        // than guess a neighbour.
        tracing::warn!(recipe, "an unknown filesystem");
        return None;
    };
    let Some(Val::String(label)) = get("label") else {
        return None;
    };
    let Some(Val::Bool(whole_disk)) = get("whole-disk") else {
        return None;
    };
    // AND WHETHER IT IS ENCRYPTED, refused when missing like the rest: a disk
    // written in the clear by a buffer that asked for a passphrase is the
    // quiet failure this lift exists to refuse.
    let Some(Val::Bool(encrypt)) = get("encrypt") else {
        return None;
    };

    Some(ronoctua_protocol::Plan {
        device: device.clone(),
        recipe,
        label: label.clone(),
        whole_disk: *whole_disk,
        encrypt: *encrypt,
    })
}

/// The setting behind `effect::tune`.
///
/// # THE FIRST VARIANT THIS FILE LIFTS FROM INSIDE A RECORD
///
/// Every other payload on the effect path is a string, an enum or a flat record
/// of them. `knob` is a `variant`, and a variant has a case name AND a payload
/// whose type depends on it — so the two have to be read together. Reading the
/// name and then trusting the payload's type is how `interval-days(7)` would
/// become `enabled(true)` with nothing said.
///
/// # A field missing here is a refusal, never a default
///
/// The same rule as `plan_from`, and for a nearby reason: what is being
/// remembered is when the machine rebuilds its own operating system. A
/// `part-of-day` that failed to lift would default to `any` and move a rebuild
/// out of the small hours into the middle of somebody's afternoon — a change
/// nobody asked for and nobody would connect to the key they pressed.
/// The remap the guest asked for, read back out of its record.
///
/// Three strings and no interpretation: the host is what decides whether they
/// name anything, because the buffer got them FROM the host in the first
/// place and a buffer that invented one is a buffer that gets refused.
fn remap_from(v: Option<&wasmtime::component::Val>) -> Option<ronoctua_protocol::Remap> {
    use wasmtime::component::Val;

    let Some(Val::Record(fields)) = v else {
        return None;
    };
    let get = |name: &str| fields.iter().find(|(n, _)| n == name).map(|(_, v)| v);
    let text = |name: &str| match get(name) {
        Some(Val::String(s)) => Some(s.clone()),
        _ => None,
    };
    Some(ronoctua_protocol::Remap {
        device: text("device")?,
        key: text("key")?,
        becomes: text("becomes")?,
    })
}

fn setting_from(v: Option<&wasmtime::component::Val>) -> Option<ronoctua_protocol::Setting> {
    use ronoctua_protocol::Knob;
    use wasmtime::component::Val;

    let Some(Val::Record(fields)) = v else {
        return None;
    };
    let get = |name: &str| fields.iter().find(|(n, _)| n == name).map(|(_, v)| v);

    let Some(Val::String(task)) = get("task") else {
        return None;
    };
    let Some(Val::Variant(case, payload)) = get("knob") else {
        return None;
    };

    let knob = match (case.as_str(), payload.as_deref()) {
        ("interval-days", Some(Val::U16(days))) => Knob::IntervalDays(*days),
        ("part-of-day", Some(Val::Enum(part))) => {
            let Some(part) = ronoctua_protocol::DayPart::of_wit_name(part) else {
                // A NAME THIS BUILD CANNOT READ. The WIT gained a case and the
                // host did not learn it; rounding it to a neighbour would put
                // the rebuild in a stretch of the day nobody chose.
                tracing::warn!(part, "an unknown part of the day");
                return None;
            };
            Knob::PartOfDay(part)
        }
        ("enabled", Some(Val::Bool(on))) => Knob::Enabled(*on),
        ("step", Some(Val::Record(f))) => {
            let g = |name: &str| f.iter().find(|(n, _)| n == name).map(|(_, v)| v);
            let (Some(Val::String(id)), Some(Val::Bool(on))) = (g("id"), g("on")) else {
                return None;
            };
            Knob::Step(ronoctua_protocol::StepSwitch {
                id: id.clone(),
                on: *on,
            })
        }
        // It carries nothing: there is one default and it is the flake's.
        ("reset-steps", None) => Knob::ResetSteps,
        (otherwise, _) => {
            tracing::warn!(knob = otherwise, "an unknown preference");
            return None;
        }
    };

    Some(ronoctua_protocol::Setting {
        task: task.clone(),
        knob,
    })
}

fn demand_from(v: Option<&wasmtime::component::Val>) -> Option<ronoctua_protocol::Demand> {
    use ronoctua_protocol::Request as R;
    use wasmtime::component::Val;

    let Some(Val::Record(fields)) = v else {
        return None;
    };
    let get = |name: &str| fields.iter().find(|(n, _)| n == name).map(|(_, v)| v);
    let Some(Val::Enum(what)) = get("what") else {
        return None;
    };
    let what = match what.as_str() {
        "refresh" => R::Refresh,
        "scan-ram" => R::ScanRam,
        "scan-disk" => R::ScanDisk,
        "cancel-scan" => R::CancelScan,
        "guard-tolerant" => R::GuardTolerant,
        "guard-default" => R::GuardDefault,
        "guard-persist" => R::GuardPersist,
        "stop-consumer" => R::StopConsumer,
        "sweep-generations" => R::SweepGenerations,
        "sweep-store" => R::SweepStore,
        "sweep-journal" => R::SweepJournal,
        "pause-job" => R::PauseJob,
        "resume-job" => R::ResumeJob,
        "cancel-job" => R::CancelJob,
        "forget-job" => R::ForgetJob,
        "sweep-jobs" => R::SweepJobs,
        "launch-job" => R::LaunchJob,
        "run-task" => R::RunTask,
        "rehearse-task" => R::RehearseTask,
        "abort-task" => R::AbortTask,
        "proceed" => R::Proceed,
        "postpone" => R::Postpone,
        "watch-screen" => R::WatchScreen,
        "act-row" => R::ActRow,
        "act-row-also" => R::ActRowAlso,
        "act-page" => R::ActPage,
        "dismiss-notice" => R::DismissNotice,
        "snooze-notice" => R::SnoozeNotice,
        "act-notice" => R::ActNotice,
        otherwise => {
            tracing::warn!(request = otherwise, "an unknown request");
            return None;
        }
    };
    let target = match get("target") {
        Some(Val::U64(n)) => *n,
        _ => 0,
    };
    Some(ronoctua_protocol::Demand { what, target })
}

/// The effects returned by `event`.
///
/// An unknown effect is IGNORED, for the same reason as an unknown command
/// in the display list: a component compiled against a newer WIT is still
/// useful for what the host understands.
fn effects_from(v: &wasmtime::component::Val) -> Vec<ronoctua_protocol::Effect> {
    use ronoctua_protocol::Effect;
    use wasmtime::component::Val;

    let Val::List(items) = v else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|x| {
            let Val::Variant(case, payload) = x else {
                return None;
            };
            let text = || match payload.as_deref() {
                Some(Val::String(s)) => Some(s.clone()),
                _ => None,
            };
            match case.as_str() {
                "redraw" => Some(Effect::Redraw),
                "copy" => text().map(Effect::Copy),
                "command" => text().map(Effect::Command),
                "close" => Some(Effect::Close),
                // IT WAS MISSING, AND THE EDITOR'S SAVE WAS THE PRICE.
                //
                // `buffers/editor` emits `effect::save` and
                // `crates/ronoctua-shell-buffers/src/buffer.rs` handles it — but nothing
                // ever built it: the case fell into `otherwise` below, came out
                // as a `warn` in a log nobody reads, and `SPC m s` looked like a
                // key that does nothing. Which is exactly what this arm's
                // neighbours were written to prevent.
                "save" => text().map(Effect::Save),
                "ask" => demand_from(payload.as_deref()).map(Effect::Ask),
                "format" => plan_from(payload.as_deref()).map(Effect::Format),
                "state-changed" => text().map(Effect::StateChanged),
                // THE ARM THAT `save` DID NOT HAVE, written in the same commit
                // as the effect. Its neighbour above spent a release as a
                // `warn` in a log nobody reads while `SPC m s` looked like a
                // key that does nothing; there is a test at the end of this
                // file so that `tune` cannot become the second one.
                "tune" => setting_from(payload.as_deref()).map(Effect::Tune),
                "remap" => remap_from(payload.as_deref()).map(Effect::Remap),
                "keyboard-mode" => match payload.as_deref() {
                    Some(Val::Enum(n)) if n == "independent" => Some(Effect::KeyboardMode(
                        ronoctua_protocol::KeyboardMode::Independent,
                    )),
                    Some(Val::Enum(n)) if n == "together" => Some(Effect::KeyboardMode(
                        ronoctua_protocol::KeyboardMode::Together,
                    )),
                    _ => None,
                },
                "unit-change" => unit_change_from(payload.as_deref()).map(Effect::UnitChange),
                otherwise => {
                    tracing::warn!(effect = otherwise, "an unknown effect");
                    None
                }
            }
        })
        .collect()
}

/// The descriptor returned by `init`.
fn descriptor_of(v: &wasmtime::component::Val) -> Result<ronoctua_protocol::Descriptor> {
    use ronoctua_protocol as prot;
    use wasmtime::component::Val;

    let Val::Record(fields) = v else {
        anyhow::bail!("`init` did not give back a record");
    };
    let field = |n: &str| fields.iter().find(|(k, _)| k == n).map(|(_, v)| v);
    let text_of = |v: Option<&Val>| match v {
        Some(Val::String(s)) => s.clone(),
        _ => String::new(),
    };
    let list_of_str = |v: Option<&Val>| -> Vec<String> {
        match v {
            Some(Val::List(l)) => l
                .iter()
                .filter_map(|x| match x {
                    Val::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    };

    let keymap = match field("keymap") {
        Some(Val::List(l)) => l
            .iter()
            .filter_map(|x| {
                let Val::Record(f) = x else { return None };
                let g = |n: &str| {
                    f.iter().find(|(k, _)| k == n).and_then(|(_, v)| match v {
                        Val::String(s) => Some(s.clone()),
                        _ => None,
                    })
                };
                Some(prot::Binding {
                    keys: g("keys")?,
                    command: g("command")?,
                })
            })
            .collect(),
        _ => Vec::new(),
    };

    let capabilities = match field("capabilities") {
        Some(Val::List(l)) => l
            .iter()
            .filter_map(|x| match x {
                Val::Enum(n) if n == "terminal" => Some(prot::Capability::Terminal),
                Val::Enum(n) if n == "raw-surface" => Some(prot::Capability::RawSurface),
                Val::Enum(n) if n == "vector" => Some(prot::Capability::Vector),
                Val::Enum(n) if n == "system" => Some(prot::Capability::System),
                Val::Enum(n) if n == "transfers" => Some(prot::Capability::Transfers),
                Val::Enum(n) if n == "disks" => Some(prot::Capability::Disks),
                Val::Enum(n) if n == "brain" => Some(prot::Capability::Brain),
                Val::Enum(n) if n == "keyboard" => Some(prot::Capability::Keyboard),
                // AN UNKNOWN CAPABILITY DISAPPEARS HERE, and that is the
                // quietest failure in this file: the buffer declared it, the
                // host did not learn to read it, and every gated effect is then
                // refused with a message saying "you did not declare it" —
                // which blames the buffer for the host's gap. A new case in the
                // WIT's `capability` needs an arm HERE, in the same commit.
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };

    // THE STATES, and the tone is read by NAME — the same reason the capability
    // list above says an unknown case must not disappear. A tone the host has
    // not learned to read would silently become `standard`, which draws the
    // right colour by accident until the day somebody uses `warning`.
    let states = match field("states") {
        Some(Val::List(l)) => l
            .iter()
            .filter_map(|x| {
                let Val::Record(f) = x else { return None };
                let g = |n: &str| f.iter().find(|(k, _)| k == n).map(|(_, v)| v);
                Some(prot::BufferState {
                    id: match g("id") {
                        Some(Val::String(s)) => s.clone(),
                        _ => return None,
                    },
                    letter: match g("letter") {
                        Some(Val::Char(c)) => *c,
                        _ => return None,
                    },
                    tone: match g("tone") {
                        Some(Val::Enum(n)) if n == "accent" => prot::Tone::Accent,
                        Some(Val::Enum(n)) if n == "success" => prot::Tone::Success,
                        Some(Val::Enum(n)) if n == "warning" => prot::Tone::Warning,
                        Some(Val::Enum(n)) if n == "error" => prot::Tone::Error,
                        _ => prot::Tone::Standard,
                    },
                })
            })
            .collect(),
        _ => Vec::new(),
    };

    Ok(prot::Descriptor {
        name: text_of(field("name")),
        mode: text_of(field("mode")),
        states,
        keymap,
        commands: list_of_str(field("commands")),
        phrases: list_of_str(field("phrases")),
        // Modeline segments arrive once the modeline consumes them; the
        // shape is already in the WIT so there is no protocol change later.
        segments: Vec::new(),
        capabilities,
    })
}

/// The display list returned by `render`.
///
/// The conversion from WIT's types to `ronoctua-protocol`'s happens
/// here: they are two representations of the SAME contract — one generated by the
/// bindgen, the other hand-written for the host — and keeping them separate is
/// what lets the protocol not depend on wasmtime.
fn display_list_from(v: &wasmtime::component::Val) -> Result<ronoctua_protocol::DisplayList> {
    use ronoctua_protocol as prot;
    use wasmtime::component::Val;

    let Val::List(commands) = v else {
        anyhow::bail!("`render` did not give back a list");
    };

    let mut list = prot::DisplayList::fresh_one();
    for c in commands {
        let Val::Variant(case, payload) = c else {
            continue;
        };
        let Some(payload) = payload.as_deref() else {
            continue;
        };
        let Val::Record(fields) = payload else {
            continue;
        };
        let field = |name: &str| fields.iter().find(|(n, _)| n == name).map(|(_, v)| v);

        let u16_de = |v: Option<&Val>| match v {
            Some(Val::U16(n)) => *n,
            _ => 0,
        };
        let u32_de = |v: Option<&Val>| match v {
            Some(Val::U32(n)) => *n,
            _ => 0,
        };
        // The colour arrives as an ENUM, and no longer as a record with a string.
        //
        // The `Val::Enum` carries the CASE's name in `kebab-case`
        // (`text-primary`), which is the same name the token generator
        // emits on both sides. A case that does not exist never gets here:
        // the guest cannot produce it, because over there it is an enum.
        let color_of = |v: Option<&Val>| -> prot::Color {
            match v {
                Some(Val::Enum(name)) => prot::Color(name.clone().into()),
                // There is no `unwrap_or` with a pretty default: a colour that
                // did not come is a protocol violation, and the host refuses the
                // command further along instead of papering over it.
                _ => prot::Color(String::new().into()),
            }
        };

        match case.as_str() {
            "grid" => {
                let g = |n: &str| fields.iter().find(|(k, _)| k == n).map(|(_, v)| v);
                let text_lines = match g("lines") {
                    Some(Val::List(l)) => l
                        .iter()
                        .filter_map(|x| match x {
                            Val::String(s) => Some(s.clone()),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                list.push(prot::Command::Grid {
                    line: u16_de(g("line")),
                    column: u16_de(g("column")),
                    lines: text_lines,
                    color: color_of(g("color")),
                });
            }
            "text" => {
                let content = match field("content") {
                    Some(Val::Variant(case, inside)) => match (case.as_str(), inside.as_deref()) {
                        ("key", Some(Val::String(s))) => prot::Text::key(s.clone()),
                        // The ICON arrives as an ENUM, not a string: the token's
                        // name comes from the generated enum, so a name that does
                        // not exist does not compile in the guest.
                        ("icon", Some(Val::Enum(n))) => prot::Text::icon(n.clone()),
                        (_, Some(Val::String(s))) => prot::Text::datum(s.clone()),
                        _ => continue,
                    },
                    _ => continue,
                };
                list.push(prot::Command::Text {
                    line: u16_de(field("line")),
                    column: u16_de(field("column")),
                    text: content,
                    color: color_of(field("color")),
                    scale: scale_of(field("scale")),
                    max_width: u16_de(field("max-width")),
                    edge: matches!(field("edge"), Some(Val::Bool(true))),
                });
            }
            "rule" => list.push(prot::Command::Rule {
                line: u16_de(field("line")),
                column: u16_de(field("column")),
                columns: u16_de(field("columns")),
                color: color_of(field("color")),
            }),
            "selection" => list.push(prot::Command::Selection {
                line: u16_de(field("line")),
                column: u16_de(field("column")),
                columns: u16_de(field("columns")),
                color: color_of(field("color")),
            }),
            "svg" => {
                // AN EMPTY SVG IS NOT A COMMAND. It would reach the rasteriser, which
                // would return `None`, and the area would stay blank — which is the
                // same as the buffer not having asked for anything. Dropping it here leaves
                // the list telling the truth about what will be drawn.
                let Some(Val::String(svg)) = field("svg") else {
                    continue;
                };
                if svg.is_empty() {
                    continue;
                }
                list.push(prot::Command::Svg {
                    line: u16_de(field("line")),
                    column: u16_de(field("column")),
                    lines: u16_de(field("lines")),
                    columns: u16_de(field("columns")),
                    svg: svg.clone(),
                    color: color_of(field("color")),
                });
            }
            "paragraph" => {
                let content = match field("content") {
                    Some(Val::Variant(case, inside)) => match (case.as_str(), inside.as_deref()) {
                        ("key", Some(Val::String(s))) => prot::Text::key(s.clone()),
                        ("icon", Some(Val::Enum(n))) => prot::Text::icon(n.clone()),
                        (_, Some(Val::String(s))) => prot::Text::datum(s.clone()),
                        _ => continue,
                    },
                    _ => continue,
                };
                list.push(prot::Command::Paragraph {
                    line: u16_de(field("line")),
                    column: u16_de(field("column")),
                    lines: u16_de(field("lines")),
                    columns: u16_de(field("columns")),
                    content,
                    color: color_of(field("color")),
                    scale: scale_of(field("scale")),
                    first_row: u16_de(field("first-row")),
                });
            }
            "bar" => list.push(prot::Command::Bar {
                line: u16_de(field("line")),
                lines: u16_de(field("lines")),
                column: u16_de(field("column")),
                total: u32_de(field("total")),
                shown: u32_de(field("shown")),
                offset: u32_de(field("offset")),
                color: color_of(field("color")),
            }),
            // A FACT ABOUT THE FRAME, and the only command that draws
            // nothing: from this line down the host places the ink counting UP
            // from the window's bottom, so the surface's residue falls where
            // nothing is drawn. See `prot::Command::Anchor`.
            "anchor" => list.push(prot::Command::Anchor {
                foot_from: u16_de(field("foot-from")),
            }),
            otherwise => {
                tracing::warn!(
                    command = otherwise,
                    "an unknown command in the display list"
                )
            }
        }
    }
    // Ignore the unknown, and do not fail: a component compiled
    // against a newer WIT is still useful for the commands the host
    // understands. Failing would lose the whole list over one command.
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_engine_comes_up_with_the_configuration_of_section_5_2() {
        // If the pooling allocator is refused on the platform, it is better to find out
        // here than at the first buffer opened.
        assert!(Runtime::fresh().is_ok());
    }

    /// The names of a record's fields, in the order they were built.
    fn shape(v: &wasmtime::component::Val) -> Vec<&str> {
        match v {
            wasmtime::component::Val::Record(f) => f.iter().map(|(n, _)| n.as_str()).collect(),
            _ => Vec::new(),
        }
    }

    /// Looks a field up by name, so the assertions do not depend on position.
    fn field<'a>(
        v: &'a wasmtime::component::Val,
        name: &str,
    ) -> Option<&'a wasmtime::component::Val> {
        match v {
            wasmtime::component::Val::Record(f) => {
                f.iter().find(|(n, _)| n == name).map(|(_, v)| v)
            }
            _ => None,
        }
    }

    /// A phrase is two strings, and the ORDER of the two is the contract.
    ///
    /// Swapped, every buffer would read the text as its key and the key as its
    /// text — and it would not fail, it would draw `[family]` where the word
    /// goes. The same reason as the shape test below, on the smallest record
    /// this file lowers.
    #[test]
    fn a_phrase_has_the_wit_shape_field_by_field() {
        let v = val_of_phrases(&[ronoctua_protocol::Phrase {
            key: "unicode-prefix-family".to_owned(),
            text: "familia".to_owned(),
        }]);

        let wasmtime::component::Val::List(one) = &v else {
            panic!("a list of phrases");
        };
        assert_eq!(shape(&one[0]), ["key", "text"]);
        assert_eq!(
            field(&one[0], "key"),
            Some(&wasmtime::component::Val::String(
                "unicode-prefix-family".to_owned()
            ))
        );
        assert_eq!(
            field(&one[0], "text"),
            Some(&wasmtime::component::Val::String("familia".to_owned()))
        );
    }

    /// THE SHAPE IS THE CONTRACT, and nothing else checks it.
    ///
    /// `wasmtime` lowers a record by zipping it POSITIONALLY against the WIT and
    /// only then compares names — so a field moved in `val_of_survey` without
    /// being moved in `buffer.wit` fails when the guest is CALLED, with a
    /// message about a name and a stack that points at the call. Here the same
    /// mistake fails in `cargo test`, and the list below is the `.wit`'s order
    /// copied by hand on purpose: it is the second pair of eyes.
    #[test]
    fn the_survey_has_the_wit_shape_field_by_field() {
        let s = ronoctua_protocol::Survey {
            readings: vec![ronoctua_protocol::Reading {
                of: ronoctua_protocol::Gauge::RamTotal,
                bytes: 16_000_000_000,
            }],
            consumers: vec![ronoctua_protocol::Consumer {
                kind: ronoctua_protocol::ConsumerKind::Scope,
                name: "ronoctua-buffer-9f3a.scope".to_owned(),
                bytes: 42,
                hint: 4321,
            }],
            ..Default::default()
        };
        let v = val_of_survey(&s);

        assert_eq!(
            shape(&v),
            [
                "readings",
                "pressure",
                "consumers",
                "guard",
                "scanning",
                "progress"
            ]
        );
        assert_eq!(
            shape(field(&v, "pressure").expect("pressure")),
            ["some10", "some60", "some300", "full10", "full60"]
        );
        assert_eq!(
            shape(field(&v, "guard").expect("guard")),
            [
                "mode",
                "pressure-limit",
                "pressure-seconds",
                "ceiling",
                "acted",
                "zram",
                "persisted"
            ]
        );

        let wasmtime::component::Val::List(readings) = field(&v, "readings").expect("readings")
        else {
            panic!("readings is not a list")
        };
        assert_eq!(shape(&readings[0]), ["of", "bytes"]);
        let wasmtime::component::Val::List(consumers) = field(&v, "consumers").expect("consumers")
        else {
            panic!("consumers is not a list")
        };
        assert_eq!(shape(&consumers[0]), ["kind", "name", "bytes", "hint"]);

        // And the enums cross by NAME, in the WIT's kebab — not by number.
        assert_eq!(
            field(&readings[0], "of"),
            Some(&wasmtime::component::Val::Enum("ram-total".to_owned()))
        );
    }

    #[test]
    fn the_rack_has_the_wit_shape_field_by_field() {
        // THE LIST BELOW IS THE `.wit`'s ORDER, COPIED BY HAND, for the same
        // reason as the queue's above — and with more at stake: two fields of
        // this record are `bool`, and a pair of booleans swapped positionally
        // still lowers CLEANLY. `mounted` and `removable` traded would draw an
        // eject glyph on a mounted system volume and no complaint anywhere.
        let r = ronoctua_protocol::Rack {
            devices: vec![ronoctua_protocol::Device {
                id: "/org/freedesktop/UDisks2/block_devices/sda2".to_owned(),
                name: "MISO_EFI".to_owned(),
                node: "/dev/sda2".to_owned(),
                kind: "vfat".to_owned(),
                bytes: 41_943_040,
                mounted: false,
                removable: true,
                needed: String::new(),
                whole: false,
            }],
        };
        let v = val_of_rack(&r);

        assert_eq!(shape(&v), ["devices"]);
        let wasmtime::component::Val::List(devices) = field(&v, "devices").expect("devices") else {
            panic!("devices is not a list")
        };
        assert_eq!(
            shape(&devices[0]),
            [
                "id",
                "name",
                "node",
                "kind",
                "bytes",
                "mounted",
                "removable",
                "needed",
                "whole"
            ]
        );

        // AND THE TWO BOOLEANS ARE READ BACK BY NAME, which is the half the
        // shape check cannot do: the shape would still pass with the values
        // swapped, because both fields are the same type.
        assert_eq!(
            field(&devices[0], "mounted"),
            Some(&wasmtime::component::Val::Bool(false))
        );
        assert_eq!(
            field(&devices[0], "removable"),
            Some(&wasmtime::component::Val::Bool(true))
        );
    }

    #[test]
    fn a_plan_missing_a_field_is_refused_rather_than_defaulted() {
        use wasmtime::component::Val;

        let whole = |b: bool| {
            Val::Record(vec![
                ("device".into(), Val::String("sda".into())),
                ("recipe".into(), Val::Enum("universal".into())),
                ("label".into(), Val::String("PENDRIVE".into())),
                ("whole-disk".into(), Val::Bool(b)),
                ("encrypt".into(), Val::Bool(!b)),
            ])
        };

        // The whole plan crosses, and `whole-disk` keeps the value it was given.
        let got = plan_from(Some(&whole(true))).expect("a full plan");
        assert_eq!(got.device, "sda");
        assert_eq!(got.recipe.filesystem(), "vfat");
        assert_eq!(got.label, "PENDRIVE");
        assert!(got.whole_disk);
        assert!(!got.encrypt, "`encrypt` keeps its own value");
        assert!(plan_from(Some(&whole(false))).expect("a full plan").encrypt);
        // A PLAN THAT SAYS NOTHING ABOUT ENCRYPTION is refused, not written in
        // the clear.
        let silent = Val::Record(vec![
            ("device".into(), Val::String("sda".into())),
            ("recipe".into(), Val::Enum("universal".into())),
            ("label".into(), Val::String("PENDRIVE".into())),
            ("whole-disk".into(), Val::Bool(true)),
        ]);
        assert!(plan_from(Some(&silent)).is_none());

        // AND THE GAP IS A REFUSAL. `Plan` derives `Default`, so the tempting
        // shape here is `unwrap_or_default()` — which would turn a `whole-disk`
        // that failed to cross into `false` and format a partition where the
        // buffer asked for a disk, or the reverse. There is no safe default for
        // a field that decides how much gets erased.
        let short = Val::Record(vec![
            ("device".into(), Val::String("sda".into())),
            ("recipe".into(), Val::Enum("universal".into())),
            ("label".into(), Val::String("PENDRIVE".into())),
        ]);
        assert!(plan_from(Some(&short)).is_none());

        // A filesystem this build cannot write is refused too, and not rounded
        // to a neighbour: `xfs` is exactly the case the slim udisks has no
        // `mkfs` for.
        let unknown = Val::Record(vec![
            ("device".into(), Val::String("sda".into())),
            ("recipe".into(), Val::Enum("xfs".into())),
            ("label".into(), Val::String(String::new())),
            ("whole-disk".into(), Val::Bool(false)),
            ("encrypt".into(), Val::Bool(false)),
        ]);
        assert!(plan_from(Some(&unknown)).is_none());
    }

    #[test]
    fn the_queue_has_the_wit_shape_field_by_field() {
        // THE LIST BELOW IS THE `.wit`'s ORDER, COPIED BY HAND on purpose. It
        // is the second pair of eyes on `val_of_queue`: `wasmtime` zips a
        // record positionally, so a field swapped in one file alone fails only
        // when the guest is CALLED, and the message names a field rather than
        // the swap.
        let q = ronoctua_protocol::Queue {
            jobs: vec![ronoctua_protocol::Job {
                id: 7,
                engine: ronoctua_protocol::Engine::Torrent,
                errand: ronoctua_protocol::Errand::MoveIt,
                stage: ronoctua_protocol::Stage::Running,
                amount: ronoctua_protocol::Amount::Bytes,
                done: 512,
                total: 2048,
                subject: "debian.iso".to_owned(),
                trouble: String::new(),
                answer: 0,
                identified: true,
                pauses: true,
                opens: false,
            }],
        };
        let v = val_of_queue(&q);

        assert_eq!(shape(&v), ["jobs"]);
        let wasmtime::component::Val::List(jobs) = field(&v, "jobs").expect("jobs") else {
            panic!("jobs is not a list")
        };
        assert_eq!(
            shape(&jobs[0]),
            [
                "id",
                "engine",
                "errand",
                "stage",
                "amount",
                "done",
                "total",
                "subject",
                "trouble",
                "answer",
                // The one that cannot be undone after the fact, so it is on the
                // row and not only in a log.
                "identified",
                // What the row's keys may offer: holding it, and opening what
                // an install made.
                "pauses",
                "opens"
            ]
        );

        // The enums cross by NAME, and `move-it` is the case that would be
        // silently wrong if it were spelled `move` on one side only.
        assert_eq!(
            field(&jobs[0], "errand"),
            Some(&wasmtime::component::Val::Enum("move-it".to_owned()))
        );
        assert_eq!(
            field(&jobs[0], "engine"),
            Some(&wasmtime::component::Val::Enum("torrent".to_owned()))
        );
    }

    #[test]
    fn the_states_come_back_whole_and_an_unknown_tone_is_the_standard_one() {
        // THE DESCRIPTOR CROSSES BY NAME and not by position — `descriptor_of`
        // looks each field up — so what this proves is the other half: that the
        // TONE is read by its case name, and that a case this host has not
        // learned to read falls back to `standard` instead of vanishing.
        //
        // The fallback is deliberate and it is the reason a wrong tone is a
        // colour and never a missing state: losing the state would lose the row.
        use wasmtime::component::Val;
        let d = Val::Record(vec![
            ("name".into(), Val::String("glyphs".into())),
            ("mode".into(), Val::String("glyphs".into())),
            (
                "states".into(),
                Val::List(vec![
                    Val::Record(vec![
                        ("id".into(), Val::String("normal".into())),
                        ("letter".into(), Val::Char('n')),
                        ("tone".into(), Val::Enum("standard".into())),
                    ]),
                    Val::Record(vec![
                        ("id".into(), Val::String("filter".into())),
                        ("letter".into(), Val::Char('f')),
                        ("tone".into(), Val::Enum("warning".into())),
                    ]),
                    Val::Record(vec![
                        ("id".into(), Val::String("sheet".into())),
                        ("letter".into(), Val::Char('s')),
                        ("tone".into(), Val::Enum("a-tone-from-the-future".into())),
                    ]),
                ]),
            ),
            ("keymap".into(), Val::List(vec![])),
            ("commands".into(), Val::List(vec![])),
            ("segments".into(), Val::List(vec![])),
            ("capabilities".into(), Val::List(vec![])),
        ]);

        let got = descriptor_of(&d).expect("descriptor");
        assert_eq!(
            got.states
                .iter()
                .map(|s| (s.id.as_str(), s.letter, s.tone))
                .collect::<Vec<_>>(),
            [
                ("normal", 'n', ronoctua_protocol::Tone::Standard),
                ("filter", 'f', ronoctua_protocol::Tone::Warning),
                ("sheet", 's', ronoctua_protocol::Tone::Standard),
            ]
        );
    }

    #[test]
    fn a_buffer_with_no_state_or_a_wrong_base_is_refused_at_load() {
        // The three refusals are at LOAD, next to the keymap's prefix guard and
        // for the same reason: each is a way of putting a word on the modeline
        // that no key can get out of.
        use ronoctua_protocol::{BufferState, Descriptor, Tone};
        let normal = BufferState {
            id: "normal".to_owned(),
            letter: 'n',
            tone: Tone::Standard,
        };

        let none = Descriptor {
            name: "x".to_owned(),
            ..Descriptor::default()
        };
        assert!(
            none.validate().is_err(),
            "no state at all has to be refused"
        );

        let wrong_base = Descriptor {
            name: "x".to_owned(),
            states: vec![BufferState {
                id: "search".to_owned(),
                ..normal
            }],
            ..Descriptor::default()
        };
        assert!(
            wrong_base.validate().is_err(),
            "a base other than `normal` has to be refused: the colour decides by that id"
        );

        let same_letter = Descriptor {
            name: "x".to_owned(),
            states: vec![
                normal.clone(),
                BufferState {
                    id: "filter".to_owned(),
                    letter: 'n',
                    tone: Tone::Standard,
                },
            ],
            ..Descriptor::default()
        };
        assert!(
            same_letter.validate().is_err(),
            "two identical letters leave a state unreachable under `SPC w s`"
        );

        let good = Descriptor {
            name: "x".to_owned(),
            states: vec![
                normal,
                BufferState {
                    id: "filter".to_owned(),
                    letter: 'f',
                    tone: Tone::Standard,
                },
            ],
            ..Descriptor::default()
        };
        assert!(good.validate().is_ok());
    }

    #[test]
    fn a_queue_request_needs_the_queue_capability() {
        // The gate that decides is the REQUEST and not the effect, because
        // `Effect::Ask` now carries requests of two different capabilities.
        use ronoctua_protocol::{Capability, Demand, Effect, Request};
        let cancel = Effect::Ask(Demand {
            what: Request::CancelJob,
            target: 3,
        });
        assert_eq!(cancel.requires(), Some(Capability::Transfers));
        let sweep = Effect::Ask(Demand {
            what: Request::SweepStore,
            target: 0,
        });
        assert_eq!(sweep.requires(), Some(Capability::System));
    }

    /// A demand comes back whole, and an unknown one comes back as nothing.
    #[test]
    fn a_demand_survives_the_round_trip_and_an_unknown_one_does_not() {
        use wasmtime::component::Val;
        let good = Val::Record(vec![
            ("what".into(), Val::Enum("stop-consumer".to_owned())),
            ("target".into(), Val::U64(4321)),
        ]);
        assert_eq!(
            demand_from(Some(&good)),
            Some(ronoctua_protocol::Demand {
                what: ronoctua_protocol::Request::StopConsumer,
                target: 4321,
            })
        );

        // A request this host does not know is REFUSED, and not guessed: acting
        // on the closest match would be the host inventing what the buffer meant.
        let strange = Val::Record(vec![
            ("what".into(), Val::Enum("format-the-disk".to_owned())),
            ("target".into(), Val::U64(0)),
        ]);
        assert_eq!(demand_from(Some(&strange)), None);
    }

    /// The editor's save had no way back, and this is the regression net.
    #[test]
    fn saving_is_an_effect_the_host_can_actually_build() {
        use wasmtime::component::Val;
        let list = Val::List(vec![Val::Variant(
            "save".to_owned(),
            Some(Box::new(Val::String("conteudo".to_owned()))),
        )]);
        assert_eq!(
            effects_from(&list),
            vec![ronoctua_protocol::Effect::Save("conteudo".to_owned())]
        );
    }

    #[test]
    fn the_mind_has_the_wit_shape_field_by_field() {
        // THE LISTS BELOW ARE THE `.wit`'s ORDER, COPIED BY HAND, for the same
        // reason as the rack's above — and with more at stake than any of them:
        // `task-state` carries `enabled`, `ran` and three step booleans, and
        // `run-record` carries four strings in a row. Any pair of them traded
        // lowers CLEANLY and comes out on screen as a task that is off when it
        // is on, or a failure attributed to the wrong step.
        let m = ronoctua_protocol::Mind {
            remembers: 100,
            recent: 10,
            // ONE OF EACH of the modules' lists, so their shapes are walked
            // below and not merely declared.
            activity_rows: vec![ronoctua_protocol::ActivityRow {
                newest: ronoctua_protocol::RunRecord {
                    task: "update".to_owned(),
                    outcome: ronoctua_protocol::OutcomeKind::Noted,
                    says: "fixture-says".to_owned(),
                    says_text: "It said so".to_owned(),
                    ..ronoctua_protocol::RunRecord::default()
                },
                repeats: 2,
                last_text: "2026-02-01 03:00".to_owned(),
                first_text: "2026-01-31 03:00".to_owned(),
                members: vec![ronoctua_protocol::RunRecord::default()],
            }],
            notices: vec![ronoctua_protocol::Notice {
                module: "fixture".to_owned(),
                id: "due".to_owned(),
                says: "fixture-says-due".to_owned(),
                says_text: "It is due".to_owned(),
                args: vec![ronoctua_protocol::NoticeArg {
                    name: "when".to_owned(),
                    text: "03:00".to_owned(),
                }],
                level: ronoctua_protocol::NoticeLevel::Alarm,
                since: 1_769_999_000,
                since_text: "2026-02-01 02:43".to_owned(),
                controls: vec![ronoctua_protocol::ModuleAction {
                    id: "forget".to_owned(),
                    label_key: "cmd-fixture-forget".to_owned(),
                    row: false,
                    asks_key: String::new(),
                }],
                dismissable: true,
                snooze: vec![5, 10],
            }],
            now: 1_770_000_000,
            pulse: ronoctua_protocol::Pulse {
                at: 1_769_999_940,
                signs: vec![ronoctua_protocol::VitalSign {
                    of: ronoctua_protocol::Vital::CpuStall,
                    amount: 37,
                }],
            },
            tasks: vec![ronoctua_protocol::TaskState {
                // ROUTINE AND `Once` BOTH GET DRAWN by the driver; here the
                // shape is what is being pinned, so one is enough.
                kind: ronoctua_protocol::TaskKind::Routine,
                id: "update".to_owned(),
                // THE NAME AS THE HOST RESOLVED IT. The component measures
                // this one and draws the key beside it, so a fixture with an
                // empty string would pin a record the screen never sees.
                name_text: "Update the system".to_owned(),
                standing: ronoctua_protocol::Standing::Queued,
                enabled: true,
                interval_days: 7,
                part_of_day: ronoctua_protocol::DayPart::Night,
                next: 1_770_010_000,
                next_text: "2026-02-02 03:00".to_owned(),
                hold: ronoctua_protocol::HoldKind::Battery,
                why_not: String::new(),
                ran: true,
                // A HISTORY WITH SOMETHING IN IT, because this test reads
                // `history[0]`'s shape: the record it shares with `last` is
                // marshalled by one function, and pinning it here is what
                // proves the two have not become two.
                history: vec![ronoctua_protocol::RunRecord {
                    task: "made-up".to_owned(),
                    task_text: "Update the system".to_owned(),
                    began: 1_769_800_000,
                    ended: 1_769_800_100,
                    when_text: "2026-01-30 09:00".to_owned(),
                    outcome: ronoctua_protocol::OutcomeKind::Held,
                    at: String::new(),
                    why: "ram".to_owned(),
                    trouble: String::new(),
                    steps: Vec::new(),
                    ..ronoctua_protocol::RunRecord::default()
                }],
                last: ronoctua_protocol::RunRecord {
                    task: "made-up".to_owned(),
                    task_text: "Update the system".to_owned(),
                    began: 1_769_900_000,
                    ended: 1_769_900_600,
                    when_text: "2026-02-01 03:00".to_owned(),
                    outcome: ronoctua_protocol::OutcomeKind::Failed,
                    at: "build".to_owned(),
                    why: String::new(),
                    trouble: "error: attribute missing".to_owned(),
                    steps: vec![ronoctua_protocol::StepOutcome {
                        id: "build".to_owned(),
                        ok: false,
                        seconds: 600,
                        trouble: "error: attribute missing".to_owned(),
                    }],
                    task_key: "brain-task-update".to_owned(),
                    at_key: "fixture-step-build".to_owned(),
                    ..ronoctua_protocol::RunRecord::default()
                },
                steps: vec![ronoctua_protocol::TaskStep {
                    id: "build".to_owned(),
                    command: "nix build .#system".to_owned(),
                    needs_root: false,
                    on: true,
                    on_by_default: false,
                    name_key: "fixture-step-build".to_owned(),
                    does_key: "fixture-step-build-does".to_owned(),
                }],
                // A CALENDAR THAT IS NOT KEPT, so the one boolean the task's
                // page reads to hide its cadence is pinned as sent.
                calendar: false,
                name_key: "brain-task-update".to_owned(),
                does_key: "brain-task-update-does".to_owned(),
                why_key: String::new(),
                screen_generation: 4,
                actions: vec![ronoctua_protocol::ModuleAction {
                    id: "keep".to_owned(),
                    label_key: "cmd-fixture-keep".to_owned(),
                    row: true,
                    asks_key: "ask-fixture-keep".to_owned(),
                }],
            }],
            queue: vec!["update".to_owned()],
            running: String::new(),
            rehearsing: "update".to_owned(),
            announcement: ronoctua_protocol::Announcement {
                task: "update".to_owned(),
                total_seconds: 60,
                deadline: 1_770_000_060,
                why: "due".to_owned(),
            },
            gate: ronoctua_protocol::Gate {
                ram_ceiling_pct: 80,
                cpu_stall_per_mille: 200,
                abort_on_battery: true,
            },
            cadence: vec![1, 3, 7, 14, 30],
        };
        let v = val_of_mind(&m);

        assert_eq!(
            shape(&v),
            [
                "now",
                "pulse",
                "tasks",
                "queue",
                "running",
                "rehearsing",
                "announcement",
                "gate",
                "cadence",
                "activity-rows",
                "remembers",
                "recent",
                "notices"
            ]
        );
        assert_eq!(shape(field(&v, "pulse").expect("pulse")), ["at", "signs"]);
        let wasmtime::component::Val::List(notices) = field(&v, "notices").expect("notices") else {
            panic!("notices is not a list")
        };
        assert_eq!(
            shape(&notices[0]),
            [
                "module",
                "id",
                "says",
                "says-text",
                "args",
                "level",
                "since",
                "since-text",
                "controls",
                "dismissable",
                "snooze"
            ]
        );
        assert_eq!(
            field(&notices[0], "level"),
            Some(&wasmtime::component::Val::Enum("alarm".to_owned()))
        );
        assert_eq!(
            field(&notices[0], "dismissable"),
            Some(&wasmtime::component::Val::Bool(true))
        );
        let wasmtime::component::Val::List(controls) =
            field(&notices[0], "controls").expect("controls")
        else {
            panic!("controls is not a list")
        };
        let wanted_action = ["id", "label-key", "row", "asks-key"];
        assert_eq!(shape(&controls[0]), wanted_action);
        let wasmtime::component::Val::List(rows) =
            field(&v, "activity-rows").expect("activity rows")
        else {
            panic!("activity-rows is not a list")
        };
        assert_eq!(
            shape(&rows[0]),
            ["newest", "repeats", "last-text", "first-text", "members"]
        );
        assert_eq!(
            field(&rows[0], "repeats"),
            Some(&wasmtime::component::Val::U32(2))
        );
        assert_eq!(
            shape(field(&v, "announcement").expect("announcement")),
            ["task", "total-seconds", "deadline", "why"]
        );
        assert_eq!(
            shape(field(&v, "gate").expect("gate")),
            ["ram-ceiling-pct", "cpu-stall-per-mille", "abort-on-battery"]
        );

        let wasmtime::component::Val::List(signs) =
            field(field(&v, "pulse").expect("pulse"), "signs").expect("signs")
        else {
            panic!("signs is not a list")
        };
        assert_eq!(shape(&signs[0]), ["of", "amount"]);
        assert_eq!(
            field(&signs[0], "of"),
            Some(&wasmtime::component::Val::Enum("cpu-stall".to_owned()))
        );

        let wasmtime::component::Val::List(tasks) = field(&v, "tasks").expect("tasks") else {
            panic!("tasks is not a list")
        };
        assert_eq!(
            shape(&tasks[0]),
            [
                "id",
                // THE NAME AS A WORD, for the component to MEASURE. It draws
                // the key beside it; this is the width it could not know.
                "name-text",
                "kind",
                "standing",
                "enabled",
                "interval-days",
                "part-of-day",
                "next",
                "next-text",
                "hold",
                "why-not",
                "ran",
                "last",
                "history",
                "steps",
                "calendar",
                "name-key",
                "does-key",
                "why-key",
                "screen-generation",
                "actions"
            ]
        );
        let last = field(&tasks[0], "last").expect("last");
        let wasmtime::component::Val::List(outcomes) = field(last, "steps").expect("outcomes")
        else {
            panic!("the registry's steps are not a list")
        };
        assert_eq!(shape(&outcomes[0]), ["id", "ok", "seconds", "trouble"]);
        let wasmtime::component::Val::List(steps) = field(&tasks[0], "steps").expect("steps")
        else {
            panic!("steps is not a list")
        };
        assert_eq!(
            shape(&steps[0]),
            [
                "id",
                "command",
                "needs-root",
                "on",
                "on-by-default",
                "name-key",
                "does-key"
            ]
        );
        // AND THE RECORD THE HISTORY AND THE ACTIVITY LOG SHARE. It is one
        // shape in two places on purpose — `run_val` is a function and not a
        // closure for exactly that — so pinning it once pins both.
        let wanted_run = [
            "task",
            "task-text",
            "began",
            "ended",
            "when-text",
            "outcome",
            "at",
            "why",
            "trouble",
            "steps",
            "task-key",
            "at-key",
            "why-key",
            "says",
            "says-text",
        ];
        assert_eq!(shape(field(&tasks[0], "last").expect("last")), wanted_run);
        let wasmtime::component::Val::List(history) = field(&tasks[0], "history").expect("history")
        else {
            panic!("history is not a list")
        };
        assert_eq!(shape(&history[0]), wanted_run);
        assert_eq!(
            shape(field(&rows[0], "newest").expect("newest")),
            wanted_run
        );
        // THE KEYS ARE THE ONES SENT, and not blanks the host forgot to fill:
        // `at-key` and `task-key` are two strings side by side.
        assert_eq!(
            field(last, "at-key"),
            Some(&wasmtime::component::Val::String(
                "fixture-step-build".to_owned()
            ))
        );
        let wasmtime::component::Val::List(actions) = field(&tasks[0], "actions").expect("actions")
        else {
            panic!("actions is not a list")
        };
        assert_eq!(shape(&actions[0]), wanted_action);
        assert_eq!(
            field(&actions[0], "row"),
            Some(&wasmtime::component::Val::Bool(true))
        );
        assert_eq!(
            field(&tasks[0], "calendar"),
            Some(&wasmtime::component::Val::Bool(false))
        );
        assert_eq!(
            field(&tasks[0], "screen-generation"),
            Some(&wasmtime::component::Val::U64(4))
        );

        // AND THE BOOLEANS ARE READ BACK BY NAME, which is the half the shape
        // check cannot do: it would still pass with any two of them swapped,
        // because they are the same type. `enabled` and `ran` traded would draw
        // a task that never ran as switched off.
        assert_eq!(
            field(&tasks[0], "enabled"),
            Some(&wasmtime::component::Val::Bool(true))
        );
        assert_eq!(
            field(&steps[0], "needs-root"),
            Some(&wasmtime::component::Val::Bool(false))
        );
        assert_eq!(
            field(&steps[0], "on"),
            Some(&wasmtime::component::Val::Bool(true))
        );
        // THE TWO THAT ARE EASIEST TO TRADE, read back by name and with
        // DIFFERENT values so a swap cannot pass. A step that is on and ships
        // off is exactly the row that used to draw wrong.
        assert_eq!(
            field(&steps[0], "on-by-default"),
            Some(&wasmtime::component::Val::Bool(false))
        );
        assert_eq!(
            field(&outcomes[0], "ok"),
            Some(&wasmtime::component::Val::Bool(false))
        );

        // The enums cross by NAME, in the WIT's kebab.
        assert_eq!(
            field(&tasks[0], "part-of-day"),
            Some(&wasmtime::component::Val::Enum("night".to_owned()))
        );
        assert_eq!(
            field(&tasks[0], "kind"),
            Some(&wasmtime::component::Val::Enum("routine".to_owned()))
        );
        assert_eq!(
            field(&tasks[0], "hold"),
            Some(&wasmtime::component::Val::Enum("battery".to_owned()))
        );
        assert_eq!(
            field(last, "outcome"),
            Some(&wasmtime::component::Val::Enum("failed".to_owned()))
        );
    }

    /// THE NET `save` DID NOT HAVE, and the reason this one exists at all.
    ///
    /// `effects_from` had no arm for `save`: the editor emitted it, the case
    /// fell into `otherwise`, and the whole failure was a `warn!` in a log
    /// nobody reads while `SPC m s` looked like a key that does nothing. A
    /// variant added to the protocol does NOT make the lift fail to compile —
    /// nothing here matches on `Effect` exhaustively — so the only thing
    /// standing between `tune` and that same silence is this test.
    #[test]
    fn tuning_is_an_effect_the_host_can_actually_build() {
        use ronoctua_protocol::{Effect, Knob, Setting, StepSwitch};
        use wasmtime::component::Val;

        let tune = |knob: Val| {
            Val::List(vec![Val::Variant(
                "tune".to_owned(),
                Some(Box::new(Val::Record(vec![
                    ("task".into(), Val::String("update".into())),
                    ("knob".into(), knob),
                ]))),
            )])
        };
        let of = |knob: Knob| {
            vec![Effect::Tune(Setting {
                task: "update".to_owned(),
                knob,
            })]
        };

        // EVERY CASE OF THE VARIANT, because a `knob` is lifted by matching the
        // case name AND the payload's type together: one case reached by an arm
        // that reads the wrong type comes back as nothing, which on screen is a
        // key that does not answer.
        assert_eq!(
            effects_from(&tune(Val::Variant(
                "interval-days".to_owned(),
                Some(Box::new(Val::U16(7)))
            ))),
            of(Knob::IntervalDays(7))
        );
        assert_eq!(
            effects_from(&tune(Val::Variant(
                "part-of-day".to_owned(),
                Some(Box::new(Val::Enum("night".to_owned())))
            ))),
            of(Knob::PartOfDay(ronoctua_protocol::DayPart::Night))
        );
        assert_eq!(
            effects_from(&tune(Val::Variant(
                "enabled".to_owned(),
                Some(Box::new(Val::Bool(false)))
            ))),
            of(Knob::Enabled(false))
        );
        assert_eq!(
            effects_from(&tune(Val::Variant(
                "step".to_owned(),
                Some(Box::new(Val::Record(vec![
                    ("id".into(), Val::String("collect-garbage".into())),
                    ("on".into(), Val::Bool(false)),
                ])))
            ))),
            of(Knob::Step(StepSwitch {
                id: "collect-garbage".to_owned(),
                on: false,
            }))
        );
        assert_eq!(
            effects_from(&tune(Val::Variant("reset-steps".to_owned(), None))),
            of(Knob::ResetSteps)
        );

        // AND THE GAPS ARE REFUSALS, not defaults. A `part-of-day` this build
        // cannot read would otherwise fall back to `any` and move a rebuild out
        // of the small hours into the middle of somebody's afternoon.
        assert!(
            effects_from(&tune(Val::Variant(
                "part-of-day".to_owned(),
                Some(Box::new(Val::Enum("siesta".to_owned())))
            )))
            .is_empty()
        );
        assert!(
            effects_from(&tune(Val::Variant(
                "knob-from-the-future".to_owned(),
                Some(Box::new(Val::U16(1)))
            )))
            .is_empty()
        );
        // The case name is right and the payload is not: refused, and never
        // read as some other case that happens to take that type.
        assert!(
            effects_from(&tune(Val::Variant(
                "interval-days".to_owned(),
                Some(Box::new(Val::Bool(true)))
            )))
            .is_empty()
        );
    }

    /// The brain's requests come back whole, behind their own capability.
    #[test]
    fn a_brain_request_needs_the_brain_capability() {
        use ronoctua_protocol::{Capability, Demand, Effect, Request};
        use wasmtime::component::Val;

        let asked = Val::Record(vec![
            ("what".into(), Val::Enum("rehearse-task".to_owned())),
            ("target".into(), Val::U64(1)),
        ]);
        assert_eq!(
            demand_from(Some(&asked)),
            Some(Demand {
                what: Request::RehearseTask,
                target: 1,
            })
        );
        // And it is gated behind `brain` and not `system` — the memory panel
        // asked for neither, and this one ends with the machine rebuilding its
        // own operating system.
        assert_eq!(
            Effect::Ask(Demand {
                what: Request::RunTask,
                target: 1,
            })
            .requires(),
            Some(Capability::Brain)
        );
    }

    /// A MODULE'S PAGE has the WIT's shape, cell by cell: the variant's case
    /// crosses by name, and a moment and a size carry their number and text.
    #[test]
    fn a_module_screen_has_the_wit_shape() {
        use ronoctua_protocol::{
            BrainTone, Measured, ModuleRow, ModuleScreen, ModuleSection, ScreenCell, Timed,
        };
        use wasmtime::component::Val;
        let screen = ModuleScreen {
            module: "fixture".to_owned(),
            generation: 9,
            sections: vec![ModuleSection {
                heading: "fixture-heading".to_owned(),
                rows: vec![ModuleRow {
                    id: "one".to_owned(),
                    label: ScreenCell::Key("fixture-row".to_owned()),
                    value: ScreenCell::Bytes(Measured {
                        bytes: 2048,
                        text: "2 KB".to_owned(),
                    }),
                    note: ScreenCell::Moment(Timed {
                        at: 5,
                        text: "1970-01-01 00:00".to_owned(),
                    }),
                    tone: BrainTone::Watching,
                    act: "forget".to_owned(),
                    also: "keep".to_owned(),
                }],
            }],
            refused: "fixture refusal".to_owned(),
        };
        let v = val_of_screen(&screen);
        assert_eq!(shape(&v), ["module", "generation", "sections", "refused"]);
        assert_eq!(
            field(&v, "refused"),
            Some(&Val::String("fixture refusal".to_owned()))
        );
        let Val::List(sections) = field(&v, "sections").expect("sections") else {
            panic!("sections is not a list")
        };
        assert_eq!(shape(&sections[0]), ["heading", "rows"]);
        let Val::List(rows) = field(&sections[0], "rows").expect("rows") else {
            panic!("rows is not a list")
        };
        assert_eq!(
            shape(&rows[0]),
            ["id", "label", "value", "note", "tone", "act", "also"]
        );
        assert_eq!(
            field(&rows[0], "tone"),
            Some(&Val::Enum("watching".to_owned()))
        );
        let Some(Val::Variant(case, Some(payload))) = field(&rows[0], "value") else {
            panic!("the value is not a variant with a payload")
        };
        assert_eq!(case, "bytes");
        assert_eq!(shape(payload), ["bytes", "text"]);
        assert_eq!(
            field(&rows[0], "label"),
            Some(&Val::Variant(
                "key".to_owned(),
                Some(Box::new(Val::String("fixture-row".to_owned())))
            ))
        );
        // AND IT CROSSES AS ITS OWN INPUT, under the WIT's case name.
        let Val::Variant(case, _) =
            val_of_input(&ronoctua_protocol::Input::ModuleScreen(Box::new(screen)))
        else {
            panic!("the input is not a variant")
        };
        assert_eq!(case, "module-screen");
    }

    /// EVERY VERB OF THE MODULES comes back from the guest, behind the brain's
    /// capability — a verb with no arm here is a key that does nothing, with
    /// a `warn` in a log nobody reads.
    #[test]
    fn the_modules_verbs_come_back_whole() {
        use ronoctua_protocol::{Capability, Request};
        use wasmtime::component::Val;
        for (name, what) in [
            ("watch-screen", Request::WatchScreen),
            ("act-row", Request::ActRow),
            ("act-row-also", Request::ActRowAlso),
            ("act-page", Request::ActPage),
            ("dismiss-notice", Request::DismissNotice),
            ("snooze-notice", Request::SnoozeNotice),
            ("act-notice", Request::ActNotice),
        ] {
            let asked = Val::Record(vec![
                ("what".into(), Val::Enum(name.to_owned())),
                ("target".into(), Val::U64(3)),
            ]);
            let demand = demand_from(Some(&asked)).expect(name);
            assert_eq!(demand.what, what, "{name}");
            assert_eq!(demand.target, 3, "{name}");
            assert_eq!(what.requires(), Capability::Brain, "{name}");
        }
    }

    #[test]
    fn an_invalid_cwasm_fails_with_a_useful_message() {
        let r = Runtime::fresh().unwrap();
        // `unwrap_err` would require `Debug` on the `Ok`, and `Component` does not have it.
        let Err(e) = r.load(b"I am not a cwasm") else {
            panic!("wasmtime accepted bytes that are not a .cwasm")
        };
        assert!(
            format!("{e}").contains("wasmtime"),
            "the message does not help: {e}"
        );
    }
}
