//! Exercises the Lucide icon browser.
//!
//! # Why a binary, and not a `#[test]` in the buffer
//!
//! The component compiles to `wasm32-wasip2` and lives in a workspace of its own
//! (invariant 12): a `cargo test` of it would test the logic **outside** the
//! component — precisely what proves nothing. Here it is loaded the way the host
//! loads it, driven by `event` and read by `render`.
//!
//! Usage: `ronoctua-test-lucide <component.wasm> <i18n-dir>`

use anyhow::{Context as _, Result, bail};
use ronoctua_protocol::{Command, Effect, Input, Text};

fn heights() -> ronoctua_protocol::Heights {
    ronoctua_protocol::Heights::from_sizes(15.0, 18.0, 20.0, 40.0, 116.0, 1.45)
}

const ROWS: u16 = 60;
const COLUMNS: u16 = 100;

/// How many icons the database has. **Exact**, and not a floor: a floor would catch a
/// truncated database, the exact number also catches the generator emitting what it should not.
const TOTAL: usize = 1668;

/// How many categories Lucide 0.563.0 has. Plus one: the "all" option.
const CATEGORIES_OF: usize = 43;

/// THE RANGE THE ICONS ARE DRAWN IN.
///
/// Written here on purpose, and not read from the database: it is what ties the two
/// ends of this buffer's most expensive defect together. Lucide publishes the icons at
/// `U+E038..U+E6BB`, and **634 of those codepoints already belong to Nerd Fonts**
/// inside the Iosevka — which is the primary face, and therefore wins the
/// fallback chain. A generator that went back to the source range would not produce tofu:
/// it would produce the WRONG icon, drawn with confidence. Only a criterion about the
/// range catches that.
const RANGE: std::ops::RangeInclusive<u32> = 0x000F_2000..=0x000F_2683;

/// How many actionable rows the sheet has: 5 adjustments + 19 copy forms.
const ACTIONS: usize = 24;

/// How many of those are ADJUSTMENTS.
const ADJUSTMENTS: usize = 5;

/// How many affordance marks the sheet draws in total.
///
/// Four rows move with the arrows and two accept typing: `4 + 2`. The
/// number is here, and not counted from the buffer's code, because that is precisely
/// what the criterion has to assert from outside.
const ADJUSTMENTS_WITH_MARK: usize = 6;

/// What is seen in the grid.
struct Vision {
    filtered: usize,
    total: usize,
    /// What is written in the search FIELD. `None` when it shows the
    /// invitation (an empty search).
    field: Option<String>,
    /// What was drawn in each grid cell, in reading order.
    drawings: Vec<String>,
    /// The focused icon's name, read from the footer.
    focus: Option<String>,
}

fn read(list: &ronoctua_protocol::DisplayList) -> Result<Vision> {
    let mut counter: Option<(u16, String)> = None;
    let mut field: Option<String> = None;
    let mut cells: std::collections::BTreeMap<(u16, u16), String> =
        std::collections::BTreeMap::new();

    for c in &list.commands {
        let Command::Text {
            line, column, text, ..
        } = c
        else {
            continue;
        };
        let Text::Data(s) = text else { continue };
        if *line == 0 {
            // ROW 0 HAS TWO DATA RUNS: what is in the field and the
            // counter, flush right. What decides is the COLUMN.
            match &counter {
                Some((col, _)) if *col >= *column => field = Some(s.clone()),
                Some((_, anterior)) => {
                    field = Some(anterior.clone());
                    counter = Some((*column, s.clone()));
                }
                None => counter = Some((*column, s.clone())),
            }
        } else {
            cells.insert((*line, *column), s.clone());
        }
    }

    // THE LAST ROW IS THE FOOTER: the focused icon, its name and the tags. The
    // NAME is what identifies the icon — the drawing is a PUA character, which
    // compares against nothing readable.
    let mut focus = None;
    if let Some(last_one) = cells.keys().next_back().map(|(l, _)| *l) {
        let on_line: Vec<(u16, String)> = cells
            .range((last_one, 0)..=(last_one, u16::MAX))
            .map(|((_, c), s)| (*c, s.clone()))
            .collect();
        focus = on_line.get(1).map(|(_, s)| s.clone());
        for (column, _) in &on_line {
            cells.remove(&(last_one, *column));
        }
    }

    let (_, cab) = counter.context("o `render` nao emitiu contador na linha 0")?;
    let (n, m) = cab
        .split_once('/')
        .with_context(|| format!("contador fora do formato `N / M`: {cab}"))?;

    Ok(Vision {
        filtered: n.trim().parse().context("contagem de resultados")?,
        total: m.trim().parse().context("total da base")?,
        field,
        drawings: cells.into_values().collect(),
        focus,
    })
}

