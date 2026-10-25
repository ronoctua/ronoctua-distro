# The parts' watch's own checks, in the shape every module's `checks.nix` has
# (`nix/brain-modules.nix`, `checkArguments`).
{
  mkCheck,
  pkgs,
  pyPrelude,
  ...
}:

{
  checks = {
    # THE MODULE WATCHES WHERE THE PARTS' CRATE SAYS THE RECORDS ARE, and its
    # mark opens a command the shell's registry has.
    #
    # A watch on a path written here would go quiet the day the crate moved
    # the records; a mark that opened a command nobody registered would be a
    # click that does nothing.
    parts-watch-watches-the-records = mkCheck "parts-watch-watches-the-records" [ pkgs.python3 ] ''
      python3 - <<'END'
      import pathlib, re, sys
      exec(open("${pyPrelude}").read())

      # FILE-WIDE: the module's code without its tests (`without_tests`, its
      # strings kept by `uncommented`: the folder's name and the command ARE
      # strings), searched whole — a path or a program of its own anywhere in
      # it is the defect; and the parts' page and the update view, searched
      # whole for the one constant that names the command (`pub const OPEN`)
      # and the one registration of it (`id: crate::parts::OPEN,`), each a
      # line that exists once in its file.
      module = uncommented(without_tests(pathlib.Path("${./module.rs}").read_text()))
      page = uncommented(pathlib.Path("${../../../ronoctua-shell-update/src/parts.rs}").read_text())
      view = uncommented(pathlib.Path("${../../../ronoctua-shell-update/src/update_window.rs}").read_text())
      floor(len(module), 2000, "characters of the module's code")
      floor(len(view), 20000, "characters of the update view")
      wrong = []
      if "ronoctua_parts::names::RECORDS" not in module:
          wrong.append("the module does not watch the folder the parts' crate names")
      for path in re.findall(r'"(/[^"]*)"', module):
          wrong.append(f"the module writes a path of its own: {path}")
      if re.search(r"Command::new|Source::Lines", module):
          wrong.append("the module runs a program; it only reads records")
      opens = re.search(r'const OPENS: &str = "([a-z-]+)";', module)
      if not opens:
          wrong.append("the module no longer says which command its mark opens")
      elif f'pub const OPEN: &str = "{opens.group(1)}";' not in page or "id: crate::parts::OPEN," not in view:
          wrong.append(f"the mark opens `{opens.group(1)}`, which the update view does not register as the parts' page")
      if wrong:
          for w in wrong:
              print("ERROR: " + w, file=sys.stderr)
          sys.exit(1)
      print(f"OK: the module watches the parts' records and its mark opens `{opens.group(1)}`")
      END
    '';
  };
  slow = [ ];
}
