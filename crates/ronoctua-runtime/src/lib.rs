//! Native buffer runtime — sections 5.2, 7.8 and 7.9.
//!
//! # The 5 ms budget is not a goal, it is a configuration
//!
//! Instantiating a component with 1.5 MB of data (the `glyphdb`) copies those
//! 1.5 MB per instance — unless the memory image is mapped
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
// In wasmtime-wasi 38 the types are at the crate's ROOT, and `WasiView::ctx`
// returns `ctx` and `table` together in a `WasiCtxView` — the separate `IoView`
// from earlier versions no longer exists.
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

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
    /// by ear. Do not use it in production: it is the configuration that blows section
    /// 5.2's budget with the `glyphdb`'s data.
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

        let engine = wasmtime::Engine::new(&c).context("configuracao recusada pelo wasmtime")?;

        // The linker is built ONCE and reused. Building it per instance would put
        // the resolution of hundreds of WASI imports inside the 5 ms
        // budget — and it is work that does not change between instances.
        let mut linker = wasmtime::component::Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .context("nao consegui ligar o WASI ao linker")?;

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
            .context("nao consegui pre-compilar o componente")
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
            .context("o .cwasm nao casa com esta versao do wasmtime")
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
            .context("nao consegui instanciar o componente")?;
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
            .context("`init` nao e funcao")?;
        let mut out = [Val::Bool(false)];
        func.call(&mut self.store, &[], &mut out)
            .context("o `init` do componente falhou")?;
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
            .context("`render` nao e funcao")?;
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
            .context("o `render` do componente falhou")?;
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
            .context("`evento` nao e funcao")?;
        let mut out = [Val::Bool(false)];
        func.call(&mut self.store, &[val_of_input(e)], &mut out)
            .context("o `evento` do componente falhou")?;
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
            .context("nao consegui instanciar o componente")?;

        let vision = instance
            .get_export_index(&mut store, None, "ronoctua:buffer/view@0.1.0")
            .context("o componente nao exporta `ronoctua:buffer/view@0.1.0`")?;
        let export = |store: &mut wasmtime::Store<State>, name: &str| {
            instance
                .get_export_index(store, Some(&vision), name)
                .with_context(|| format!("a interface `visao` nao exporta `{name}`"))
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
        _componente: &wasmtime::component::Component,
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

    let (caso, carga) = match e {
        Input::Command(id) => ("command", Some(Val::String(id.clone()))),
        // `caractere`, and not `char`: see the comment in the `.wit`.
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
        Input::Click { column, line } => (
            "click",
            Some(Val::Record(vec![
                ("column".into(), Val::U16(*column)),
                ("line".into(), Val::U16(*line)),
            ])),
        ),
    };
    Val::Variant(caso.to_owned(), carga.map(Box::new))
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
            let Val::Variant(caso, carga) = x else {
                return None;
            };
            let text = || match carga.as_deref() {
                Some(Val::String(s)) => Some(s.clone()),
                _ => None,
            };
            match caso.as_str() {
                "redraw" => Some(Effect::Redraw),
                "copy" => text().map(Effect::Copy),
                "command" => text().map(Effect::Command),
                "close" => Some(Effect::Close),
                otherwise => {
                    tracing::warn!(effect = otherwise, "efeito desconhecido");
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
        anyhow::bail!("`init` nao devolveu record");
    };
    let field = |n: &str| fields.iter().find(|(k, _)| k == n).map(|(_, v)| v);
    let texto_de = |v: Option<&Val>| match v {
        Some(Val::String(s)) => s.clone(),
        _ => String::new(),
    };
    let lista_de_str = |v: Option<&Val>| -> Vec<String> {
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
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };

    Ok(prot::Descriptor {
        name: texto_de(field("name")),
        mode: texto_de(field("mode")),
        keymap,
        commands: lista_de_str(field("commands")),
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
        anyhow::bail!("`render` nao devolveu lista");
    };

    let mut list = prot::DisplayList::fresh_one();
    for c in commands {
        let Val::Variant(caso, carga) = c else {
            continue;
        };
        let Some(carga) = carga.as_deref() else {
            continue;
        };
        let Val::Record(fields) = carga else { continue };
        let field = |name: &str| fields.iter().find(|(n, _)| n == name).map(|(_, v)| v);

        let u16_de = |v: Option<&Val>| match v {
            Some(Val::U16(n)) => *n,
            _ => 0,
        };
        // The colour arrives as an ENUM, and no longer as a record with a string.
        //
        // The `Val::Enum` carries the CASE's name in `kebab-case`
        // (`text-primary`), which is the same name the token generator
        // emits on both sides. A case that does not exist never gets here:
        // the guest cannot produce it, because over there it is an enum.
        let cor_de = |v: Option<&Val>| -> prot::Color {
            match v {
                Some(Val::Enum(name)) => prot::Color(name.clone().into()),
                // There is no `unwrap_or` with a pretty default: a colour that
                // did not come is a protocol violation, and the host refuses the
                // command further along instead of papering over it.
                _ => prot::Color(String::new().into()),
            }
        };

        match caso.as_str() {
            "grid" => {
                let g = |n: &str| fields.iter().find(|(k, _)| k == n).map(|(_, v)| v);
                let linhas_txt = match g("lines") {
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
                    lines: linhas_txt,
                    color: cor_de(g("color")),
                });
            }
            "text" => {
                let content = match field("content") {
                    Some(Val::Variant(qual, inside)) => match (qual.as_str(), inside.as_deref()) {
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
                    color: cor_de(field("color")),
                    scale: match field("scale") {
                        Some(Val::Enum(n)) if n == "large" => prot::Scale::Large,
                        Some(Val::Enum(n)) if n == "small" => prot::Scale::Small,
                        Some(Val::Enum(n)) if n == "base" => prot::Scale::Base,
                        Some(Val::Enum(n)) if n == "huge" => prot::Scale::Huge,
                        // An unknown enum falls back to NORMAL: a new buffer with
                        // a new scale shows up legible, and not gigantic.
                        _ => prot::Scale::Normal,
                    },
                    max_width: u16_de(field("max-width")),
                });
            }
            "rule" => list.push(prot::Command::Rule {
                line: u16_de(field("line")),
                color: cor_de(field("color")),
            }),
            "selection" => list.push(prot::Command::Selection {
                line: u16_de(field("line")),
                color: cor_de(field("color")),
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
                    color: cor_de(field("color")),
                });
            }
            otherwise => {
                tracing::warn!(command = otherwise, "comando desconhecido na display list")
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

    #[test]
    fn an_invalid_cwasm_fails_with_a_useful_message() {
        let r = Runtime::fresh().unwrap();
        // `unwrap_err` would require `Debug` on the `Ok`, and `Component` does not have it.
        let Err(e) = r.load(b"nao sou um cwasm") else {
            panic!("o wasmtime aceitou bytes que nao sao um .cwasm")
        };
        assert!(
            format!("{e}").contains("wasmtime"),
            "a mensagem nao ajuda: {e}"
        );
    }
}