fn screen(inst: &mut ronoctua_runtime::Instance) -> Result<Vision> {
    read(&inst.render(COLUMNS, ROWS, heights())?)
}

/// Types a query from scratch.
fn query(inst: &mut ronoctua_runtime::Instance, q: &str) -> Result<Vision> {
    inst.event(&Input::Command("lucide-filter-clear".to_owned()))?;
    for c in q.chars() {
        inst.event(&Input::Char(c))?;
    }
    screen(inst)
}

/// The text keys emitted.
fn keys_of(list: &ronoctua_protocol::DisplayList) -> Vec<String> {
    list.commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                text: Text::Key(k), ..
            } => Some(k.clone()),
            _ => None,
        })
        .collect()
}

fn ftl(path: &str) -> Result<std::collections::HashMap<String, String>> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("nao consegui ler {path}"))?;
    Ok(raw
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect())
}

/// The value of a sheet row, by label.
fn sheet_value(list: &ronoctua_protocol::DisplayList, label: &str) -> Option<String> {
    let line = list.commands.iter().find_map(|c| match c {
        Command::Text {
            line,
            text: Text::Key(k),
            ..
        } if k == label => Some(*line),
        _ => None,
    })?;
    list.commands.iter().find_map(|c| match c {
        Command::Text {
            line: l,
            text: Text::Data(s),
            ..
        } if *l == line => Some(s.clone()),
        _ => None,
    })
}

/// The icons drawn on row `line`, by token name.
///
/// None of the three buffer drivers read `Text::Icon` — they filtered only
/// `Data` and `Key`. Without this there is no way to assert that the affordance mark
/// is where it should be, which is precisely what it promises.
fn icons_on_row(list: &ronoctua_protocol::DisplayList, line: u16) -> Vec<String> {
    list.commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                line: l,
                text: Text::Icon(name),
                ..
            } if *l == line => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// On which screen row the label `label` was drawn.
fn label_row(list: &ronoctua_protocol::DisplayList, label: &str) -> Option<u16> {
    list.commands.iter().find_map(|c| match c {
        Command::Text {
            line,
            text: Text::Key(k),
            ..
        } if k == label => Some(*line),
        _ => None,
    })
}

/// The SVG the sheet's preview is drawing.
fn preview(list: &ronoctua_protocol::DisplayList) -> Option<String> {
    list.commands.iter().find_map(|c| match c {
        Command::Svg { svg, .. } => Some(svg.clone()),
        _ => None,
    })
}

/// The text an `Enter` asked to be copied.
fn copied(effects: &[Effect]) -> Option<String> {
    effects.iter().find_map(|e| match e {
        Effect::Copy(s) => Some(s.clone()),
        _ => None,
    })
}

/// Puts the cursor on the icon `name` and opens its sheet.
fn open(inst: &mut ronoctua_runtime::Instance, name: &str) -> Result<()> {
    let v = query(inst, name)?;
    if v.focus.as_deref() != Some(name) {
        bail!(
            "procurar `{name}` nao poe `{name}` em foco (foco: {:?})",
            v.focus
        );
    }
    inst.event(&Input::Confirm)?;
    Ok(())
}

