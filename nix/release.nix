# THE RELEASE, and the three programs around it.
#
#   nix run .#release        publishes the distro: the `release` branch every
#                            machine follows, and the ISO
#   nix run .#fetch-iso      downloads a release's ISO, checks it, joins it
#   nix run .#adopt-machine  moves a machine of the old layout to its own flake
#   nix run .#apply-here     builds this machine with the checkout in place of
#                            the distro input, and switches to it
#
# What the release DECIDES — how the ISO travels, the notes — is
# `nix/scripts/release.py`, where a check can run it on a fixture; this file
# is the order things happen in, and the programs they happen through. The
# distro has NO binary cache: its ISO carries its seed, and a person's own
# cache is theirs (`nix/modules/binary-cache.nix`). Why each step is where it
# is:
#
# docs/ARCHITECTURE.md, section "A versão, o cache binário da pessoa e as máquinas que seguem a `release`"
{
  lib,
  writeShellApplication,
  python3,
  git,
  gh,
  nix,
  curl,
  cacert,
  coreutils,
  gnugrep,
  findutils,
  jq,
  util-linux,
  # `nix run .#gate`'s program: a release is what passed it.
  gate,
  # The machine directory's one name (`nix/machine.nix`).
  machineDir,
  # The reference machine's `config.ronoctua.machine` (`nix/modules/machine.nix`):
  # `ronoctua-machine` wrapped with the environment every unit gives it, and
  # where its record lives. Read only by the two programs that change this
  # machine, so evaluating the release alone does not evaluate a system.
  machine,
}:

