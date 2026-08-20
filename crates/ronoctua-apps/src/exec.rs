//! An entry's `Exec=`, becoming `(program, arguments)`.
//!
//! Two phases, and the order matters: **tokenise first, expand afterwards**.
//! Expanding before tokenising would make a `%c` whose value has a space — the
//! application's name, which almost always does — become two arguments.
//!
//! # Why not `split_whitespace`
//!
//! Because the spec defines double quotes, and they do appear: measured on a actual host,
//! **6 of 108** files use them. The case that hurts is a `/nix/store` path
//! with a space — the app does not open, and the error comes out as "could not open", far
//! from the cause.

/// What `%i`, `%c` and `%k` need to know about the entry.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    /// The `Icon=`, for `%i`.
    pub icon: &'a str,
    /// The already-localised name, for `%c`.
    pub name: &'a str,
    /// The path of the `.desktop` itself, for `%k`.
    pub file: &'a str,
    /// What to OPEN, for `%f`, `%F`, `%u` and `%U`. Empty when there is
    /// nothing — which is the app launcher's case.
    ///
    /// # Why the codes used to be dropped instead
    ///
    /// Because until the file manager there was nothing to put in them. `%f`
    /// left in place makes an application open a file literally named `%F` —
    /// 66 of 108 measured entries carry one — so dropping them was right for
    /// the one caller there was. With a file to hand over, dropping them means
    /// the application opens with nothing, which looks exactly like an
    /// application that ignored the request.
    pub open: &'a [String],
}

