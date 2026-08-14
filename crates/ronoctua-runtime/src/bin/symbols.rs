//! Exercises the Unicode symbol browser.
//!
//! # Why a binary, and not a `#[test]` in the buffer
//!
//! The component compiles to `wasm32-wasip2` and lives in a workspace of its own
//! (invariant 12): a `cargo test` of it would test the logic **outside** the
//! component — precisely what proves nothing. Here it is loaded the way the host
//! loads it, driven by `event` and read by `render`.
//!
//! Usage: `ronoctua-test-symbols <component.wasm> <i18n-dir>`

use anyhow::{Context as _, Result, bail};
use ronoctua_protocol::{Command, Input, Text};

/// How many columns a single-width character occupies at icon scale.
const ICON_COLUMNS: u16 = 2;

/// The scale heights the production host sends. The numbers are the tokens';
/// the ARITHMETIC is the same the host uses.
fn heights() -> ronoctua_protocol::Heights {
    ronoctua_protocol::Heights::from_sizes(15.0, 18.0, 20.0, 40.0, 116.0, 1.45)
}

const ROWS: u16 = 60;
const COLUMNS: u16 = 80;

/// How many characters the database has. **Exact**, and not a floor.
///
/// A floor would catch a truncated database; the exact number also catches the opposite —
/// the generator starting to emit what it should not. And this buffer's database has a
/// trap of its own: `}` is INSIDE it (`U+007D`), and the component's parser
/// splits records on `"},"`. If some string field goes back to
/// carrying an arbitrary character, this number changes.
const TOTAL: usize = 11614;

/// The dotted circle a combining mark is drawn on top of.
///
/// Written here on purpose, like the icon token's name in the other driver:
/// it is what ties the two ends together. Whoever removes `shows` from the generator makes this
/// test fail instead of the mark disappearing from the screen silently.
const CIRCLE: char = '\u{25CC}';

/// The prefix the buffer OPENS with.
const INITIAL_FILTER: &str = "familia:emoji";

/// What is seen on screen.
struct Vision {
    /// How many results the header says there are.
    filtered: usize,
    /// How many characters there are in total.
    total: usize,
    /// What is written in the search FIELD. `None` when it shows the
    /// invitation (an empty search).
    field: Option<String>,
    /// What was DRAWN in each grid cell, in reading order.
    ///
    /// The whole string, and not the first `char`: for a combining mark the
    /// drawing starts at the dotted circle, and reading only the first would make every
    /// criterion about marks compare against `◌`.
    drawings: Vec<String>,
    /// Where each was drawn, in the same order.
    positions: Vec<(u16, u16)>,
    /// Each drawing's declared budget, in the same order.
    budgets: Vec<u16>,
}

/// Reads the display list: the header on row 0, the grid below, the focus on the last.
fn read(list: &ronoctua_protocol::DisplayList) -> Result<Vision> {
    let mut counter: Option<(u16, String)> = None;
    let mut field: Option<String> = None;
    let mut cells: std::collections::BTreeMap<(u16, u16), (String, u16)> =
        std::collections::BTreeMap::new();

    for c in &list.commands {
        let Command::Text {
            line,
            column,
            text,
            max_width,
            ..
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
                Some((col, texto_ant)) => {
                    field = Some(texto_ant.clone());
                    let _ = col;
                    counter = Some((*column, s.clone()));
                }
                None => counter = Some((*column, s.clone())),
            }
        } else {
            cells.insert((*line, *column), (s.clone(), *max_width));
        }
    }

    // THE LAST ROW HAS THREE RUNS: the focused character, its name, and the
    // tags in whatever is left.
    // THERE IS NO `focus` FIELD HERE, and the absence is deliberate: the focused
    // character's identity comes from the SHEET (`unicode-sheet-codepoint`), never from the
    // drawing in the footer. For a combining mark the drawing starts at the
    // dotted circle, and comparing drawings would give false in 302 cases.
    let last_one = cells.keys().next_back().map(|(l, _)| *l);
    if let Some(l) = last_one {
        let on_line: Vec<(u16, String)> = cells
            .range((l, 0)..=(l, u16::MAX))
            .map(|((_, c), (s, _))| (*c, s.clone()))
            .collect();
        for (column, _) in &on_line {
            cells.remove(&(l, *column));
        }
    }

    let mut drawings = Vec::new();
    let mut positions = Vec::new();
    let mut budgets = Vec::new();
    for (k, (s, w)) in &cells {
        drawings.push(s.clone());
        positions.push(*k);
        budgets.push(*w);
    }

    let (_, cab) = counter.context("o `render` nao emitiu contador na linha 0")?;
    let (n, m) = cab
        .split_once('/')
        .with_context(|| format!("contador fora do formato `N / M`: {cab}"))?;

    Ok(Vision {
        filtered: n.trim().parse().context("contagem de resultados")?,
        total: m.trim().parse().context("total da base")?,
        field,
        drawings,
        positions,
        budgets,
    })
}