let
  # THE REPOSITORY A RELEASE LANDS IN, and the branch machines follow. The
  # stick (`ronoctua-stick`'s `REPOSITORY`) reads releases from the same
  # repository and the machine template (`templates/machine/flake.nix`)
  # follows the same branch.
  repository = "ronoctua/ronoctua-distro";
  branch = "release";

  releasePy = "${python3}/bin/python3 ${./scripts/release.py}";

  # A word for the shell, always in quotes: `lib.escapeShellArg` leaves a
  # plain one bare, and shellcheck reads `booted=booted-system` as
  # arithmetic.
  quoted = s: "'${lib.replaceStrings [ "'" ] [ "'\\''" ] s}'";

  # THE UPDATE'S OWN LOCK (`updateLock` in `nix/modules/machine.nix`): the
  # root half of the update holds it for its whole run, and every apply that
  # does not run inside the update takes it first (`applyCommand`). Spelled
  # from the same folder, and held to that file's spelling at evaluation: a
  # rename there that did not reach here would be two locks, and no
  # exclusion.
  updateLock =
    let
      lock = "${machine.runDir}/system.lock";
    in
    assert lib.assertMsg (lib.hasInfix " ${lock} " machine.applyCommand)
      "nix/release.nix: the update's lock is no longer ${lock} (nix/modules/machine.nix, applyCommand)";
    lock;

  # THE DISTRO'S PINS THAT FELL BEHIND, said before a release publishes
  # (`distro-pins.py --stale`): read-only questions to GitHub (and F-Droid)
  # about what each pin follows — nothing is downloaded or written, and no
  # pin moves: a pin is what the gate tested, and moving one is the owner's
  # commit (`nix run .#distro-pins`). Its one argument is the checkout.
  stalePins = writeShellApplication {
    name = "ronoctua-stale-pins";
    runtimeInputs = [ nix ];
    text = ''
      export SSL_CERT_FILE=${cacert}/etc/ssl/certs/ca-bundle.crt
      exec ${python3}/bin/python3 ${./scripts/distro-pins.py} --stale --root "$1"
    '';
  };

  # THE RELEASE, as a function of the programs it drives: the check
  # (`release-refuses-dirty-tree`) builds it with programs that only write
  # down that they were called, and the refusals it proves are this same
  # text.
  releaseWith =
    {
      gh,
      nix,
      gate,
      pins,
      # How long the pins may take to be compared, all of them: a network
      # that does not answer must not hold the release.
      pinsLimitS ? 120,
    }:
    writeShellApplication {
      name = "ronoctua-release";
      runtimeInputs = [
        git
        coreutils
        gnugrep
        findutils
      ];
      text = ''
        repository=${lib.escapeShellArg repository}
        branch=${lib.escapeShellArg branch}
        gh=${gh}
        nix=${nix}
        gate=${gate}
        pins=${pins}

        usage() {
          cat <<'EOF'
        usage: nix run .#release [-- OPTIONS]

        Publishes the distro from this checkout, which must be `main`, clean and
        equal to `origin/main`: runs the gate, builds the two reference systems and
        the ISO (which carries its seed), moves `release` forward (only forward), and
        publishes the ISO as a GitHub release with SHA256SUMS. It never makes a commit.
        First it says which of the distro's pins are behind their latest release
        (`nix run .#distro-pins` moves them); it never moves one, and goes on.

          --dry-run          check everything that costs nothing, say what would happen,
                             build and publish nothing
          --sign             also put the checksums in a signed tag (git tag -s: the
                             signing key is asked for)
          --rollback-to REV  move `release` back to REV, the commit of an earlier
                             published release, and mark that release GitHub's latest
                             again; nothing is built: the one way `release` goes backwards
          -h, --help         this text
        EOF
        }

        # A refusal is a sentence, and what to do about it on the lines below.
        refuse() {
          echo "refused: $1" >&2
          shift
          for line in "$@"; do
            echo "  $line" >&2
          done
          exit 1
        }
        step() {
          echo
          echo "== $* =="
        }

        dry=0 sign=0 rollback=""
        while [ $# -gt 0 ]; do
          case "$1" in
            --dry-run) dry=1 ;;
            --sign) sign=1 ;;
            --rollback-to)
              [ $# -ge 2 ] || refuse "--rollback-to needs a revision"
              rollback=$2
              shift
              ;;
            -h | --help)
              usage
              exit 0
              ;;
            *)
              usage >&2
              exit 2
              ;;
          esac
          shift
        done

        # THE CHECKOUT, and the repository it pushes to.
        top=$(git rev-parse --show-toplevel 2>/dev/null) ||
          refuse "this is not a git checkout" "run it from the distro's checkout: nix run .#release"
        cd "$top"
        [ -f flake.nix ] || refuse "$top has no flake.nix: it is not the distro's checkout"
        # By the URL as written in the configuration, not as git rewrites it:
        # the branch and the GitHub release must land in the same repository.
        origin=$(git config --get remote.origin.url || true)
        pattern="^(git@github\.com:|ssh://git@github\.com/|https://github\.com/)$repository(\.git)?/?$"
        if ! [[ "$origin" =~ $pattern ]]; then
          refuse "origin is '$origin', not github.com/$repository" \
            "the $branch branch and the GitHub release have to land in the same repository"
        fi
        echo "reading origin ($origin)"
        git fetch --quiet --prune origin || refuse "origin did not answer" "nothing was changed"
        released=$(git rev-parse --verify --quiet "refs/remotes/origin/$branch" || true)

        # The commit a tag of origin names (`tags` holds `git ls-remote --tags`):
        # an annotated tag — the signed one `--sign` makes — by the commit it
        # points at (`^{}`), a lightweight one — the one GitHub makes when it
        # publishes a release whose tag is not there — by itself.
        tag_commit() {
          local sha ref plain="" peeled=""
          while read -r sha ref; do
            case "$ref" in
              "refs/tags/$1^{}") peeled=$sha ;;
              "refs/tags/$1") plain=$sha ;;
            esac
          done <<<"$tags"
          echo "''${peeled:-$plain}"
        }

        # ROLLBACK: the one way the branch moves backwards, and all it does.
        if [ -n "$rollback" ]; then
          rev=$(git rev-parse --verify --quiet "$rollback^{commit}") ||
            refuse "$rollback is not a commit of this checkout"
          [ -n "$released" ] || refuse "origin has no $branch branch: there is nothing to roll back"
          [ "$rev" != "$released" ] || refuse "$branch is already at $rev"
          git merge-base --is-ancestor "$rev" "$released" ||
            refuse "$rev is not behind $branch: a rollback only goes back to what was released before" \
              "to publish something new, run nix run .#release from main"
          # ONLY TO A COMMIT THAT WAS RELEASED. `release` jumps forward many
          # commits at a time, so most of what lies behind it never passed the
          # gate: every machine would build an untested commit. What was
          # released is what GitHub published, by the commit its tag names.
          "$gh" auth status --hostname github.com >/dev/null 2>&1 ||
            refuse "gh is not logged in to github.com" "gh auth login"
          published=$("$gh" release list --repo "$repository" --exclude-drafts --limit 1000 \
            --json tagName --jq '.[].tagName') ||
            refuse "GitHub did not list the releases" "nothing was changed"
          tags=$(git ls-remote --tags origin 'refs/tags/v*') ||
            refuse "origin did not list its tags" "nothing was changed"
          back=""
          while read -r t; do
            if [ -n "$t" ] && [ "$(tag_commit "$t")" = "$rev" ]; then
              back=$t
              break
            fi
          done <<<"$published"
          [ -n "$back" ] ||
            refuse "$rev was never published as a release: a rollback goes back to one that passed the gate" \
              "— the published releases are: $(tr '\n' ' ' <<<"$published")"
          echo "$branch: $released -> $rev (back, to the release $back)"
          echo "machines move to it at their next update, and $back becomes GitHub's latest release again:"
          echo "the stick and fetch-iso hand out its ISO. Nothing is built."
          if [ "$dry" = 1 ]; then
            echo "dry run: nothing was pushed"
            exit 0
          fi
          git push --force-with-lease="refs/heads/$branch:$released" origin "$rev:refs/heads/$branch"
          # THE LATEST FOLLOWS THE BRANCH: the stick and `.#fetch-iso` ask
          # GitHub for its latest release, and it must not stay the one the
          # owner just retreated from.
          "$gh" release edit "$back" --repo "$repository" --latest ||
            refuse "$branch is at $rev, and GitHub did not mark $back as its latest release" \
              "gh release edit $back --repo $repository --latest"
          echo "$branch is at $rev; $back is the latest release"
          exit 0
        fi

        # WHAT IS RELEASED IS WHAT ORIGIN HAS, AND NOTHING ELSE: `main`, clean,
        # neither ahead nor behind. The ISO carries this tree, and the commit it
        # names has to be one anybody can fetch.
        here=$(git symbolic-ref --quiet --short HEAD) || refuse "HEAD is detached" "check out main first"
        [ "$here" = main ] || refuse "this checkout is on $here, not main"
        dirty=$(git status --porcelain=v1 --untracked-files=normal)
        if [ -n "$dirty" ]; then
          refuse "the tree is not clean: a release is a commit, not a working tree" \
            "$(printf '%s\n' "$dirty" | head -5)" "commit or remove what is there, and push it"
        fi
        rev=$(git rev-parse HEAD)
        theirs=$(git rev-parse --verify --quiet refs/remotes/origin/main) || refuse "origin has no main branch"
        if [ "$rev" != "$theirs" ]; then
          if git merge-base --is-ancestor "$rev" "$theirs"; then
            refuse "main is behind origin/main" "pull first: the release is what origin has"
          elif git merge-base --is-ancestor "$theirs" "$rev"; then
            refuse "main is ahead of origin/main" "push main first: the release is what origin has"
          else
            refuse "main and origin/main have diverged"
          fi
        fi
        # FORWARD ONLY. A release that is not behind main would need a force
        # push, and a force push of what every machine follows is a rollback:
        # it has its own flag.
        if [ -n "$released" ] && ! git merge-base --is-ancestor "$released" "$rev"; then
          refuse "$branch ($released) is not behind main: moving it would not be a fast-forward" \
            "nix run .#release -- --rollback-to <rev> is the one way back"
        fi

        short=$(git rev-parse --short=7 "$rev")
        day=$(TZ=UTC git show -s --format=%cd --date=format-local:%Y.%m.%d "$rev")
        tag="v$day-$short"

        # GITHUB, asked before anything costs time.
        "$gh" auth status --hostname github.com >/dev/null 2>&1 ||
          refuse "gh is not logged in to github.com" "gh auth login"
        draft=""
        if state=$("$gh" release view "$tag" --repo "$repository" --json isDraft --jq .isDraft 2>/dev/null); then
          if [ "$state" != true ]; then
            if [ "$released" = "$rev" ]; then
              refuse "the release $tag is already published" "this commit was released, and $branch is at it: there is nothing to do"
            fi
            refuse "the release $tag is already published, and $branch was rolled back from it to ''${released:-nothing}" \
              "publishing it again would not undo the rollback's reason: release a commit after it"
          fi
          draft=yes
        fi
        if [ "$sign" = 1 ]; then
          git config --get user.signingkey >/dev/null ||
            refuse "--sign needs a signing key (git config user.signingkey)"
          if there=$(git rev-parse --verify --quiet "refs/tags/$tag^{commit}"); then
            [ "$there" = "$rev" ] || refuse "the tag $tag already names another commit ($there)"
          fi
        fi

        echo
        echo "release $tag"
        echo "  commit   $rev"
        echo "  $branch  ''${released:-(none yet)} -> $rev"
        if [ -n "$draft" ]; then
          echo "  github   the draft $tag exists: its assets are uploaded again"
        fi
        if [ "$sign" = 1 ]; then
          echo "  tag      signed, carrying the checksums"
        fi

        # THE PINS THAT FELL BEHIND, said before anything costs time: the
        # owner may stop here, move them (`nix run .#distro-pins`, a commit
        # and a push) and release again. Never a refusal and never a move —
        # a pin is what the gate tests —, and a question the network did not
        # answer is said and the release goes on.
        #
        # THE LOGGED-IN `gh`'s TOKEN goes in the environment of that one call —
        # never an argument, never printed —: GitHub answers sixty questions
        # an hour without one, and a release is asked again and again.
        echo
        echo "the distro's pins, against their latest releases:"
        token=$("$gh" auth token 2>/dev/null || true)
        limit=${toString pinsLimitS}
        status=0
        if [ -n "$token" ]; then
          said=$(GH_TOKEN="$token" timeout "$limit" "$pins" "$top" 2>&1) || status=$?
        else
          said=$(timeout "$limit" "$pins" "$top" 2>&1) || status=$?
        fi
        unset token
        while IFS= read -r line; do
          echo "  $line"
        done <<<"$said"
        case "$status" in
          0) ;;
          124) echo "  (no answer within $limit s: they were not compared)" ;;
          *) echo "  (they could not be compared: see above)" ;;
        esac
        echo "  this release goes on with the pins it has"
        if [ "$dry" = 1 ]; then
          echo
          echo "dry run: nothing was built, pushed or published"
          exit 0
        fi

        work="''${XDG_CACHE_HOME:-$HOME/.cache}/ronoctua-distro/release/$tag"
        rm -rf "$work"
        mkdir -p "$work"
        flake="git+file://$top?rev=$rev"

        # 1. THE GATE. A release is what passed it, on this tree.
        step "1. the gate"
        "$gate" || refuse "the gate failed" "nothing was built for the release, pushed or published"
        # The gate takes long, and the tree is somebody's: what it tested must
        # still be the commit about to be published.
        now=$(git rev-parse HEAD)
        if [ "$now" != "$rev" ] || [ -n "$(git status --porcelain=v1 --untracked-files=normal)" ]; then
          refuse "the checkout changed while the gate ran" "nothing was published; run it again"
        fi

        # 2. WHAT IS PUBLISHED: the two reference systems and the ISO — which
        # carries its seed —, from the commit itself (not the folder), much of
        # it built by the gate already.
        step "2. the systems and the ISO"
        "$nix" build --no-link --print-out-paths --max-jobs 1 --option cores 4 \
          "$flake#nixosConfigurations.noctua.config.system.build.toplevel" \
          "$flake#nixosConfigurations.minimal.config.system.build.toplevel" \
          "$flake#iso" >"$work/built"
        mapfile -t built <"$work/built"
        [ "''${#built[@]}" = 3 ] || refuse "the build answered ''${#built[@]} paths instead of three"
        iso=$(find "''${built[2]}/iso" -name '*.iso' | head -1)
        [ -n "$iso" ] || refuse "the ISO's derivation holds no .iso"

        # 3. THE ASSETS AND THE NOTES.
        step "3. the ISO as assets"
        ${releasePy} pack "$iso" "$work/assets" >"$work/upload"
        mapfile -t upload <"$work/upload"
        cat "$work/assets/SHA256SUMS"
        signed=()
        if [ "$sign" = 1 ]; then
          signed=(--signed)
        fi
        ${releasePy} notes --repository "$repository" --tag "$tag" --rev "$rev" --date "$day" --assets "$work/assets" \
          "''${signed[@]}" >"$work/notes.md"

        # 4. THE SIGNED TAG, when asked: the checksums under the owner's key,
        # never in a commit (the ISO carries the tree; a commit of its sums
        # would be a tree the ISO does not carry). Made HERE and kept in this
        # checkout: it names a release, and it reaches origin only once the
        # branch has moved (step 7) — a public tag of a release that never
        # came would be a promise nobody keeps.
        published_with=(--latest)
        kept=""
        if [ "$sign" = 1 ]; then
          step "4. the signed tag (in this checkout until $branch moves)"
          if ! git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null; then
            { echo "ronoctua-distro $tag"; echo; cat "$work/assets/SHA256SUMS"; } >"$work/tag-message"
            git tag --sign --file "$work/tag-message" "$tag" "$rev"
          fi
          published_with+=(--verify-tag)
          kept=", and the signed tag $tag is only in this checkout"
        fi

        # 5. A DRAFT FIRST: nobody sees a release whose assets are still
        # uploading (the stick reads only published ones). A draft makes no
        # tag; GitHub makes one at `--target` when it is published, unless
        # the signed one is already there.
        step "5. the draft on GitHub"
        title="ronoctua-distro $day ($short)"
        if [ -n "$draft" ]; then
          "$gh" release upload "$tag" --repo "$repository" --clobber "''${upload[@]}"
          "$gh" release edit "$tag" --repo "$repository" --title "$title" --notes-file "$work/notes.md"
        else
          "$gh" release create "$tag" --repo "$repository" --draft --target "$rev" \
            --title "$title" --notes-file "$work/notes.md" "''${upload[@]}"
        fi

        # 6. THE BRANCH, FORWARD ONLY: no force, so git itself refuses
        # anything but a fast-forward.
        step "6. $branch -> $rev"
        git push origin "$rev:refs/heads/$branch" ||
          refuse "$branch did not move" \
            "the draft $tag stays unpublished$kept, and nothing else changed"

        # 7. THE SIGNED TAG REACHES ORIGIN, now that the release it names is
        # where machines look.
        if [ "$sign" = 1 ]; then
          step "7. the signed tag -> origin"
          git push origin "refs/tags/$tag" ||
            refuse "$branch is at $rev, and the signed tag $tag did not reach origin" \
              "the draft stays unpublished: run nix run .#release -- --sign again, which pushes the tag and publishes"
        fi

        # 8. PUBLISHED, and the latest: the stick and fetch-iso ask GitHub for
        # the latest release, and a fonts release must never be it.
        step "8. published"
        "$gh" release edit "$tag" --repo "$repository" --draft=false "''${published_with[@]}"
        echo "released $tag: $branch is at $rev"
      '';
    };
