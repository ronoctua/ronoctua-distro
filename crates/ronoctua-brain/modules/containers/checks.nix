# The containers module's own checks, in the shape every module's `checks.nix`
# has (`nix/brain-modules.nix`, `checkArguments`).
{
  mkCheck,
  pkgs,
  pyPrelude,
  ...
}:

{
  checks = {
    # THE MODULE WATCHES THE FOLDER crun KEEPS, AND THE CLICK OPENS THE VIEW.
    #
    # Three places have to agree on one fact, and none of them fails on its
    # own when another moves: the boundary crate says where crun's state is
    # (`runtime_dir`: `$XDG_RUNTIME_DIR/crun`), the module watches what the
    # crate says — never a path of its own — and the command its mark opens
    # is one the shell's registry has. A module that watched a path written
    # here would go quiet the day the crate learnt a new layout; a mark that
    # opened a command nobody registered would be a click that does nothing.
    containers-watches-where-crun-keeps-its-folders =
      mkCheck "containers-watches-where-crun-keeps-its-folders" [ pkgs.python3 ]
        ''
          python3 - <<'END'
          import pathlib, re, sys
          exec(open("${pyPrelude}").read())

          # PARSED: one function's body in the crate (`body_of`, its strings
          # kept by `uncommented`: the variable and the folder ARE strings),
          # the module's code without its tests (`without_tests`), and the
          # registry's ids as the containers' keymap contribution writes them
          # (`id: "…"` in `KEYS`, cut at its own closing `};`).
          crate = uncommented(without_tests(pathlib.Path("${../../../ronoctua-containers/src/lib.rs}").read_text()))
          module = uncommented(without_tests(pathlib.Path("${./module.rs}").read_text()))
          view = uncommented(pathlib.Path("${../../../ronoctua-shell-containers/src/containers.rs}").read_text())
          keys = view.find("pub const KEYS: crate::bindings::Contribution")
          registry = view[keys : view.find("\n};\n", keys)] if keys >= 0 else ""
          floor(len(module), 2000, "characters of the module's code")
          floor(len(registry), 2000, "characters of the containers' keymap contribution")
          wrong = []

          # THE FOLDER IS `runtime_dir`'s ANSWER, asked of its own body: the
          # same two strings anywhere else in the crate would say nothing
          # about the function the module calls.
          runtime = must(body_of(crate, "runtime_dir"), "the crate's `runtime_dir`")
          if '"XDG_RUNTIME_DIR"' not in runtime:
              wrong.append("`runtime_dir` no longer reads XDG_RUNTIME_DIR for crun's folder")
          if 'join("crun")' not in runtime:
              wrong.append("`runtime_dir` no longer puts crun's folders under `crun`")

          if "ronoctua_containers::runtime_dir()" not in module:
              wrong.append("the module does not watch the folder the crate names")
          for path in re.findall(r'"(/[^"]*)"', module):
              wrong.append(f"the module writes a path of its own: {path}")
          if "Source::Path" not in module:
              wrong.append("the module does not watch a path")
          if re.search(r"Command::new|Source::Lines", module):
              wrong.append("the module runs a program; it only counts folders")

          opens = re.search(r'const OPENS: &str = "([a-z-]+)";', module)
          if not opens:
              wrong.append("the module no longer says which command its mark opens")
          elif f'id: "{opens.group(1)}"' not in registry:
              wrong.append(f"the mark opens `{opens.group(1)}`, which the shell's registry does not have")

          if wrong:
              for w in wrong:
                  print("ERROR: " + w, file=sys.stderr)
              sys.exit(1)
          print(f"OK: the module watches the crate's crun folder and its mark opens `{opens.group(1)}`")
          END
        '';
  };
  slow = [ ];
}