fn screen(inst: &mut ronoctua_runtime::Instance) -> Result<Vision> {
    read(&inst.render(COLUMNS, ROWS, heights())?)
}

/// Types a query from scratch.
fn query(inst: &mut ronoctua_runtime::Instance, q: &str) -> Result<Vision> {
    inst.event(&Input::Command("unicode-filter-clear".to_owned()))?;
    inst.event(&Input::Paste(q.to_owned()))?;
    screen(inst)
}

/// The text keys emitted, with the budget the buffer reserved.
fn keys(list: &ronoctua_protocol::DisplayList) -> Vec<(String, u16)> {
    list.commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                text: Text::Key(k),
                max_width,
                ..
            } => Some((k.clone(), *max_width)),
            _ => None,
        })
        .collect()
}

/// The panel's options: key, row, column and budget. Empty = closed.
fn panel_of(list: &ronoctua_protocol::DisplayList) -> Vec<(String, u16, u16, u16)> {
    list.commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                line,
                column,
                text: Text::Key(k),
                max_width,
                ..
            } if k.starts_with("unicode-family-")
                || k.starts_with("unicode-group-")
                || k.starts_with("unicode-tone-") =>
            {
                Some((k.clone(), *line, *column, *max_width))
            }
            _ => None,
        })
        .collect()
}

/// A `.ftl` as a `key -> text` table.
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