in
rec {
  inherit releaseWith stalePins;

  release = releaseWith {
    gh = lib.getExe gh;
    nix = lib.getExe' nix "nix";
    inherit gate;
    pins = lib.getExe stalePins;
  };

  # ANYBODY'S, not only the owner's: no account, nothing to install but Nix.
  fetchIso = writeShellApplication {
    name = "ronoctua-fetch-iso";
    runtimeInputs = [ curl ];
    text = ''
      case "''${1:-}" in
        -h | --help)
          cat <<'EOF'
      usage: nix run .#fetch-iso [-- [--tag TAG] [--from FOLDER] [--into FOLDER]]

      Downloads the ISO of the latest release (or of TAG), checks every part and the
      whole against the release's SHA256SUMS, and joins it. A download that stopped
      is resumed by running it again. --from reads the assets from a folder instead
      (what `.#release` packs, before it is published).
      EOF
          exit 0
          ;;
      esac
      export SSL_CERT_FILE=${cacert}/etc/ssl/certs/ca-bundle.crt
      exec ${releasePy} fetch --repository ${lib.escapeShellArg repository} "$@"
    '';
  };

  # A MACHINE OF THE OLD LAYOUT — a copy of the whole distro in the machine
  # directory — runs an update that does not know how to migrate. This is the
  # migration from a checkout: `ronoctua-machine migrate` (the new flake is
  # built beside the old one, and only a flake that builds replaces it), then
  # `apply`, so the machine runs a system built from its own flake — the one
  # whose update knows the way from there.
  #
  # A function of the tool, so the check (`machine-apps-stop-before-the-switch`)
  # runs it around a stand-in `ronoctua-machine` and a stand-in `sudo`.
  adoptMachine = adoptMachineWith { tool = "${machine.package}/bin/ronoctua-machine"; };
  adoptMachineWith =
    { tool }:
    writeShellApplication {
      name = "ronoctua-adopt-machine";
      runtimeInputs = [ coreutils ];
      text = ''
        distro=""
        while [ $# -gt 0 ]; do
          case "$1" in
            --distro)
              [ $# -ge 2 ] || { echo "--distro needs a flake address" >&2; exit 2; }
              distro=$2
              shift
              ;;
            -h | --help)
              cat <<'EOF'
        usage: nix run .#adopt-machine [-- --distro FLAKE]

        Moves this machine from the old layout (a copy of the distro in ${machineDir})
        to a flake of its own that follows the distro's `release` branch, and switches
        to the system it builds. The old layout stays as it is unless the new flake
        builds, and is kept aside afterwards. Asks for sudo.

          --distro FLAKE   the distro input to write instead of the release branch
                           (while there is no release yet: github:ronoctua/ronoctua-distro/main)
        EOF
              exit 0
              ;;
            *)
              echo "unknown argument: $1 (see --help)" >&2
              exit 2
              ;;
          esac
          shift
        done
        if [ "$(id -u)" = 0 ]; then
          echo "refused: run it as the machine's person, not as root; it asks for sudo itself" >&2
          exit 1
        fi
        # THE ATTRIBUTE IS THE PERSON'S NAME (`ronoctua.machine.attr`): a wrong
        # one only fails the build of the new flake, and the old layout stays.
        person=$(id -un)
        tool=${tool}
        told=(RONOCTUA_MACHINE_ATTR="$person")
        if [ -n "$distro" ]; then
          told+=(RONOCTUA_MACHINE_DISTRO="$distro")
        fi
        # The last line on standard output is the verdict, and the exit status
        # says whether it is one; every step goes to standard error, and so to
        # the terminal. ONLY A VERDICT THAT SAYS IT FINISHED goes on: a sudo
        # that was not let in, or a migration killed before it spoke, says
        # nothing — and nothing is not "migrated".
        run() {
          local out status=0
          out=$(sudo env "''${told[@]}" "$tool" "$@") || status=$?
          verdict=$(printf '%s\n' "$out" | tail -n 1)
          return "$status"
        }
        echo "migrating ${machineDir} (the new flake is built before anything moves)"
        status=0
        run migrate || status=$?
        case "$status:$verdict" in
          "0:migrated"*) ;;
          *":refused nothing-to-migrate")
            echo "${machineDir} is already the machine's own flake: nothing to adopt"
            exit 0
            ;;
          *":refused "*)
            echo "refused: the migration stopped ($verdict); the old layout is as it was" >&2
            if [ -z "$distro" ]; then
              echo "  the new flake follows github:ronoctua/ronoctua-distro/release; while that branch" >&2
              echo "  does not exist, name the distro: nix run .#adopt-machine -- --distro <flake>" >&2
            fi
            exit 1
            ;;
          *)
            echo "refused: the migration did not say it finished (exit $status, '$verdict'), so nothing" >&2
            echo "  is switched; look at what it said above, and at ${machineDir}, before running it again" >&2
            exit 1
            ;;
        esac
        echo "$verdict"
        echo "switching to the system the new flake builds"
        status=0
        run apply || status=$?
        case "$status:$verdict" in
          "0:switched"*) ;;
          *)
            echo "refused: the apply did not say it switched (exit $status, '$verdict'): see what it said" >&2
            echo "  above; the machine's own flake is in place, and its next update applies it" >&2
            exit 1
            ;;
        esac
        echo "$verdict"
      '';
    };

  # THE OWNER'S CHECKOUT ON THE OWNER'S MACHINE, without a release: the
  # machine's own flake built with this checkout in place of the `ronoctua`
  # input. `--override-input` and `--no-write-lock-file`: the machine
  # directory is not touched, so the next update builds the release again,
  # unless the machine is held. Built as the person (the checkout is theirs);
  # only the switch is root's (`applyHereSwitch`).
  applyHere = writeShellApplication {
    name = "ronoctua-apply-here";
    runtimeInputs = [
      git
      coreutils
      findutils
      jq
    ];
    text = ''
      attr=$(id -un) dry=0
      while [ $# -gt 0 ]; do
        case "$1" in
          --dry-run) dry=1 ;;
          --attr)
            [ $# -ge 2 ] || { echo "--attr needs a name" >&2; exit 2; }
            attr=$2
            shift
            ;;
          -h | --help)
            cat <<'EOF'
      usage: nix run .#apply-here [-- --dry-run] [-- --attr NAME]

      Builds this machine (${machineDir}) with this checkout in place of the distro
      input, and switches to it. The machine directory and its lock are not
      touched: the next update goes back to the release. Asks for sudo to switch,
      and switches only while the update is not running, no boot trial waits, and
      the last good system stays in the boot menu.

        --dry-run    say what would be built and downloaded, and stop
        --attr NAME  the machine flake's attribute (the person's name; default: you)
      EOF
            exit 0
            ;;
          *)
            echo "unknown argument: $1 (see --help)" >&2
            exit 2
            ;;
        esac
        shift
      done
      refuse() {
        echo "refused: $1" >&2
        shift
        for line in "$@"; do echo "  $line" >&2; done
        exit 1
      }
      top=$(git rev-parse --show-toplevel 2>/dev/null) ||
        refuse "this is not a git checkout" "run it from the distro's checkout: nix run .#apply-here"
      [ -f "$top/flake.nix" ] || refuse "$top has no flake.nix"
      dir=${machineDir}
      [ -f "$dir/flake.nix" ] || refuse "$dir has no flake.nix: this machine was not installed with the distro"
      # The old layout's mark, the one `ronoctua-machine migrate` reads.
      [ ! -e "$dir/nix/modules/base.nix" ] ||
        refuse "$dir is still the old layout (a copy of the distro)" "nix run .#adopt-machine first"
      # A BOOT TRIAL WAITING TO BE JUDGED, asked now so a build of minutes is
      # not spent on a switch that would be refused — and asked AGAIN by the
      # switch, under the update's lock, because minutes are long enough for
      # the update to arm one.
      state=${machine.stateDir}/state.json
      if [ -e "$state" ]; then
        outcome=$(jq -r '.trial.outcome // empty' "$state") ||
          refuse "$state cannot be read" "this cannot tell whether a boot trial is waiting"
        if [ "$outcome" = pending ]; then
          refuse "a boot trial is waiting to be judged" "reboot first, and let the boot be blessed"
        fi
      fi
      words=(
        --no-write-lock-file
        --override-input ronoctua "git+file://$top"
        "path:$dir#nixosConfigurations.$attr.config.system.build.toplevel"
      )
      # BUILT AS THE PERSON, whose checkout it is — unless the machine
      # directory holds a file only root reads (`path:` copies all of it),
      # and then as root, said.
      as=()
      unreadable=$(find "$dir" ! -readable -print -quit 2>/dev/null || true)
      if [ -n "$unreadable" ]; then
        echo "$unreadable is readable only by root: the machine is built as root" >&2
        as=(sudo)
      fi
      if [ "$dry" = 1 ]; then
        exec "''${as[@]}" ${lib.getExe' nix "nix"} build --dry-run "''${words[@]}"
      fi
      link="''${XDG_STATE_HOME:-$HOME/.local/state}/ronoctua-distro/apply-here"
      mkdir -p "$link"
      "''${as[@]}" ${lib.getExe' nix "nix"} build -L --out-link "$link/system" "''${words[@]}"
      system=$(readlink -f "$link/system")
      echo "built $system"
      status=0
      sudo ${applyHereSwitch}/bin/ronoctua-apply-here-switch "$system" "$top" || status=$?
      rm -f "$link/system"
      exit "$status"
    '';
  };

  # THE ROOT HALF OF `.#apply-here`: the profile and the switch, UNDER THE
  # UPDATE'S LOCK, the way every apply outside the update runs
  # (`applyCommand`, `nix/modules/machine.nix`) — busy is refused, never
  # waited for. Under it, and only under it, the record is read again: an
  # update that ran while the checkout was building may have armed a trial
  # (its system named by the EFI variables, the profile then naming this
  # one: the next boot would bless the update's and this switch would not
  # stick). And the boot menu keeps its way back: a new generation that
  # would push the last good system out of the menu is refused, as
  # `ronoctua-machine apply` refuses it (`last-good-would-fall-out`) — no
  # trial is armed here, so the menu is the only way back there is.
  applyHereSwitch = applyHereSwitchWith {
    lock = updateLock;
    inherit (machine) stateDir;
    profiles = "/nix/var/nix/profiles";
    booted = "/run/booted-system";
    menuLimit = machine.environment.RONOCTUA_MACHINE_MENU_LIMIT;
    nixEnv = lib.getExe' nix "nix-env";
  };

  # A function of where it looks, so the check
  # (`machine-apps-stop-before-the-switch`) runs it on a planted profile, record
  # and lock.
  applyHereSwitchWith =
    {
      lock,
      stateDir,
      profiles,
      booted,
      menuLimit,
      nixEnv,
    }:
    writeShellApplication {
      name = "ronoctua-apply-here-switch";
      runtimeInputs = [
        coreutils
        util-linux
        jq
      ];
      text = ''
        refuse() {
          echo "refused: $1" >&2
          shift
          for line in "$@"; do echo "  $line" >&2; done
          exit 1
        }
        [ $# = 2 ] || refuse "usage: ronoctua-apply-here-switch <system> <checkout>"
        system=$(readlink -f "$1")
        top=$2
        [ -x "$system/bin/switch-to-configuration" ] || refuse "$system is not a NixOS system"

        # THE UPDATE'S LOCK, not waited for. Its folder is the update's
        # runtime folder, which exists only once an update ran this boot:
        # made as the update's unit makes it. The file is made private.
        umask 077
        lock=${quoted lock}
        folder=$(dirname "$lock")
        if [ ! -d "$folder" ]; then
          install -d -m 0755 "$folder"
        fi
        exec 8>>"$lock"
        flock --nonblock 8 ||
          refuse "the update, or another apply, is running" \
            "let it finish and run nix run .#apply-here again: what was built stays in the store"

        # THE RECORD, read again under the lock. One that cannot be read is
        # not "no trial".
        state=${quoted "${stateDir}/state.json"}
        last_good=""
        if [ -e "$state" ]; then
          outcome=$(jq -r '.trial.outcome // empty' "$state") ||
            refuse "$state cannot be read" "this cannot tell whether a boot trial is waiting"
          if [ "$outcome" = pending ]; then
            refuse "a boot trial is waiting to be judged (the update armed one while this was building)" \
              "reboot first, and let the boot be blessed"
          fi
          last_good=$(jq -r '."last-good".toplevel // empty' "$state")
        fi
        # Before any bless, the system that booted this machine is the last
        # good one — `ronoctua-machine apply`'s reading.
        booted=${quoted booted}
        if [ -z "$last_good" ] && [ -e "$booted" ]; then
          last_good=$(readlink -f "$booted")
        fi

        # THE WAY BACK STAYS IN THE MENU.
        profile=${quoted "${profiles}/system"}
        limit=${quoted menuLimit}
        if [ -n "$last_good" ] &&
          ! ${releasePy} menu-room --profile "$profile" --toplevel "$system" --last-good "$last_good" --limit "$limit"; then
          refuse "one more generation would push the last good system out of the boot menu, which lists $limit" \
            "($last_good) — and no trial is armed here, so the menu is the only way back." \
            "Let the update apply the release once (its boot, blessed, is a newer last good system), then run this again."
        fi

        # THE PROFILE, then the switch. 4 is "switched, and some units it
        # started failed": the new system runs.
        ${quoted nixEnv} --profile "$profile" --set "$system" ||
          refuse "the system profile did not take $system" "nothing was switched"
        status=0
        "$system/bin/switch-to-configuration" switch || status=$?
        case "$status" in
          0) echo "switched to the checkout ($top)" ;;
          4) echo "switched to the checkout ($top), and some units failed: see systemctl --failed" ;;
          *) refuse "the switch failed ($status)" "the profile names the new system; the running one is unchanged" ;;
        esac
        exit "$status"
      '';
    };
}