/// Moves `n` rows down the sheet.
fn go_down(inst: &mut ronoctua_runtime::Instance, n: usize) -> Result<()> {
    for _ in 0..n {
        inst.event(&Input::Down)?;
    }
    Ok(())
}

/// Copies the form on action row `n` (0 = the first adjustment).
fn copy_action(inst: &mut ronoctua_runtime::Instance, name: &str, n: usize) -> Result<String> {
    open(inst, name)?;
    go_down(inst, n)?;
    let effects = inst.event(&Input::Confirm)?;
    let t = copied(&effects)
        .with_context(|| format!("a linha {n} da ficha de `{name}` nao copiou nada"))?;
    inst.event(&Input::Cancel)?;
    Ok(t)
}

/// Standard base64, decoded. Only to check what the buffer produced.
fn from_base64(s: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits = 0u32;
    let mut n = 0u8;
    let mut out = Vec::new();
    for c in s.bytes() {
        if c == b'=' {
            break;
        }
        let v = ALPHABET.iter().position(|a| *a == c)?;
        bits = (bits << 6) | u32::try_from(v).ok()?;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push(u8::try_from((bits >> n) & 0xFF).ok()?);
        }
    }
    Some(out)
}

#[expect(
    clippy::too_many_lines,
    reason = "e uma lista de criterios, um por bloco"
)]
fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("uso: ronoctua-test-lucide <componente.wasm> <dir-i18n>")?;
    let i18n = std::env::args()
        .nth(2)
        .context("uso: ronoctua-test-lucide <componente.wasm> <dir-i18n>")?;
    let wasm = std::fs::read(&path).with_context(|| format!("nao consegui ler {path}"))?;

    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    // --- the descriptor ----------------------------------------------------
    let d = inst.init()?;
    if d.mode != "lucide" {
        bail!("o modo do buffer e `{}` e devia ser `lucide`", d.mode);
    }
    // EVERY BINDING UNDER `m`: the host refuses at load whoever leaves that, and a keymap
    // the host refuses becomes a buffer with no keys at all — with no error on screen.
    for l in &d.keymap {
        if !l.keys.starts_with("m ") {
            bail!("a ligacao `{}` sai de `SPC m`", l.keys);
        }
    }
    // THE 19 COPY FORMS HAVE DISTINCT KEYS. Two identical ones give no error:
    // one of them simply never fires.
    let mut keys: Vec<&str> = d.keymap.iter().map(|l| l.keys.as_str()).collect();
    keys.sort_unstable();
    let before = keys.len();
    keys.dedup();
    if keys.len() != before {
        bail!("ha teclas repetidas no keymap do buffer");
    }
    println!(
        "OK: descritor com {} ligacoes, todas sob `SPC m`",
        d.keymap.len()
    );

    // --- the database, and the OPENING POSTURE -----------------------------
    let v = screen(&mut inst)?;
    if v.total != TOTAL {
        bail!("a base tem {} icones e devia ter {TOTAL}", v.total);
    }
    // IT OPENS WHOLE: with no filter written in the field, and with all 1,668 in view.
    if v.field.is_some() {
        bail!(
            "o buffer abriu com `{:?}` no campo, e devia abrir vazio",
            v.field
        );
    }
    if v.filtered != TOTAL {
        bail!("o buffer abriu mostrando {} de {TOTAL}", v.filtered);
    }
    if v.drawings.is_empty() {
        bail!("a grade abriu vazia");
    }
    println!(
        "OK: abre inteiro, {TOTAL} icones, {} desenhados",
        v.drawings.len()
    );

    // --- THE DRAWING RANGE -------------------------------------------------
    //
    // The criterion that exists because of the collision with Nerd Fonts. See `RANGE`.
    for (n, s) in v.drawings.iter().enumerate() {
        let mut cs = s.chars();
        let (Some(c), None) = (cs.next(), cs.next()) else {
            bail!("a celula {n} desenhou {s:?}, que nao e um caractere so");
        };
        if !RANGE.contains(&(c as u32)) {
            bail!(
                "a celula {n} desenhou U+{:04X}, fora da faixa remapeada \
                 U+{:04X}..U+{:04X}.\n\
                 A faixa de origem da Lucide colide com 634 codepoints do Nerd \
                 Fonts na Iosevka, e a face primaria vence o fallback: estes \
                 icones sairiam desenhados como OUTRO simbolo.",
                c as u32,
                RANGE.start(),
                RANGE.end()
            );
        }
    }
    println!(
        "OK: os {} desenhos estao na faixa remapeada",
        v.drawings.len()
    );

    // --- the search --------------------------------------------------------
    let v = query(&mut inst, "house")?;
    if v.focus.as_deref() != Some("house") {
        bail!("`house` nao poe `house` em primeiro (foco: {:?})", v.focus);
    }
    // THE ALIAS FINDS IT. `home` is a deprecated alias of `house`, and whoever types
    // `home` is looking for the little house — not a deprecation warning.
    let v = query(&mut inst, "home")?;
    if v.filtered == 0 {
        bail!("`home`, que e apelido de `house`, nao acha nada");
    }
    // IN PORTUGUESE. Without this the buffer is bilingual only in its labels.
    let v = query(&mut inst, "casa")?;
    if v.filtered == 0 {
        bail!("`casa` nao acha nada — as tags em portugues nao chegaram");
    }
    // THE PHRASE. Two words require the icon to have BOTH: matching by a
    // single term returned zero here, and it was the defect that cost the symbol
    // browser.
    let v = query(&mut inst, "arrow up")?;
    if v.filtered == 0 {
        bail!("`arrow up` devolve zero — o casamento voltou a ser de termo unico");
    }
    if v.focus
        .as_deref()
        .is_none_or(|n| !n.contains("arrow") || !n.contains("up"))
    {
        bail!("`arrow up` poe `{:?}` em primeiro", v.focus);
    }
    println!("OK: busca por nome, apelido, portugues e frase");

    // --- the filter, and relevance WITHIN it -------------------------------
    let v = query(&mut inst, "categoria:arrows")?;
    if v.filtered == 0 || v.filtered >= TOTAL {
        bail!("`categoria:arrows` devolveu {} de {TOTAL}", v.filtered);
    }
    let so_categoria = v.filtered;
    // A FREE TERM COEXISTS WITH THE FILTER. The enumeration filter eats ONE
    // word: with the rest, typing with a filter active was impossible.
    let v = query(&mut inst, "categoria:arrows up")?;
    if v.filtered == 0 {
        bail!("`categoria:arrows up` devolveu zero — o filtro comeu o termo livre");
    }
    if v.filtered >= so_categoria {
        bail!("o termo livre nao restringiu nada dentro do filtro");
    }
    // THE SCORE SURVIVES THE PREFIX: within the category, the order changes with the
    // term. Without that the list comes out in alphabetical order of the English name.
    if v.focus.as_deref().is_none_or(|n| !n.contains("up")) {
        bail!(
            "com filtro ativo, o primeiro resultado de `up` e {:?}",
            v.focus
        );
    }
    println!("OK: filtro por categoria, com termo livre e relevancia");

    // --- the panel ---------------------------------------------------------
    inst.event(&Input::Command("lucide-filter-clear".to_owned()))?;
    inst.event(&Input::Toggle)?;
    let list = inst.render(COLUMNS, ROWS, heights())?;
    let options: Vec<String> = keys_of(&list)
        .into_iter()
        .filter(|k| k.starts_with("lucide-category-"))
        .collect();
    if options.len() != CATEGORIES_OF + 1 {
        bail!(
            "o painel mostra {} opcoes e devia mostrar {} (as {CATEGORIES_OF} categorias e `todas`)",
            options.len(),
            CATEGORIES_OF + 1
        );
    }
    // `Tab` CLOSES. A key that only opens is half a key.
    inst.event(&Input::Toggle)?;
    let closed = keys_of(&inst.render(COLUMNS, ROWS, heights())?)
        .into_iter()
        .filter(|k| k.starts_with("lucide-category-"))
        .count();
    if closed != 0 {
        bail!("o `Tab` nao fechou o painel ({closed} opcoes ainda na tela)");
    }
    println!(
        "OK: painel com {} opcoes, e o Tab abre e fecha",
        options.len()
    );

    // --- THE LABELS FIND THEIR OWN CATEGORY --------------------------------
    //
    // The label is read on screen AND typed into the search. They are two representations of the
    // same fact — the panel says "Setas", the database says `arrows` — and without this
    // criterion they diverge silently: the panel stays pretty and what
    // it promises stops being findable.
    for (file, language) in [("pt-BR.ftl", "pt-BR"), ("en-GB.ftl", "en-GB")] {
        let table = ftl(&format!("{i18n}/{file}"))?;
        for key in &options {
            let label = table
                .get(key)
                .with_context(|| format!("{language} nao tem a chave `{key}`"))?;
            if key.ends_with("-all") {
                continue;
            }
            let category = key.trim_start_matches("lucide-category-");
            let pelo_rotulo = query(&mut inst, &label.to_lowercase())?;
            let pela_categoria = query(&mut inst, &format!("categoria:{category}"))?;
            if pela_categoria.filtered == 0 {
                bail!("a categoria `{category}` esta vazia");
            }
            if pelo_rotulo.filtered < pela_categoria.filtered {
                bail!(
                    "em {language}, o rotulo `{label}` acha {} icones e a categoria \
                     `{category}` tem {}. O que o painel mostra tem de ser \
                     achavel pelo que ele mostra.",
                    pelo_rotulo.filtered,
                    pela_categoria.filtered
                );
            }
        }
    }
    println!("OK: os {CATEGORIES_OF} rotulos acham a categoria inteira, nos dois idiomas");

    // --- the sheet ---------------------------------------------------------
    open(&mut inst, "house")?;
    let f = inst.render(COLUMNS, ROWS, heights())?;
    if sheet_value(&f, "lucide-sheet-name").as_deref() != Some("house") {
        bail!("a ficha nao mostra o nome");
    }
    if sheet_value(&f, "lucide-sheet-component").as_deref() != Some("House") {
        bail!("a ficha nao mostra o nome de componente");
    }
    // THE SHEET'S CODEPOINT IS LUCIDE'S, and not the screen's.
    //
    // They are two different facts from two different fonts, and this is the only
    // place where both appear together. `house` lives at `U+E0F5` in the published
    // font; in our remapped face it is in the `U+F2xxx` range.
    let cp = sheet_value(&f, "lucide-sheet-codepoint").unwrap_or_default();
    if cp != "U+E0F5" {
        bail!("a ficha diz que `house` e {cp}, e a Lucide o publica em U+E0F5");
    }
    inst.event(&Input::Cancel)?;
    println!("OK: ficha com nome, componente e o codepoint DA LUCIDE");

    // --- the copy forms ----------------------------------------------------
    //
    // `house`'s two `path` lines are written out here: it is what ties the
    // COPY to the DRAWING. Without them, the formatter could emit a well-formed,
    // empty SVG, and every syntax criterion would pass.
    const RIDGE: &str = "M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8";
    let svg = copy_action(&mut inst, "house", 5)?;
    for pedaco in [
        "<svg ",
        r#"viewBox="0 0 24 24""#,
        r#"width="24""#,
        r#"stroke-width="2""#,
        r#"stroke-linecap="round""#,
        r#"class="lucide lucide-house""#,
        RIDGE,
        "</svg>",
    ] {
        if !svg.contains(pedaco) {
            bail!("o SVG copiado nao contem `{pedaco}`:\n{svg}");
        }
    }
    // THE JSX IS THE SAME DRAWING with the attributes in camelCase.
    let jsx = copy_action(&mut inst, "house", 6)?;
    if !jsx.contains("strokeWidth=") || !jsx.contains("className=") || !jsx.contains(RIDGE) {
        bail!("o JSX copiado nao esta em camelCase ou perdeu o desenho:\n{jsx}");
    }
    if jsx.contains("stroke-width=") {
        bail!("o JSX copiado ainda tem atributo hifenizado");
    }
    // THE COPIED CHARACTER IS LUCIDE'S. Copying the screen's would send to the
    // user's document a plane-15 codepoint that only this face draws.
    let caractere = copy_action(&mut inst, "house", 5 + 12)?;
    if caractere != "\u{E0F5}" {
        bail!(
            "o caractere copiado e U+{:04X}, e devia ser o da Lucide (U+E0F5)",
            caractere.chars().next().map_or(0, |c| c as u32)
        );
    }
    // THE BASE64 HAS TO DECODE BACK TO THE SVG. A wrong encoder produces a plausible
    // string that only fails in the browser of whoever pasted it.
    let b64 = copy_action(&mut inst, "house", 5 + 16)?;
    let data_bytes = b64
        .strip_prefix("data:image/svg+xml;base64,")
        .context("o base64 nao veio como data URI")?;
    let raw_bytes = from_base64(data_bytes).context("o base64 nao decodifica")?;
    let text = String::from_utf8(raw_bytes).context("o base64 nao decodifica para texto")?;
    if !text.contains(RIDGE) || !text.starts_with("<svg ") {
        bail!("o base64 nao decodifica para o SVG:\n{text}");
    }
    // THE CHARACTER ROW SHOWS ONE CODEPOINT AND COPIES ANOTHER, on purpose.
    //
    // Drawing Lucide's here would ask our face chain for `U+E34D`, and there
    // that codepoint belongs to **Nerd Fonts**: `air-vent` showed up as a WEATHER
    // icon — the collision that forced the remapping, sprouting on the very screen that
    // explains it. Both codepoints draw the same icon, each in the font it
    // belongs to; ours is the only true preview.
    open(&mut inst, "house")?;
    let f = inst.render(COLUMNS, ROWS, heights())?;
    let shown = sheet_value(&f, "cmd-lucide-copy-character")
        .context("a ficha nao tem a linha do caractere")?;
    let cp = shown.chars().next().map_or(0, |c| c as u32);
    if !RANGE.contains(&cp) {
        bail!(
            "a ficha mostra U+{cp:04X} na linha do caractere, fora da faixa \
             remapeada — na nossa cadeia de faces esse codepoint desenha um \
             glifo do Nerd Fonts, e nao o icone."
        );
    }
    inst.event(&Input::Cancel)?;
    println!("OK: SVG, JSX, caractere e base64 — o desenho chega inteiro nos quatro");
    println!("OK: a ficha PREVE com o codepoint desta face e COPIA o da Lucide");

    // --- THE CUSTOMISATION CHANGES THE COPY --------------------------------
    //
    // It is the only place where it is observable: the font has the stroke baked
    // into the glyph, so the screen does not change. If the adjustments do not reach the
    // copied text, they reach nothing.
    open(&mut inst, "house")?;
    inst.event(&Input::Right)?; // size 24 -> 25
    go_down(&mut inst, 5)?;
    let svg = copied(&inst.event(&Input::Confirm)?).context("nao copiou")?;
    if !svg.contains(r#"width="25""#) {
        bail!("mudar o tamanho nao mudou o SVG:\n{svg}");
    }
    // THE ABSOLUTE STROKE IS A CALCULATION, and not a flag: `stroke * 24 /
    // size`. With size 48 and stroke 2, the `stroke-width` has to become 1.
    inst.event(&Input::Cancel)?;
    open(&mut inst, "house")?;
    for c in "48".chars() {
        inst.event(&Input::Char(c))?;
    }
    inst.event(&Input::Down)?;
    inst.event(&Input::Down)?;
    inst.event(&Input::Right)?; // turns the absolute stroke on
    go_down(&mut inst, 3)?;
    let svg = copied(&inst.event(&Input::Confirm)?).context("nao copiou")?;
    // THE FIRST DIGIT REPLACES: reaching the row with `24` and typing `48` gives
    // `48`, and not `2448` clipped to `512` — a number nobody asked for coming
    // from an obvious gesture.
    if !svg.contains(r#"width="48""#) {
        bail!("digitar `48` sobre `24` nao deu 48:\n{svg}");
    }
    if !svg.contains(r#"stroke-width="1""#) {
        bail!("o traco absoluto nao virou a conta `2 * 24 / 48 = 1`:\n{svg}");
    }
    // `Tab` RESTORES EVERYTHING. Five rows to undo one by one is the kind of
    // work that stops anyone from experimenting.
    inst.event(&Input::Toggle)?;
    go_down(&mut inst, 0)?;
    let svg = copied(&inst.event(&Input::Confirm)?).context("nao copiou")?;
    if !svg.contains(r#"width="24""#) || !svg.contains(r#"stroke-width="2""#) {
        bail!("o `Tab` nao restaurou os padroes da Lucide:\n{svg}");
    }
    inst.event(&Input::Cancel)?;
    println!("OK: tamanho, traco absoluto e restauracao chegam a copia");

    // --- THE SHEET SCROLLS -------------------------------------------------
    //
    // There are 24 actionable rows against the symbol browser's 9. In a
    // short window they do NOT fit, and without scrolling the last copy forms
    // would be unreachable — the key moves, the highlight leaves the screen, and nothing
    // gives it away.
    open(&mut inst, "house")?;
    let low = 20;
    for n in 0..ACTIONS {
        let list = inst.render(COLUMNS, low, heights())?;
        let realcadas = list
            .commands
            .iter()
            .filter(|c| {
                matches!(c, Command::Text { color, line, .. }
                    if color.as_str() == "state-accent" && *line < low)
            })
            .count();
        if realcadas == 0 {
            bail!("na linha {n} de {ACTIONS}, a selecao saiu da tela de {low} linhas");
        }
        if n + 1 < ACTIONS {
            inst.event(&Input::Down)?;
        }
    }
    // AND THE LAST ROW COPIES. Getting there and seeing is not the same as acting.
    let last =
        copied(&inst.event(&Input::Confirm)?).context("a ultima forma de copia nao copiou nada")?;
    if !last.contains("lucide.svg#house") {
        bail!("a ultima forma de copia nao e o sprite:\n{last}");
    }
    inst.event(&Input::Cancel)?;
    println!("OK: as {ACTIONS} linhas da ficha sao alcancaveis numa janela de {low} linhas");

    // --- the mark says what the key does -----------------------------------
    //
    // THE DEFECT: the 24 rows all looked the same, and the side arrow did
    // THREE things depending on the row — it changed the value, swapped the icon, or (on
    // the colour one) nothing, silently. A mark drawn from a second
    // list would lie on the first new adjustment; this criterion requires it to
    // match the table, row by row.
    //
    // `adjust` and `edit` are the TOKENS' names; the codepoint is the host's.
    const EXPECTED: [(&str, bool, bool); ADJUSTMENTS] = [
        ("lucide-adjust-size", true, true),
        ("lucide-adjust-stroke", true, false),
        ("lucide-adjust-absolute", true, false),
        // THE COLOUR DOES NOT MOVE WITH THE ARROWS. It is the row that used to be a dead key.
        ("lucide-adjust-color", false, true),
        ("lucide-adjust-cap", true, false),
    ];
    open(&mut inst, "house")?;
    let sheet = inst.render(COLUMNS, ROWS, heights())?;
    for (label, setas, digita) in EXPECTED {
        let line =
            label_row(&sheet, label).with_context(|| format!("a ficha nao desenha `{label}`"))?;
        let marks = icons_on_row(&sheet, line);
        if marks.contains(&"adjust".to_owned()) != setas {
            bail!(
                "`{label}` mostra {marks:?}, e as setas {} ajustam ali",
                if setas { "DEVEM" } else { "NAO" }
            );
        }
        if marks.contains(&"edit".to_owned()) != digita {
            bail!(
                "`{label}` mostra {marks:?}, e digitar {} muda o valor ali",
                if digita { "DEVE" } else { "NAO" }
            );
        }
    }
    // AND THE 19 COPY FORMS HAVE NO MARK AT ALL: there the arrow swaps the
    // icon, and a mark would promise an adjustment that does not exist.
    for n in ADJUSTMENTS..ACTIONS {
        go_down(&mut inst, if n == ADJUSTMENTS { ADJUSTMENTS } else { 1 })?;
        let l = inst.render(COLUMNS, ROWS, heights())?;
        let em_foco = l
            .commands
            .iter()
            .filter(|c| {
                matches!(c, Command::Text { text: Text::Icon(i), .. }
                    if i == "adjust" || i == "edit")
            })
            .count();
        if em_foco != ADJUSTMENTS_WITH_MARK {
            bail!(
                "com a linha de copia {n} em foco ha {em_foco} marcas, e deviam ser {ADJUSTMENTS_WITH_MARK}"
            );
        }
    }
    inst.event(&Input::Cancel)?;
    println!("OK: a marca de afordancia case com a tabela nas {ACTIONS} linhas");

    // --- the preview IS the artefact ---------------------------------------
    //
    // THE DEFECT: a second SVG builder for the screen, derived from what
    // the copy delivers. Here the two are compared BYTE BY BYTE, before and
    // after touching the stroke — the preview is only worth anything if it is the artefact itself.
    open(&mut inst, "house")?;
    let before = preview(&inst.render(COLUMNS, ROWS, heights())?)
        .context("a ficha nao desenha a previa vetorial")?;
    // IT CLOSES FIRST: `copy_action` opens the sheet again, and with it already open its
    // query would go inside it.
    inst.event(&Input::Cancel)?;
    let copy_of = copy_action(&mut inst, "house", ADJUSTMENTS)?;
    if before != copy_of {
        bail!("a previa e a copia divergem:\n--- previa ---\n{before}\n--- copia ---\n{copy_of}");
    }

    // AND IT FOLLOWS THE ADJUSTMENT. A frozen preview would be the most expensive defect this
    // buffer could have: it would look like it works.
    open(&mut inst, "house")?;
    go_down(&mut inst, 1)?;
    inst.event(&Input::Left)?;
    let after = preview(&inst.render(COLUMNS, ROWS, heights())?)
        .context("a previa sumiu depois de ajustar o traco")?;
    if !after.contains(r#"stroke-width="1.5""#) {
        bail!("a seta na linha do traco nao mudou a previa:\n{after}");
    }
    let copy_of = copied(&{
        go_down(&mut inst, ADJUSTMENTS - 1)?;
        inst.event(&Input::Confirm)?
    })
    .context("a linha do SVG nao copiou")?;
    if after != copy_of {
        bail!("depois do ajuste a previa deixou de ser a copia:\n{after}\n---\n{copy_of}");
    }
    inst.event(&Input::Cancel)?;
    println!("OK: a previa e byte a byte o SVG copiado, e segue o traco");

    println!("\nTODOS OS CRITERIOS DO NAVEGADOR DE ICONES LUCIDE PASSARAM");
    Ok(())
}
