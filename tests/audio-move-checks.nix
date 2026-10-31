# SENDING A PROGRAM TO ANOTHER SOUND DEVICE (owner, 2026-10-31): the second
# program that writes to PipeWire's graph, and what bounds it.
#
#   - `audio-move-writes-one-thing`: `pw-metadata` is run from ONE function of
#     the boundary (`pipewire.rs`, `send_away`), with the arguments ONE pure
#     function builds (`move_argv`): the `default` metadata, the stream as the
#     subject, the key `target.object`, the sink's SERIAL typed `Spa:Id` —
#     never a delete, another key or the sink's id. The `Move` it takes has
#     private fields and is built only in `Snapshot::send`, which asks the
#     reading for a playing stream that may be moved and a sink with a serial.
#     No other source of the workspace runs the program, names its variable or
#     writes the key (`graph.rs` only READS it).
#
# Joined into `tests/checks.nix` with one line, like every file of checks a
# wave adds.
{
  pkgs,
  mkCheck,
  pyPrelude,
}:
{
  audio-move-writes-one-thing = mkCheck "audio-move-writes-one-thing" [ pkgs.python3 ] ''
    python3 - <<'END'
    import pathlib, re, sys
    exec(open("${pyPrelude}").read())

    # FILE-WIDE: a write is a program run and an argument built, so the
    # boundary's two files are the question; the cut at `mod tests` is the
    # narrowing, and comments are prose about the boundary, not calls
    # (`uncommented`, never `code_of`: every name here is a string literal).
    crates = pathlib.Path("${../crates}")
    audio = crates / "ronoctua-audio" / "src"
    wrong = []

    def product(path):
        return without_tests(uncommented(path.read_text()))

    boundary = product(audio / "pipewire.rs")
    lib = product(audio / "lib.rs")
    floor(len(boundary), 5000, "characters of pipewire.rs")
    floor(len(lib), 5000, "characters of lib.rs")

    # THE PROGRAM, baked once and run from one function.
    if boundary.count('option_env!("RONOCTUA_PW_METADATA")') != 1:
        wrong.append("pipewire.rs does not bake `RONOCTUA_PW_METADATA` exactly once")
    send_away = body_of(boundary, "send_away")
    must(send_away, "`send_away` in pipewire.rs")
    uses = len(re.findall(r"\bPW_METADATA\b", boundary))
    if uses != 2 or "PW_METADATA" not in send_away:
        wrong.append(f"`PW_METADATA` is named {uses} times; it is defined once and run once, in `send_away`")
    for needed in ("move_argv(m)", '"PIPEWIRE_DEBUG", "E"', '"LC_ALL", "C"', "answer_of("):
        if needed not in send_away:
            wrong.append(f"`send_away` lost `{needed}`: the arguments are the pure function's, and the server's refusal is read from its log")
    carry = body_of(boundary, "carry_out")
    must(carry, "`carry_out` in pipewire.rs")
    if "send_away(" not in carry or "Order::Move" not in carry:
        wrong.append("`carry_out` no longer hands a move to `send_away`")

    # THE ARGUMENTS: one key, one metadata, the serial, nothing that deletes.
    argv = body_of(boundary, "move_argv")
    must(argv, "`move_argv` in pipewire.rs")
    for needed in ('"--name"', '"default"', "m.stream()", '"target.object"', "m.serial()", '"Spa:Id"'):
        if needed not in argv:
            wrong.append(f"`move_argv` lost `{needed}`")
    for refused in ('"-d"', '"--delete"', '"target.node"', "m.sink()", '"-n"'):
        if refused in argv:
            wrong.append(f"`move_argv` writes `{refused}`: it sets one key of a stream to a serial, and nothing else")
    if boundary.count('"target.object"') != 1:
        wrong.append("`target.object` is written in pipewire.rs outside `move_argv`")

    # THE MOVE: private, and made only out of a reading that listed both ends.
    held = re.search(r"pub struct Move \{(.*?)\n\}", lib, re.S)
    if not held:
        wrong.append("lib.rs has no `pub struct Move`")
    elif re.search(r"^\s*pub\b", held.group(1), re.M):
        wrong.append("a field of `Move` is public: anything could name a device to write")
    # A LITERAL, and not the declaration above it.
    made = [m.start() for m in re.finditer(r"(?<!struct )\bMove \{\s*stream\b", lib)]
    send = body_of(lib, "send")
    must(send, "`Snapshot::send` in lib.rs")
    if len(made) != 1 or not send or "Move {" not in send:
        wrong.append(f"`Move` is built {len(made)} times in lib.rs; only `Snapshot::send` builds one")
    for needed in ("playing_stream(", "Kind::Sink", ".serial"):
        if needed not in (send or ""):
            wrong.append(f"`Snapshot::send` no longer asks `{needed}`")
    playing = body_of(lib, "playing_stream")
    must(playing, "`playing_stream` in lib.rs")
    for needed in ("Kind::Playing", "fixed"):
        if needed not in (playing or ""):
            wrong.append(f"`playing_stream` no longer asks `{needed}`: a recording, or a program that asked to stay, could be moved")

    # AND NOBODY ELSE: the program, its variable and the key, outside these.
    swept = 0
    for f in sorted(crates.rglob("*.rs")):
        if f.parent == audio and f.name in ("pipewire.rs", "graph.rs", "lib.rs"):
            continue
        code = product(f)
        swept += 1
        for name in ("pw-metadata", "RONOCTUA_PW_METADATA", "target.object"):
            if name in code:
                wrong.append(f"{f.relative_to(crates)} names `{name}` outside the boundary")
    floor(swept, 500, "sources of the workspace swept")
    for name in ("pw-metadata", "RONOCTUA_PW_METADATA", "target.object"):
        if name in lib:
            wrong.append(f"lib.rs names `{name}` in code; the boundary's programs and keys are pipewire.rs's")
    graph = product(audio / "graph.rs")
    if "pw-metadata" in graph or "RONOCTUA_PW_METADATA" in graph:
        wrong.append("graph.rs names the program that writes; it only reads")

    for w in wrong:
        print(f"ERROR: {w}", file=sys.stderr)
    if wrong:
        sys.exit(1)
    print(f"OK: one function runs pw-metadata, one builds its words (target.object of a playing stream, to a sink's serial), one makes the move it takes; {swept} other sources name none of it")
    END
  '';
}
