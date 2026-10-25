# WHAT THE UNITS MAKE INTO THINGS PEOPLE RUN — nix-design §2.
#
# `nix/rust/units.nix` gives every `[[bin]]` a derivation of its own; this
# file gives each of them its place:
#
#   bins.<bin>       every release program but an `apart` one, as built — what
#                    a check runs where it used to run `${workspace}/bin/<bin>`
#   <name>           the 31 named programs a module, an app or the ISO runs
#                    (`installer`, `rescue`, `brain`, …), the unit itself or,
#                    for one that paints, the unit under a wrapper that carries
#                    the palettes
#   shell, loopShell the footer as the system and `.#dev` run it: the shell
#                    and what it looks for beside itself, in one `bin/`
#   benches          the four benchmark programs, for `.#bench`
#   shipped          every `ship` program in the form a machine runs it, and
#                    the shell: the ISO's seed
#
# # A program's role
#
# Declared by its crate (`bins` in `[package.metadata.ronoctua]`; `ship` when
# not said): `ship` reaches a machine, `driver` is run by a check or a VM test,
# `bench` by `.#bench`, `apart` is built elsewhere (`ronoctua-git-askpass`, in
# `nix/git-askpass.nix`: the git library bakes its path, and a unit of the plan
# cannot be an input of a library below it). Nothing here ships a `driver` or
# a `bench` program: the system's closure and the seed hold the shell bundle
# and the programs the modules name, and no test or benchmark rides along —
# crane's workspace put all fifty programs in every closure that reached the
# shell.
{
  lib,
  pkgs,
  makeBinaryWrapper,
  # The release and loop units (`nix/rust/units.nix`).
  release,
  loop,
  # What each crate declares (`beside` above all), as the units read it.
  crates,
  # The wrapper's table and its flags (`nix/rust.nix`): everything read at
  # run time, and `--set` for what a binary is keyed to (`keyed`, there),
  # `--set-default` for the rest.
  atRunTime,
  wrapFlags,
  # THE PROGRAMS THAT PAINT, beside the footer, and what each is keyed to
  # (`painters` in `nix/rust.nix`): the PNG camera, the rescue console and
  # the installer (a terminal, in the palette's colours) paint and speak; the
  # player's daemon (the subtitle lanes' colours) paints and says no word.
  # Wherever one is installed it carries its keyed data, and only that — the
  # rescue console and the installer run with no coverage faces and no atlas
  # on purpose (`RONOCTUA_COVERAGE_FONTS`).
  painters,
}:

let
  # Their names (for `lib.elem`, and for the checks).
  painterNames = lib.attrNames painters;

  roleOf = units: bin: units.bins.${bin}.passthru.role;
  named' = units: role: lib.filter (b: roleOf units b == role) (lib.attrNames units.bins);

  # EVERY RELEASE PROGRAM BUT THE ONE BUILT ELSEWHERE.
  bins = lib.filterAttrs (bin: _: roleOf release bin != "apart") release.bins;

  # A PROGRAM THAT PAINTS, under the wrapper that hands it what it is keyed to
  # — the palettes of its own build and, if it speaks, the texts (`--set`,
  # see `keyed` in `nix/rust.nix`). `makeBinaryWrapper` and
  # not `makeWrapper`: the second puts a bash in the closure of what the ISO
  # carries. `--inherit-argv0`, so a process is named after the program and
  # not after `.X-wrapped`.
  wrapped =
    name: bin:
    let
      unit = program bin plainly;
    in
    pkgs.runCommand name
      {
        nativeBuildInputs = [ makeBinaryWrapper ];
        passthru = {
          inherit unit bin;
        };
      }
      ''
        mkdir -p $out/bin
        makeBinaryWrapper ${unit}/bin/${bin} $out/bin/${bin} \
          --inherit-argv0 ${wrapFlags painters.${bin}}
      '';

  # A RELEASE PROGRAM BY ITS NAME, or the reason the plan has none. Every one
  # of these was a `cp` out of crane's workspace; a binary the build did not
  # make was a `cp: cannot stat` minutes into it, and the first word of the
  # refusal said why only where somebody had written a sentence by hand. The
  # plan knows at evaluation which programs exist, so the refusal is an
  # evaluation error, with the sentence.
  program =
    bin: why:
    if release.bins ? ${bin} then
      release.bins.${bin}
    else
      throw "nix/rust/plan.json builds no ${bin}: ${why} (regenerate the plan with `nix run .#rust-plan` after changing a manifest)";

  plainly = "it is not a [[bin]] of any crate of the plan";

  # THE NAMED PROGRAMS: what a module, an app, a VM test or the ISO runs. Each
  # one is its unit, nothing copied — a derivation references what its bytes
  # cite, and a unit's bytes are that program's alone — or, for one that
  # paints, the wrapper over it.
  #
  # Each used to be EXTRACTED from crane's workspace, for one reason said
  # twenty-seven times: a unit that runs as root, outlives the session or runs
  # from Chrome must not carry the renderer, wgpu and the wasm runtime of a
  # derivation that held every program. That reason is the units' shape now —
  # and what keeps a closure narrow is what the program links, which
  # `brain-unit-is-narrow`, `rescue-has-no-toolchain`, `transfers-unit-is-narrow`
  # and their siblings measure.
  named = {
    # The ISO's console installer.
    installer = wrapped "ronoctua-installer" "ronoctua-install";
    # The rescue console, on its VT and from `Ctrl+Alt+Del`.
    rescue = wrapped "ronoctua-rescue" "ronoctua-rescue";
    # The player's daemon: libmpv and ffmpeg-full, never the renderer.
    player = wrapped "ronoctua-player" "ronoctua-player";
    # The VM's formatter (`disks-vm`).
    formatDisk = program "ronoctua-format-disk" plainly;
    # The file manager's privileged helper, run as root.
    filesRoot = program "ronoctua-files-root" plainly;
    # The console's font, where the console can take it: the setup's condition
    # and its step after, in the initrd and in the system.
    consoleFont = program "ronoctua-console-font" plainly;
    # The queue's daemon, a unit that outlives the session. NOT how the shell
    # finds it: that one looks next door (`ronoctua_transfers::client`), in
    # the bundle below.
    transfers = program "ronoctua-transfers" plainly;
    # The kiosk's native-messaging host.
    browser = program "ronoctua-browser" plainly;
    # The software CLI, which the queue and the update run.
    software = program "ronoctua-software" "its [[bin]] requires the features net and draw, and no member of the build enabled them";
    # The studio's daemon: the camera, the encoders, the virtual camera.
    studio = program "ronoctua-studio" plainly;
    # The keys daemon, as root in keyd's place in Independent mode.
    keys = program "ronoctua-keys" plainly;
    # The brain's daemon (`brain-unit-is-narrow`).
    brain = program "ronoctua-brain" plainly;
    # What empties the trash, the cache sweep and the disk's relief.
    emptyTrash = program "ronoctua-empty-trash" plainly;
    cacheSweep = program "ronoctua-cache-sweep" plainly;
    diskRelief = program "ronoctua-disk-relief" plainly;
    # The super index's unit and the video catalogue's scanner.
    index = program "ronoctua-index" plainly;
    videoMetrics = program "ronoctua-video-metrics" plainly;
    # The security centre's root helper, and the queue's VirusTotal child.
    securityRoot = program "ronoctua-security-root" plainly;
    scan = program "ronoctua-scan" plainly;
    # The belongings' CLI and the sealer, which link `age`.
    belongings = program "ronoctua-belongings" "its [[bin]] requires the feature crypt, and no member of the build enabled it";
    seal = program "ronoctua-seal" "its [[bin]] requires the feature cli, and no member of the build enabled it";
    # The machine's life and the person's root half.
    machine = program "ronoctua-machine" "its [[bin]] is in crates/ronoctua-machine, and the build did not reach it";
    person = program "ronoctua-person" "its [[bin]] requires the feature apply, and no member of the build enabled it";
    # The parts' root realizer (`nix/modules/parts.nix`).
    parts = program "ronoctua-parts" "its [[bin]] requires the feature realize, and no member of the build enabled it";
    # The /private vault's root side and the Incus job.
    privateRoot = program "ronoctua-private-root" plainly;
    incusJob = program "ronoctua-incus-job" "its [[bin]] requires the feature cli, and no member of the build enabled it";
    # The LLM CLI, the push to the person's cache, the stick's root writer.
    llm = program "ronoctua-llm" plainly;
    # VS Code's extensions: the record the user path unit runs.
    vscode = program "ronoctua-vscode" plainly;
    cachix = program "ronoctua-cachix" plainly;
    stickWrite = program "ronoctua-stick-write" plainly;
    # NetworkManager's secret agent and the console that asks a Wi-Fi
    # password at boot. They were `${shell}/bin/…`, which put the whole
    # footer (and, through crane's workspace, every program) in the closure
    # of two root units that draw nothing (`shell-bundle-is-only-the-shell`).
    networkConsole = program "ronoctua-network-console" plainly;
    networkAgent = program "ronoctua-network-agent" plainly;
  };

  # THE FORM A MACHINE RUNS A PROGRAM IN: its named attribute (the wrapper,
  # for one that paints), else the unit itself.
  installedFormOf = bin: namedByBin.${bin} or bins.${bin};
  namedByBin = lib.listToAttrs (map (d: lib.nameValuePair d.passthru.bin d) (lib.attrValues named));

  # THE FOOTER AS IT RUNS: the shell and every program its code looks for
  # beside itself, COPIED into one `bin/`.
  #
  # Several crates find a program by asking the filesystem next to
  # `current_exe()` (`beside` in their `[package.metadata.ronoctua]`): the
  # queue's daemon, the brain's, the player's, the studio's, the rescue console,
  # the tag writer, the belongings' CLI and the sealer (the last two looked up
  # by the queue). They have to — a store path baked into
  # one would be a derivation naming its own output. With crane's workspace
  # "beside" was its `bin/`, which held every program. Here every program is a
  # unit of its own, and a symlink would not do: the kernel resolves
  # `/proc/self/exe` through it, so a program reached by a link looks for its
  # neighbours in its own unit's `bin/`, where it is alone (the queue's daemon,
  # started from beside the footer in the nested loop, looks for the
  # belongings' CLI beside ITSELF). So the bundle holds copies, and every one
  # of them finds the others. What is copied is decided by the code that looks:
  # `beside` over every crate whose code is in the shell program.
  #
  # The shell is wrapped with everything read at run time (`atRunTime`), and a
  # painter among its neighbours with what it is keyed to (`painters`: the
  # palettes and, if it speaks, the texts), as each was in crane's bundle.
  #
  # # Why a wrapper and not a variable in each launcher
  #
  # Because there are nine launchers — the user unit, the compositor's binds,
  # the nested loop, six VM tests — and a variable one of them forgets is not a
  # failure: it is a key that stops opening a window, with a line in a journal
  # nobody is reading. Here there is one place, and no launcher has to know
  # anything.
  #
  # `--inherit-argv0` is not decoration: `nix run .#diagnostics` finds the
  # footer with `pgrep -f "bin/ronoctua-shell$"`, and without it every process
  # would show up as `.ronoctua-shell-wrapped` — the diagnostic would go blind
  # exactly when it is the thing being run. `--set-default` and not `--set`:
  # the point of taking the components out of the compilation is being able to
  # swap one without recompiling anything, and `--set` would have the wrapper
  # win over whoever exported one by hand — except for what a binary is KEYED
  # to (`keyed`, the palettes, the texts and the brain's catalogue), which is
  # `--set`.
  bundle =
    {
      name,
      units,
    }:
    let
      shellBin = units.bins.ronoctua-shell;
      besides = lib.unique (lib.concatMap (c: crates.${c}.beside) shellBin.passthru.crates);
      members = [ "ronoctua-shell" ] ++ lib.filter (b: b != "ronoctua-shell") besides;
      missing = lib.filter (b: !(units.bins ? ${b})) members;
      painted = lib.filter (b: lib.elem b painterNames) members;
    in
    if missing != [ ] then
      throw "the shell looks beside itself for ${lib.concatStringsSep ", " missing}, which no crate of the plan builds (`beside` in [package.metadata.ronoctua])"
    else
      pkgs.runCommand name
        {
          nativeBuildInputs = [ makeBinaryWrapper ];
          passthru = {
            inherit members;
            units = lib.genAttrs members (b: units.bins.${b});
          };
        }
        ''
          mkdir -p $out/bin
          ${lib.concatMapStringsSep "\n" (b: ''
            cp --reflink=auto ${units.bins.${b}}/bin/${b} $out/bin/${b}
            chmod u+w $out/bin/${b}
          '') members}
          wrapProgram $out/bin/ronoctua-shell --inherit-argv0 ${wrapFlags atRunTime}
          ${lib.concatMapStringsSep "\n" (
            b: "wrapProgram $out/bin/${b} --inherit-argv0 ${wrapFlags painters.${b}}"
          ) painted}
        '';

  shell = bundle {
    name = "ronoctua-shell";
    units = release;
  };
  # The same over the loop's programs: what `.#dev` runs. The name is the one
  # `nix/dev.nix` finds a loop's leftover processes by, and the mark
  # `the-loop-profile-is-never-installed` looks for.
  loopShell = bundle {
    name = "ronoctua-shell-loop";
    units = loop;
  };

  # THE BENCHMARKS, for `.#bench`: the four `bench` programs of the release
  # graph — measuring a build nobody installs would answer about the wrong
  # binary.
  benches = pkgs.symlinkJoin {
    name = "ronoctua-benches";
    paths = map (b: release.bins.${b}) (named' release "bench");
    passthru.members = named' release "bench";
  };

  # WHAT THE ISO CARRIES SO THE TARGET COMPILES NOTHING OF OURS: the shell
  # bundle and every `ship` program in the form a machine runs it — a module
  # runs the brain's or the queue's own unit, and the footer the copies in its
  # bundle, so both are here.
  shipped =
    let
      # A painter shipped bare would draw every colour wrong, or refuse: one
      # that ships has its wrapper among the named programs.
      bare = lib.filter (
        b: lib.elem b painterNames && !(namedByBin ? ${b} && namedByBin.${b}.passthru ? unit)
      ) (named' release "ship");
    in
    if bare != [ ] then
      throw "${lib.concatStringsSep ", " bare} paint and ship, and no named program wraps them with the palettes (nix/rust/outputs.nix)"
    else
      [ shell ]
      # A program the bundle holds, and nothing runs on its own (the shell
      # itself, the tag writer), arrives in the bundle's copy: its unit would
      # be a second copy nothing on the machine references.
      ++ map installedFormOf (
        lib.filter (b: namedByBin ? ${b} || !(lib.elem b shell.passthru.members)) (named' release "ship")
      );
in
named
// {
  inherit
    bins
    shell
    loopShell
    benches
    shipped
    ;
  # The programs that paint, by name (for checks).
  painters = painterNames;
  # Which programs have which role, by name (for checks).
  roles = lib.mapAttrs (bin: _: roleOf release bin) release.bins;
}