/// Opens the focused character's sheet and returns its display list.
fn sheet(inst: &mut ronoctua_runtime::Instance) -> Result<ronoctua_protocol::DisplayList> {
    inst.event(&Input::Confirm)?;
    inst.render(COLUMNS, ROWS, heights())
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("uso: ronoctua-test-symbols <componente.wasm> <dir-i18n>")?;
    let i18n = std::env::args()
        .nth(2)
        .context("uso: ronoctua-test-symbols <componente.wasm> <dir-i18n>")?;
    let wasm = std::fs::read(&path).with_context(|| format!("nao consegui ler {path}"))?;

    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    // ---- THE DESCRIPTOR: a command outside the keymap is not reachable ----
    let d = inst.init()?;
    if d.mode != "unicode" {
        bail!("o modo do buffer e `{}`; a modeline mostra isso", d.mode);
    }
    for id in &d.commands {
        if !d.keymap.iter().any(|l| l.command == *id) {
            bail!("`{id}` nao tem tecla: ninguem o alcanca pelo which-key");
        }
    }
    if let Err(e) = d.validate() {
        bail!("o descritor nao passa na guarda da secao 7.9: {e}");
    }
    // EVERY COMMAND HAS A KEY IN BOTH LANGUAGES. Without this which-key shows
    // `[cmd-unicode-copy-utf8]`, which is the usual silent fallback.
    for language in ["pt-BR", "en-US"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for id in &d.commands {
            let key = format!("cmd-{id}");
            if !table.contains_key(&key) {
                bail!("`{key}` nao existe em {language}.ftl");
            }
        }
    }
    println!(
        "descritor {} comandos, todos com tecla e chave",
        d.commands.len()
    );

    // ---- THE DATABASE ARRIVED WHOLE ----
    //
    // THE DEFECT THIS CATCHES: the component has no JSON parser and splits the
    // records on `"},"`. The glyph browser can afford that luxury
    // because its database starts at `U+E000`; **here `}` is in the database**. What
    // saves it is the generator not emitting a string field with an arbitrary character —
    // and if that changes, the number changes.
    let inicial = screen(&mut inst)?;
    if inicial.total != TOTAL {
        bail!(
            "a base tem {} caracteres e deveria ter {TOTAL}. Se caiu, algum campo do\n\
             gerador voltou a carregar `}}`, `\"` ou `,` e o parser partiu no meio.",
            inicial.total
        );
    }

    // ---- IT OPENS FILTERED, AND SAYS SO ----
    //
    // THE DEFECT THIS CATCHES: "opening filtered" done with a hidden
    // flag. The user would see 3.8xx of 7,529 with an empty field and with
    // no clue as to why.
    let field = inicial
        .field
        .clone()
        .context("o campo de busca nao emitiu nada na abertura")?;
    if field != INITIAL_FILTER {
        bail!("o buffer abriu com o campo `{field}`, e nao com `{INITIAL_FILTER}`");
    }
    if inicial.filtered == 0 || inicial.filtered >= TOTAL {
        bail!(
            "a abertura mostrou {} de {TOTAL}: o filtro de simbolos nao restringiu",
            inicial.filtered
        );
    }
    println!(
        "abertura  {} de {TOTAL}, com `{field}` no campo",
        inicial.filtered
    );

    // ---- `Esc` IN TWO LAYERS ----
    //
    // A single layer would drop the symbol filter on the first press, by
    // surprise; none would trap whoever wants to reach the letters.
    // WITH A SPACE: the enumeration filter takes one word and the rest is a
    // free term — which is how one types with a filter active.
    inst.event(&Input::Paste(" rosto".to_owned()))?;
    let with_term = screen(&mut inst)?;
    if with_term.filtered >= inicial.filtered {
        bail!("o termo livre nao restringiu dentro do filtro de simbolos");
    }
    inst.event(&Input::Cancel)?;
    let volta1 = screen(&mut inst)?;
    if volta1.field.as_deref() != Some(INITIAL_FILTER) {
        bail!(
            "o primeiro `Esc` devia limpar o TERMO e manter o filtro; ficou {:?}",
            volta1.field
        );
    }
    inst.event(&Input::Cancel)?;
    let volta2 = screen(&mut inst)?;
    if volta2.filtered != TOTAL {
        bail!(
            "o segundo `Esc` devia mostrar os {TOTAL}; mostrou {}. Sem isto nao ha\n\
             caminho para as letras, e o filtro de abertura vira uma armadilha.",
            volta2.filtered
        );
    }
    if !inst.event(&Input::Cancel)?.is_empty() {
        bail!("o terceiro `Esc`, com a busca ja vazia, pediu redesenho a toa");
    }
    println!("esc       termo -> filtro -> tudo ({TOTAL})");

    // ---- AND THE LETTERS ARE THERE ----
    let grego = query(&mut inst, "GREEK SMALL LETTER ALPHA")?;
    if grego.filtered == 0 {
        bail!("depois do `Esc` duplo, buscar uma letra grega nao acha nada");
    }

    // ---- THE SEARCH MATCHES BY WORD, AND IN A PHRASE ----
    //
    // THE TWO DEFECTS THIS CATCHES, both of which the owner saw on screen:
    //
    //   - `seta para cima` returned ZERO. The term was matched WHOLE against the
    //     name and against each tag, and the tags are loose words: none
    //     contains the phrase.
    //   - searching `seta` put `PESETA SIGN` FIRST, because "the name
    //     contains" scores better than "has the tag" and `pe·seta·` matches as
    //     a substring.
    let frase = query(&mut inst, "seta para cima")?;
    if frase.filtered == 0 {
        bail!(
            "`seta para cima` nao achou nada. Uma frase tem de casar quando TODAS as\n\
             palavras casam — casar a frase inteira contra cada tag nunca acha nada,\n\
             porque as tags sao palavras soltas."
        );
    }
    // AND IT RESTRICTS MORE than a single word: more words, fewer results.
    let so_seta = query(&mut inst, "seta")?;
    if frase.filtered >= so_seta.filtered {
        bail!(
            "`seta para cima` achou {} e `seta` achou {}: a segunda palavra nao\n\
             restringiu — ela esta sendo ignorada.",
            frase.filtered,
            so_seta.filtered
        );
    }
    // THE ORDER: the first result for `seta` has to BE an arrow.
    //
    // The proof is the focused character's name, and not the count: `PESETA SIGN`
    // is still in the results (it is legitimate, `peseta` starts with those letters), but
    // it must not come ahead of whoever has the tag.
    let f = sheet(&mut inst)?;
    let name = sheet_value(&f, "unicode-sheet-codepoint").unwrap_or_default();
    let first = so_seta.drawings.first().cloned().unwrap_or_default();
    inst.event(&Input::Cancel)?;
    if name == "U+20A7" {
        bail!(
            "buscar `seta` pos {first:?} (U+20A7 PESETA SIGN) em primeiro. Substring\n\
             solta no nome esta vencendo a tag: `pe·seta·` nao devia casar `seta`."
        );
    }
    println!(
        "busca     frase {} · `seta` {} · primeiro {first:?}",
        frase.filtered, so_seta.filtered
    );

    // ---- THE EMOJI ----
    //
    // The buffer OPENS on them, so the first screen has already shown them. Here we assert
    // what the rest of the database does not have: group, subgroup, and copying the SEQUENCE.
    let emoji = query(&mut inst, "familia:emoji")?;
    if emoji.filtered != 3944 {
        bail!(
            "a familia `emoji` tem {} registros; esperava 3.944",
            emoji.filtered
        );
    }
    let carinhas = query(&mut inst, "grupo:carinhas")?;
    if carinhas.filtered == 0 || carinhas.filtered >= emoji.filtered {
        bail!(
            "`grupo:carinhas` nao restringiu: {} de {}",
            carinhas.filtered,
            emoji.filtered
        );
    }
    // SEARCHING IN PORTUGUESE, coming from the CLDR and not from the lexicon.
    let pt = query(&mut inst, "bandeira brasil")?;
    if pt.filtered == 0 {
        bail!("`bandeira brasil` nao achou nada — as palavras-chave do CLDR nao chegaram");
    }
    // COPYING A SEQUENCE carries the WHOLE sequence.
    //
    // THE DEFECT THIS CATCHES: copying only the first codepoint of 👩‍💻 would give
    // 👩 — the truncated emoji. It is the easiest defect to miss, because the
    // screen looks right and what goes into the document is wrong.
    query(&mut inst, "\u{1F469}\u{200D}\u{1F4BB}")?;
    let f = sheet(&mut inst)?;
    let hex = sheet_value(&f, "unicode-sheet-codepoint").unwrap_or_default();
    if !hex.contains("U+200D") {
        bail!("a ficha de 👩‍💻 mostra {hex:?}: a sequencia foi truncada no primeiro codepoint");
    }
    let copied = inst
        .event(&Input::Confirm)?
        .iter()
        .find_map(|e| match e {
            ronoctua_protocol::Effect::Copy(s) => Some(s.clone()),
            _ => None,
        })
        .context("copiar o emoji nao pediu Copiar")?;
    if copied.chars().count() != 3 {
        bail!(
            "copiar 👩‍💻 devolveu {copied:?} com {} caracteres; a sequencia tem 3",
            copied.chars().count()
        );
    }
    inst.event(&Input::Cancel)?;
    println!(
        "emoji     {} · carinhas {} · `bandeira brasil` {}",
        emoji.filtered, carinhas.filtered, pt.filtered
    );

    // ---- THE FILTERS RESTRICT ----
    let family = query(&mut inst, "familia:letra")?;
    let category = query(&mut inst, "cat:sm")?;
    let cat_larga = query(&mut inst, "cat:s")?;
    let block = query(&mut inst, "bloco:arrows")?;
    for (name, v) in [
        ("familia:letra", &family),
        ("cat:sm", &category),
        ("bloco:arrows", &block),
    ] {
        if v.filtered == 0 || v.filtered >= TOTAL {
            bail!("`{name}` nao restringiu: {} de {TOTAL}", v.filtered);
        }
    }
    // THE CATEGORY MATCHES BY PREFIX: `cat:s` catches the four symbol types, and
    // a half-typed query must not return zero while one is typing.
    if cat_larga.filtered <= category.filtered {
        bail!(
            "`cat:s` devolveu {} e `cat:sm` devolveu {}: o codigo esta casando exato\n\
             em vez de por prefixo",
            cat_larga.filtered,
            category.filtered
        );
    }
    // THE BLOCK IS BY NAME, and not by hexadecimal range: Unicode's block
    // boundaries do not fall on round multiples, and the other browser's shortcut
    // would answer wrong on every one of them.
    let f = sheet(&mut inst)?;
    let bloco_do_foco = sheet_value(&f, "unicode-sheet-block").unwrap_or_default();
    if !bloco_do_foco.to_lowercase().contains("arrows") {
        bail!("`bloco:arrows` deixou passar um caractere do bloco `{bloco_do_foco}`");
    }
    inst.event(&Input::Cancel)?;
    println!(
        "filtros   familia {} · cat:sm {} · cat:s {} · bloco {}",
        family.filtered, category.filtered, cat_larga.filtered, block.filtered
    );

    // ---- THE 302 COMBINING MARKS ----
    //
    // THE DEFECT THIS CATCHES: a mark drawn on its own takes up no room —
    // it disappears, or paints over the neighbouring cell. The conventional base is the
    // dotted circle, and what puts it there is the GENERATOR (the component cannot
    // write a codepoint, invariants 9 and 11).
    let marks = query(&mut inst, "familia:marca")?;
    if marks.filtered == 0 {
        bail!("nao ha marcas combinantes na base — o campo `familia` sumiu?");
    }
    let first = marks
        .drawings
        .first()
        .context("a familia das marcas nao desenhou nada")?;
    if first.chars().count() < 2 || !first.starts_with(CIRCLE) {
        bail!(
            "a marca combinante foi desenhada como {first:?}: sem o circulo pontilhado\n\
             ela some da celula ou pinta sobre a vizinha."
        );
    }
    // AND THE COPY DOES NOT CARRY THE CIRCLE ALONG.
    //
    // It is the worst defect this feature can have, and the only one that
    // survives any visual inspection: the screen looks right and what goes into
    // the user's document is wrong.
    let f = sheet(&mut inst)?;
    let cp_da_marca = sheet_value(&f, "unicode-sheet-codepoint")
        .context("a ficha da marca nao mostrou o codepoint")?;
    let copied = inst
        .event(&Input::Confirm)?
        .iter()
        .find_map(|e| match e {
            ronoctua_protocol::Effect::Copy(s) => Some(s.clone()),
            _ => None,
        })
        .context("copiar a marca nao pediu Copiar")?;
    if copied.chars().count() != 1 {
        bail!(
            "copiar a marca devolveu {copied:?}, com {} caracteres; devia ser UM",
            copied.chars().count()
        );
    }
    if copied.starts_with(CIRCLE) {
        bail!("copiar a marca devolveu o CIRCULO PONTILHADO — o apoio foi junto para o texto");
    }
    let expected = format!("U+{:04X}", copied.chars().next().unwrap_or('?') as u32);
    if expected != cp_da_marca {
        bail!("a ficha diz {cp_da_marca} e a copia devolveu {expected}");
    }
    inst.event(&Input::Cancel)?;
    inst.event(&Input::Cancel)?;
    println!(
        "marcas    {} desenhadas sobre `{CIRCLE}`, e a copia sai limpa",
        marks.filtered
    );

    // ---- THE 23 DOUBLE-WIDTH ONES ----
    let largos = query(&mut inst, "LONG LEFTWARDS ARROW")?;
    if largos.filtered == 0 {
        bail!("nao achei `LONG LEFTWARDS ARROW` — a base mudou?");
    }
    // THE BUDGET FOLLOWS THE INK, and the proof is a PAIR:
    //
    //   - in the query that brings up the long ones, SOME declare more than two columns;
    //   - in the `Arrows` block, where all are single-width, NONE declares.
    //
    // One side alone does not prove it: "LEFTWARDS ARROW" also matches "LONG LEFTWARDS
    // ARROW", so both maxima would give four and the test would pass with the
    // budget fixed at four for everyone — which would elide the rest.
    let largest = largos
        .budgets
        .iter()
        .copied()
        .max()
        .context("a grade nao desenhou o caractere largo")?;
    if largest <= ICON_COLUMNS {
        bail!(
            "nenhum dos caracteres longos declarou mais que {ICON_COLUMNS} colunas.\n\
             O host vai elidir um caractere so em `…`, pelo proprio orcamento."
        );
    }
    // `box drawing`, and not `arrows`: the block filter matches on the name's
    // CONTENT, so `arrows` also brings `Supplemental Arrows-A`, which is where
    // the long arrows live. The box drawing block has none.
    let simple = query(&mut inst, "bloco:box drawing")?;
    if simple.budgets.iter().any(|o| *o != ICON_COLUMNS) {
        bail!(
            "algum caractere de largura simples declarou orcamento diferente de \
             {ICON_COLUMNS}: a largura deixou de vir da fonte"
        );
    }
    // AND THE GRID'S GAP STILL ABSORBS THE LARGEST. Without this, tightening the
    // horizontal gap would make the 23 paint over their neighbour, and nothing else on
    // screen would say so.
    let step = simple
        .positions
        .windows(2)
        .find(|p| p[0].0 == p[1].0)
        .map(|p| p[1].1 - p[0].1)
        .context("a grade nao tem duas colunas para medir o passo")?;
    if largest >= step {
        bail!(
            "a tinta mais larga declara {largest} colunas e o passo da grade e {step}:\n\
             os caracteres de largura dupla vao pintar sobre o vizinho."
        );
    }
    let budget = largest;
    let normal = ICON_COLUMNS;
    // IT PASTES THE CHARACTER ITSELF to reach it, and not a query by name:
    // which result comes first is the ORDERING's business, and this criterion
    // is about width. Pasting a character from the database matches by identity.
    query(&mut inst, "\u{27F5}")?;
    // AND THE SHEET EXPLAINS WHY THAT CELL LOOKS ODD.
    let f = sheet(&mut inst)?;
    if !keys(&f).iter().any(|(k, _)| k == "unicode-sheet-width") {
        bail!(
            "a ficha de {:?} nao diz que ele e largo (chaves: {:?})",
            sheet_value(&f, "unicode-sheet-codepoint"),
            keys(&f).iter().map(|(k, _)| k.clone()).collect::<Vec<_>>()
        );
    }
    inst.event(&Input::Cancel)?;
    println!("largos    orcamento {budget} contra {normal} do normal");

    // ---- THE FAMILY PANEL ----
    query(&mut inst, "")?;
    let header = inst.render(COLUMNS, ROWS, heights())?;
    let (l_icone, c_icone) = header
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Text {
                line,
                column,
                text: Text::Icon(n),
                ..
            } if n == "filter" => Some((*line, *column)),
            _ => None,
        })
        .context("o cabecalho nao emitiu o icone de filtro")?;
    if inst
        .event(&Input::Click {
            column: c_icone,
            line: l_icone,
        })?
        .is_empty()
    {
        bail!("clicar no icone de filtro nao abriu o painel");
    }
    let open = inst.render(COLUMNS, ROWS, heights())?;
    let options = panel_of(&open);
    if options.is_empty() {
        bail!("o painel abriu sem nenhuma opcao");
    }
    for example in ["all", "symbol", "letter", "mark"] {
        let key = format!("unicode-family-{example}");
        if !options.iter().any(|(k, ..)| *k == key) {
            bail!("o painel nao ofereceu `{key}` — a familia sumiu da base?");
        }
    }
    // EVERY OPTION HAS A KEY IN BOTH LANGUAGES AND FITS THE BUDGET, and the LABEL
    // one reads finds, when typed into the search, the whole family.
    //
    // The last is what ties the two representations of the same fact together: the family's
    // name lives in the `.ftl` (because it gets translated) and becomes a tag in the generator (because
    // it gets searched), and nothing else forces the two to agree.
    for language in ["pt-BR", "en-US"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for (key, _, _, budget) in &options {
            let Some(text) = table.get(key) else {
                bail!("a opcao `{key}` nao existe em {language}.ftl — sairia como `[{key}]`");
            };
            if text.chars().count() > usize::from(*budget) {
                bail!(
                    "em {language}, `{key}` = {text:?} tem {} celulas e o painel reservou {budget}",
                    text.chars().count()
                );
            }
            let (prefix, valor) = if let Some(f) = key.strip_prefix("unicode-family-") {
                ("familia", f)
            } else if let Some(g) = key.strip_prefix("unicode-group-") {
                ("grupo", g)
            } else if let Some(v) = key.strip_prefix("unicode-tone-") {
                ("tom", v)
            } else {
                continue;
            };
            if valor == "all" {
                continue;
            }
            let so_a_familia = query(&mut inst, &format!("{prefix}:{valor}"))?.filtered;
            let by_name = query(&mut inst, &text.to_lowercase())?.filtered;
            if by_name < so_a_familia {
                bail!(
                    "em {language}, o painel mostra {text:?} e buscar por isso acha {by_name}\n\
                     caracteres — `{prefix}:{valor}` tem {so_a_familia}. O `.ftl` e o gerador\n\
                     divergiram, e a tela esta ensinando uma palavra que a busca nao conhece."
                );
            }
        }
        println!(
            "painel    {language:8} -> as {} opcoes cabem e acham",
            options.len()
        );
    }

    // ---- THE EMOJI SUB-FILTERS ARE ALWAYS THERE, AND SEPARATED ----
    //
    // They used to appear only with the `emoji` family selected, and the objection was
    // a good one: under `family:letra`, `group:comida` narrows to ZERO. What it did not
    // foresee is the cost of the opposite — a panel that CHANGES size under the
    // hand, with the options moving between one Tab and the next, and with
    // no way to find out that the sub-filters exist.
    //
    // The answer is not to hide, but to SEPARATE: a rule, a small label, and
    // they stay. This criterion asserts both halves — that they are always there, and
    // that the separation is there with them.
    let subfiltros =
        |inst: &mut ronoctua_runtime::Instance, busca: &str| -> Result<(usize, usize)> {
            query(inst, busca)?;
            if panel_of(&inst.render(COLUMNS, ROWS, heights())?).is_empty() {
                inst.event(&Input::Toggle)?;
            }
            let p = panel_of(&inst.render(COLUMNS, ROWS, heights())?);
            Ok((
                p.iter()
                    .filter(|(k, ..)| k.starts_with("unicode-group-"))
                    .count(),
                p.iter()
                    .filter(|(k, ..)| k.starts_with("unicode-tone-"))
                    .count(),
            ))
        };
    for postura in [
        "familia:emoji",
        "grupo:comida",
        "familia:letra",
        "familia:marca",
        "",
    ] {
        let (groups, tons) = subfiltros(&mut inst, postura)?;
        if groups != 9 || tons != 5 {
            bail!(
                "com `{postura}` o painel mostrou {groups} grupos e {tons} tons; \
                 sao sempre 9 e 5"
            );
        }
    }

    // THE SEPARATION: the rule and the section's label, on screen, together with them.
    //
    // Without this criterion, "always visible" would become "mixed in with the rest" — and
    // then the old objection would hold again, because nothing would say what the nine
    // groups apply to.
    query(&mut inst, "familia:letra")?;
    if panel_of(&inst.render(COLUMNS, ROWS, heights())?).is_empty() {
        inst.event(&Input::Toggle)?;
    }
    let with_panel = inst.render(COLUMNS, ROWS, heights())?;
    let label = with_panel.commands.iter().find_map(|c| match c {
        Command::Text {
            line,
            text: Text::Key(k),
            ..
        } if k == "unicode-panel-emoji" => Some(*line),
        _ => None,
    });
    let Some(label_row) = label else {
        bail!("o painel nao trouxe o rotulo da secao de emoji — os subfiltros ficam sem legenda");
    };
    if !with_panel
        .commands
        .iter()
        .any(|c| matches!(c, Command::Rule { line, .. } if *line + 1 == label_row))
    {
        bail!("nao ha regua acima do rotulo da secao — a secao nao se ve separada");
    }
    // AND THE SUB-FILTERS COME AFTER IT. A label below what it names
    // names something else.
    let primeiro_subfiltro = panel_of(&with_panel)
        .into_iter()
        .filter(|(k, ..)| k.starts_with("unicode-group-") || k.starts_with("unicode-tone-"))
        .map(|(_, l, ..)| l)
        .min()
        .context("os subfiltros sumiram da tela")?;
    if primeiro_subfiltro <= label_row {
        bail!("os subfiltros aparecem ANTES do rotulo que os nomeia");
    }

    // THE TONES REALLY DO NARROW, and each narrows a different slice.
    let mut seen = Vec::new();
    for tom in ["light", "medium-light", "medium", "medium-dark", "dark"] {
        let n = query(&mut inst, &format!("tom:{tom}"))?.filtered;
        if n == 0 {
            bail!("`tom:{tom}` nao acha nada — o tom nao chegou a base");
        }
        seen.push(n);
    }
    let soma: usize = seen.iter().sum();
    let so_emoji = query(&mut inst, "familia:emoji")?.filtered;
    if soma >= so_emoji {
        bail!("os cinco tons somam {soma} de {so_emoji} emoji — eles nao estao recortando");
    }
    // AND A FREE TERM COEXISTS with the tone, as with the other enumeration
    // filters: `tom:dark mao` is the actual gesture of whoever is looking for a hand.
    let so_tom = query(&mut inst, "tom:escuro")?.filtered;
    let with_term = query(&mut inst, "tom:escuro mao")?.filtered;
    if with_term == 0 || with_term >= so_tom {
        bail!("`tom:escuro mao` devolveu {with_term} de {so_tom} — o termo livre nao recortou");
    }
    inst.event(&Input::Cancel)?;
    println!(
        "subfiltros 9 grupos e 5 tons em toda postura, separados por regua e rotulo\n\
         tons      {seen:?} de {so_emoji} emoji · `tom:escuro mao` -> {with_term}"
    );

    // ---- WHERE THE SELECTION IS, IN A GRID THAT DOES NOT OBEY COLOUR ----
    //
    // The token's colour is no use for emoji: it is INSIDE the glyph (a CBDT
    // bitmap), and `state-accent` does not change a pixel of them. In 3,944 of the 11,614
    // cells the selection was invisible — and precisely in the ones the buffer OPENS
    // showing. The marker is an icon, on the row below the focused cell.
    let marcador = |list: &ronoctua_protocol::DisplayList| -> Option<(u16, u16)> {
        list.commands.iter().find_map(|c| match c {
            Command::Text {
                line,
                column,
                text: Text::Icon(i),
                ..
            } if i == "selection" => Some((*line, *column)),
            _ => None,
        })
    };
    query(&mut inst, "familia:emoji")?;
    let before = marcador(&inst.render(COLUMNS, ROWS, heights())?)
        .context("a grade nao marcou a selecao — em emoji ela ficaria invisivel")?;
    inst.event(&Input::Right)?;
    let after = marcador(&inst.render(COLUMNS, ROWS, heights())?)
        .context("o marcador sumiu ao mover o cursor")?;
    if before == after {
        bail!("o marcador nao andou com o cursor: ficou em {before:?}");
    }
    if before.0 != after.0 {
        bail!("andar para o lado mudou a LINHA do marcador: {before:?} -> {after:?}");
    }
    inst.event(&Input::Left)?;
    println!("marcador  {before:?} -> {after:?} ao andar uma celula");

    // ---- `Tab` TOGGLES, AND DOES NOT GO INTO THE SEARCH ----
    //
    // The other driver's approach (looking for the invitation on screen) does NOT work here:
    // this buffer opens with text in the field and never shows the invitation. The
    // assertion has to be about the string, byte by byte.
    // IT CLOSES THE PANEL FIRST. It has been open since the click on the icon, and a
    // `Tab` over an open panel CLOSES it — the test would measure the opposite of what it
    // wants to measure.
    if !panel_of(&inst.render(COLUMNS, ROWS, heights())?).is_empty() {
        inst.event(&Input::Cancel)?;
    }
    query(&mut inst, INITIAL_FILTER)?;
    let before = screen(&mut inst)?.field.unwrap_or_default();
    inst.event(&Input::Toggle)?;
    if panel_of(&inst.render(COLUMNS, ROWS, heights())?).is_empty() {
        bail!("`Tab` nao abriu o painel");
    }
    inst.event(&Input::Toggle)?;
    let depois_tela = inst.render(COLUMNS, ROWS, heights())?;
    if !panel_of(&depois_tela).is_empty() {
        bail!("o segundo `Tab` nao fechou o painel");
    }
    let after = read(&depois_tela)?.field.unwrap_or_default();
    if before != after {
        bail!(
            "o campo era {before:?} e depois de dois `Tab` e {after:?}: a tecla entrou\n\
             na busca como caractere, e uma tabulacao no campo e invisivel."
        );
    }
    println!("tab       abre, fecha, e nao toca no campo");

    // ---- THE CLICK LANDS ON WHAT WAS DRAWN THERE ----
    query(&mut inst, "familia:letra")?;
    let grid = screen(&mut inst)?;
    const TARGET: usize = 3;
    let (line, column) = *grid
        .positions
        .get(TARGET)
        .context("a grade nao desenhou celulas suficientes")?;
    let desenhado = grid.drawings[TARGET].clone();
    if inst.event(&Input::Click { column, line })?.is_empty() {
        bail!("o clique em ({column},{line}) nao pediu redesenho");
    }
    // THE FOCUS IS READ FROM THE SHEET, and not from the drawing: for a mark the drawing starts
    // at the circle, and comparing drawings would give false.
    let f = sheet(&mut inst)?;
    let cp = sheet_value(&f, "unicode-sheet-codepoint").context("ficha sem codepoint")?;
    let expected = desenhado
        .chars()
        .next_back()
        .map(|c| format!("U+{:04X}", c as u32))
        .unwrap_or_default();
    if cp != expected {
        bail!("o clique em ({column},{line}) focou {cp}, e ali estava desenhado {expected}");
    }
    inst.event(&Input::Cancel)?;
    println!("clique    ({column},{line}) -> {cp}");

    // ---- THE SHEET CARRIES THE VALUES, AND THE COPY REPORTS BACK ----
    query(&mut inst, "FOR ALL")?;
    let f = sheet(&mut inst)?;
    let valores: Vec<String> = f
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::Text {
                text: Text::Data(s),
                ..
            } => Some(s.clone()),
            _ => None,
        })
        .collect();
    for expected in ["U+2200", "8704", "E2 88 80", "\\u{2200}", "&#x2200;"] {
        if !valores.iter().any(|v| v == expected) {
            bail!(
                "a ficha de `FOR ALL` nao mostra {expected:?}. Uma lista de rotulos sem valor\n\
                 e uma lista de promessas: {valores:?}"
            );
        }
    }
    // `Down` + `Enter` copies the SECOND option, and not the first.
    inst.event(&Input::Down)?;
    let copied = inst
        .event(&Input::Confirm)?
        .iter()
        .find_map(|e| match e {
            ronoctua_protocol::Effect::Copy(s) => Some(s.clone()),
            _ => None,
        })
        .context("copiar nao pediu Copiar")?;
    if copied != "FOR ALL" {
        bail!("baixo+Enter copiou {copied:?}; a segunda opcao e o NOME");
    }
    // THE VERDICT APPEARS, and goes away on the next key.
    inst.event(&Input::Copied(true))?;
    if !keys(&inst.render(COLUMNS, ROWS, heights())?)
        .iter()
        .any(|(k, _)| k == "unicode-sheet-copied")
    {
        bail!("depois de copiar, a ficha nao mostra a confirmacao");
    }
    inst.event(&Input::Down)?;
    if keys(&inst.render(COLUMNS, ROWS, heights())?)
        .iter()
        .any(|(k, _)| k == "unicode-sheet-copied")
    {
        bail!("o aviso de copia ficou na tela depois da tecla seguinte");
    }
    // EVERY SHEET LABEL FITS THE BUDGET, in both languages.
    let keys = keys(&inst.render(COLUMNS, ROWS, heights())?);
    for language in ["pt-BR", "en-US"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for (key, budget) in &keys {
            let Some(text) = table.get(key) else {
                bail!("a chave `{key}` da ficha nao existe em {language}.ftl");
            };
            if text.chars().count() > usize::from(*budget) {
                bail!(
                    "em {language}, `{key}` = {text:?} tem {} celulas e o buffer reservou {budget}",
                    text.chars().count()
                );
            }
        }
    }
    inst.event(&Input::Cancel)?;
    println!("ficha     valores, copia com veredito, e os rotulos cabem");

    // ---- PASTING A CHARACTER FINDS THE CHARACTER, EVEN WITH THE FILTER ACTIVE ----
    //
    // THE DEFECT THIS CATCHES, and only this buffer has it: it OPENS filtered on
    // symbols. Pasting a letter would return "no results" — because of a
    // filter whoever pasted did not type. The question is "what character is this?",
    // and the answer must not depend on the posture the buffer happened to be in.
    query(&mut inst, INITIAL_FILTER)?;
    inst.event(&Input::Paste("α".to_owned()))?;
    let pasted = screen(&mut inst)?;
    if pasted.filtered == 0 {
        bail!("colar uma LETRA com o filtro de simbolos ativo nao achou nada");
    }
    let f = sheet(&mut inst)?;
    let cp = sheet_value(&f, "unicode-sheet-codepoint").context("ficha sem codepoint")?;
    if cp != "U+03B1" {
        bail!("colar `α` achou {cp}");
    }
    inst.event(&Input::Cancel)?;
    println!("colar     `α` com `simbolos:` ativo -> {cp}");

    println!("OK: a base tem {TOTAL}, abre filtrada e sai do filtro, os filtros restringem,");
    println!("    as marcas aparecem sobre o circulo e copiam limpas, os largos nao");
    println!("    sao elididos, o painel amarra rotulo e busca, e a ficha copia com aviso");
    Ok(())
}