/// Splits the line into tokens, honouring the spec's quotes.
///
/// Inside double quotes, `\` escapes `"`, `` ` ``, `$` and `\`; outside them, a
/// space separates. The spec also demands escaping the backslash itself when
/// writing, and that is why `\\` becomes `\`.
#[must_use]
pub fn tokenize(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut has = false;
    let mut quotes = false;
    let mut cs = line.chars();
    while let Some(c) = cs.next() {
        match c {
            '"' => {
                quotes = !quotes;
                // QUOTES OPEN A TOKEN, even an empty one: `Exec=prog ""` passes an
                // empty argument, and swallowing it would change the command line.
                has = true;
            }
            '\\' if quotes => {
                // The escape only exists inside the quotes. Outside, `\` is literal —
                // it is what appears in Wine paths.
                if let Some(p) = cs.next() {
                    current.push(p);
                    has = true;
                }
            }
            c if c.is_whitespace() && !quotes => {
                if has {
                    out.push(std::mem::take(&mut current));
                    has = false;
                }
            }
            c => {
                current.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(current);
    }
    out
}

/// Expands the field codes of ONE token.
///
/// It returns **zero, one or two** tokens — and that is why the signature is not
/// `String`:
///
/// - `%f %F %u %U` → **zero**. We launch with no file and no URL. Measured: **66 of
///   108** `Exec` lines carry one of them; without removing it, `code %F` opens a
///   file called `%F`.
/// - `%i` → **two**: `--icon` and the value. A single one would pass the option with no value, and
///   the app would complain about syntax.
/// - `%c`, `%k`, `%%` → one.
/// - `%d %D %n %N %v %m` → zero; they are deprecated in the spec.
fn expand(token: &str, ctx: Context<'_>) -> Vec<String> {
    // The common case: no `%` at all. It exits without allocating anything beyond the copy.
    if !token.contains('%') {
        return vec![token.to_owned()];
    }
    // The WHOLE token is a code: it is the form the spec uses and the only one in which
    // "disappears" and "becomes two" make sense.
    match token {
        // ONE for the singular codes, ALL for the plural ones — which is what
        // the spec says, and what decides whether opening four files starts one
        // process or four.
        "%f" | "%u" => return ctx.open.first().cloned().into_iter().collect(),
        "%F" | "%U" => return ctx.open.to_vec(),
        "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => return Vec::new(),
        "%i" => {
            return if ctx.icon.is_empty() {
                Vec::new()
            } else {
                vec!["--icon".to_owned(), ctx.icon.to_owned()]
            };
        }
        "%c" => return vec![ctx.name.to_owned()],
        "%k" => return vec![ctx.file.to_owned()],
        _ => {}
    }
    // A `%` in the middle of the token: only `%%` has meaning here. An unknown
    // code disappears along with the token and says why — the spec demands invalidating the
    // whole entry, but making a working application disappear over
    // a vendor's typo is worse than ignoring the argument.
    let mut out = String::new();
    let mut cs = token.chars();
    while let Some(c) = cs.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match cs.next() {
            Some('%') => out.push('%'),
            Some(otherwise) => {
                tracing::debug!(code = %otherwise, token, "codigo de campo desconhecido; token fora");
                return Vec::new();
            }
            None => out.push('%'),
        }
    }
    vec![out]
}

/// The `Exec=` line becoming `(program, arguments)`.
///
/// `None` when no program is left — an empty line, or field codes only.
#[must_use]
pub fn assemble(line: &str, ctx: Context<'_>) -> Option<(String, Vec<String>)> {
    let mut ready: Vec<String> = Vec::new();
    for t in tokenize(line) {
        ready.extend(expand(&t, ctx));
    }
    let mut it = ready.into_iter();
    let program = it.next()?;
    Some((program, it.collect()))
}

#[cfg(test)]
mod tests {
    use super::{Context, assemble, tokenize};

    const CTX: Context<'static> = Context {
        icon: "firefox",
        name: "Navegador Web",
        file: "/usr/share/applications/firefox.desktop",
        open: &[],
    };

    /// THE DEFECT: `%F` passed literally opens a file called `%F`.
    #[test]
    fn the_file_codes_vanish() {
        // With NOTHING to open — the app launcher's case, which is 66 of 108
        // measured entries.
        for line in ["code %F", "vlc %U", "gedit %f", "eog %u"] {
            let (_, args) = assemble(line, CTX).expect("tem programa");
            assert!(args.is_empty(), "`{line}` deixou {args:?}");
        }
    }

    #[test]
    fn with_something_to_open_the_codes_are_filled() {
        // THE DEFECT THIS CATCHES: dropping the code when there IS a file. The
        // application opens with nothing, which looks exactly like an
        // application that ignored the request.
        const WITH: Context<'static> = Context {
            icon: "firefox",
            name: "Navegador Web",
            file: "/usr/share/applications/firefox.desktop",
            open: &[],
        };
        let two = ["/tmp/a.png".to_owned(), "/tmp/b.png".to_owned()];
        let ctx = Context { open: &two, ..WITH };
        // The singular codes take ONE, the plural ones take all — which is what
        // decides whether opening four files starts one process or four.
        let (_, args) = assemble("eog %f", ctx).expect("tem programa");
        assert_eq!(args, ["/tmp/a.png"]);
        let (_, args) = assemble("eog %F", ctx).expect("tem programa");
        assert_eq!(args, ["/tmp/a.png", "/tmp/b.png"]);
        let (_, args) = assemble("vlc %U", ctx).expect("tem programa");
        assert_eq!(args, ["/tmp/a.png", "/tmp/b.png"]);
    }

    /// THE DEFECT: `%i` becoming a single token passes `--icon` with no value.
    #[test]
    fn the_icon_becomes_two_tokens() {
        let (p, args) = assemble("prog %i --foo", CTX).expect("tem programa");
        assert_eq!(p, "prog");
        assert_eq!(args, ["--icon", "firefox", "--foo"]);
    }

    /// THE DEFECT: `split_whitespace` over a store path with a space.
    #[test]
    fn the_quotes_hold_the_space() {
        let (p, args) =
            assemble(r#""/nix/store/a b/bin/x" --f "dois termos""#, CTX).expect("tem programa");
        assert_eq!(p, "/nix/store/a b/bin/x");
        assert_eq!(args, ["--f", "dois termos"]);
    }

    #[test]
    fn the_backslash_escapes_inside_quotes_and_is_literal_outside() {
        assert_eq!(tokenize(r#""diz \"oi\"""#), [r#"diz "oi""#]);
        assert_eq!(tokenize(r"C:\jogos\x.exe"), [r"C:\jogos\x.exe"]);
    }

    #[test]
    fn name_and_path_go_in_where_asked_for() {
        let (_, args) = assemble("prog %c %k %%", CTX).expect("tem programa");
        assert_eq!(
            args,
            [
                "Navegador Web",
                "/usr/share/applications/firefox.desktop",
                "%"
            ]
        );
    }

    #[test]
    fn an_unknown_code_carries_only_its_own_token() {
        let (p, args) = assemble("prog %z --fica", CTX).expect("tem programa");
        assert_eq!(p, "prog");
        assert_eq!(args, ["--fica"], "o resto da linha sobrevive");
    }

    #[test]
    fn a_line_with_no_program_gives_back_nothing() {
        assert!(assemble("", CTX).is_none());
        assert!(assemble("   ", CTX).is_none());
        assert!(
            assemble("%F", CTX).is_none(),
            "so codigo de campo nao e programa"
        );
    }
}
