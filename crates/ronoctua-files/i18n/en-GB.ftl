# The texts `ronoctua-files` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# ===================================================================
# The file manager — LEADER . (footer) and the window
#
# The permission explanations are why this file grew its first SELECTOR. On a
# directory, `x` is not "execute", it is "may enter", and `r` without `x` lists
# names one cannot reach — saying "execute" there says something false. Which
# of the two sentences applies is a decision of the LANGUAGE, so it lives here
# and not in the Rust.
# ===================================================================

# What the first letter of the column says.
files-kind-directory = A directory: a place, not a thing.
files-kind-file = An ordinary file.
files-kind-symlink = A shortcut: it points somewhere else, and deleting it does not delete the target.
files-kind-fifo = A named pipe: one program writes, another reads.
files-kind-socket = A socket: two programs talking through it.
files-kind-block = A block device — a disk, or part of one.
files-kind-char = A character device.
files-perm-owner-read = { $kind ->
        [dir] { $set ->
                [yes] { $who } can see the names inside this directory
               *[no] { $who } can NOT see the names inside this directory
            }
       *[file] { $set ->
                [yes] { $who } can read this file
               *[no] { $who } can NOT read this file
            }
    }
files-perm-owner-write = { $kind ->
        [dir] { $set ->
                [yes] { $who } can create and delete things inside this directory
               *[no] { $who } can NOT create or delete anything in this directory
            }
       *[file] { $set ->
                [yes] { $who } can change this file
               *[no] { $who } can NOT change this file
            }
    }
files-perm-owner-execute = { $kind ->
        [dir] { $set ->
                [yes] { $who } can ENTER this directory
               *[no] { $who } can NOT enter this directory, nor anything inside it
            }
       *[file] { $set ->
                [yes] { $who } can run this file as a program
               *[no] { $who } cannot run this file
            }
    }
files-perm-group-read = { $kind ->
        [dir] { $set ->
                [yes] the group { $who } can see the names inside this directory
               *[no] the group { $who } can NOT see the names inside this directory
            }
       *[file] { $set ->
                [yes] the group { $who } can read this file
               *[no] the group { $who } can NOT read this file
            }
    }
files-perm-group-write = { $kind ->
        [dir] { $set ->
                [yes] the group { $who } can create and delete things inside this directory
               *[no] the group { $who } can NOT create or delete anything in this directory
            }
       *[file] { $set ->
                [yes] the group { $who } can change this file
               *[no] the group { $who } can NOT change this file
            }
    }
files-perm-group-execute = { $kind ->
        [dir] { $set ->
                [yes] the group { $who } can ENTER this directory
               *[no] the group { $who } can NOT enter this directory
            }
       *[file] { $set ->
                [yes] the group { $who } can run this file as a program
               *[no] the group { $who } cannot run this file
            }
    }
files-perm-other-read = { $kind ->
        [dir] { $set ->
                [yes] anybody on this machine can see the names in this directory
               *[no] nobody else can see the names in this directory
            }
       *[file] { $set ->
                [yes] anybody on this machine can read this file
               *[no] nobody else can read this file
            }
    }
files-perm-other-write = { $kind ->
        [dir] { $set ->
                [yes] anybody on this machine can create and delete things here
               *[no] nobody else can create or delete anything here
            }
       *[file] { $set ->
                [yes] anybody on this machine can change this file
               *[no] nobody else can change this file
            }
    }
files-perm-other-execute = { $kind ->
        [dir] { $set ->
                [yes] anybody on this machine can enter this directory
               *[no] nobody else can enter this directory
            }
       *[file] { $set ->
                [yes] anybody on this machine can run this file
               *[no] nobody else can run this file
            }
    }
# The three bits that are not about a person. The sentence leads with what
# surprises: whoever points at an `s` is asking about the `s`.
files-perm-setuid = { $set ->
        [yes] Runs as { $who }, not as whoever started it.
       *[no] The bit is set on a position that cannot execute — almost always a mistake.
    }
files-perm-setgid = { $kind ->
        [dir] { $set ->
                [yes] Whatever is created in here inherits the group { $group }.
               *[no] The bit is set on a position that cannot execute.
            }
       *[file] { $set ->
                [yes] Runs with the group { $group }, not with the starter's.
               *[no] The bit is set on a position that cannot execute.
            }
    }
files-perm-sticky = { $set ->
        [yes] Only the owner of a file may delete it from here — it is what makes /tmp work.
       *[no] The bit is set on a position that cannot execute.
    }
# The four modes that answer "what should this be".
files-preset-file = ordinary file — you write, everyone else reads
files-preset-private = you, and nobody else
files-preset-program = program — everyone runs it, only you change it
files-preset-private-dir = a directory of your own
files-preset-readonly = nobody changes it, not even you
# The size. The full stop is HERE because the decimal separator belongs to the
# language.
files-size-bytes = { $whole } B
files-size-kib = { $whole }.{ $tenth } KiB
files-size-mib = { $whole }.{ $tenth } MiB
files-size-gib = { $whole }.{ $tenth } GiB
files-size-tib = { $whole }.{ $tenth } TiB
files-order-name = name
files-order-size = size
files-order-modified = modified
files-order-kind = kind
# The rows of the action menu. The id and the key cannot diverge — a test
# compares the two, because a row copied with the id changed COMPILES and shows
# the neighbouring row's words.
files-open = open
files-open-map = open the place in a map
files-open-with = open with
files-terminal = terminal here
files-root-terminal = open as root
files-copy-path = copy the path
files-copy = copy
files-cut = cut
files-paste = paste here
files-rename = rename
files-info = information
files-mode = permissions
files-owner = owner
files-group = group
files-compress = compress…
files-extract = extract
files-trash = move to the trash
files-delete = delete for good
files-shred = delete by overwriting
files-pin = put on the sidebar
files-unhide = bring back a hidden sidebar row
# THE SIDEBAR'S MENU. Each id becomes `sidebar-<id>`, and a check compares both.
sidebar-open = open
sidebar-mount = mount
sidebar-unmount = unmount
sidebar-eject = eject safely
sidebar-information = information
sidebar-unpin = take off the sidebar
sidebar-format = format
sidebar-fact-node = device
sidebar-fact-kind = filesystem
sidebar-fact-size = size
sidebar-fact-drive = drive
# EACH ACTION'S SECOND REGISTER — what it DOES, for somebody who already knows.
#
# The label is for whoever is looking for a verb ("delete for good"); this line
# is for whoever wants to know exactly what will happen: which call, which
# options, what can be undone. It reaches the message line when the row is under
# the CURSOR — not only under the pointer, because whatever the mouse can do the
# keyboard can do.
files-does-open = opens in the MIME type's default application; a folder, or an archive, is walked into
# THE ONE ACTION IN THIS MENU THAT LEAVES THE MACHINE, and the sentence says
# so before the key is pressed — the same courtesy the glance's two hosts get.
files-does-open-map = opens the coordinate written in this file in the browser's map — the coordinate leaves this machine
files-does-open-with = choose among the `.desktop` files that declare this MIME type
files-does-terminal = opens the terminal in this folder, as you
files-does-root-terminal = opens the terminal as root, through pkexec
files-does-copy-path = puts the absolute path on the clipboard; opens nothing
files-does-copy = marks for copying; pasting runs it through the queue, which outlives the footer
files-does-cut = marks for moving; on the same filesystem it is rename(2), instant
files-does-paste = runs what was marked; across filesystems it is a copy and then a removal
files-does-rename = renames in place with rename(2); does not cross filesystems
files-does-pin = adds a row to the sidebar, written to $XDG_CONFIG_HOME
files-does-unhide = also lists what begins with a dot
files-does-info = size, owner, mode, times and type; does not touch the contents
files-does-mode = changes the permission bits with chmod(2)
files-does-owner = changes the owner with chown(2); asks for the root password
files-does-group = changes the group with chown(2)
files-does-compress = builds an archive through the queue, without blocking the listing
files-does-extract = extracts into a folder named after the archive
files-does-restore = puts it back at the origin path the trash recorded
files-does-trash = moves it to the XDG trash; does not cross filesystems
files-does-delete = unlink(2) or rmdir(2) straight away: no trash, no way back
files-does-shred = overwrites before deleting; on copy-on-write btrfs that guarantees nothing
# And the sidebar menu's, which talk to udisks.
sidebar-does-open = enters the folder; for an unmounted device it opens its sheet
sidebar-does-mount = udisks2's Filesystem.Mount — it picks the point and the options
sidebar-does-unmount = Filesystem.Unmount, never forced: it refuses while something is open
sidebar-does-eject = unmounts, locks an encrypted disk again, and cuts the drive's power (Drive.PowerOff): what makes it safe to pull
sidebar-does-information = label, node, filesystem, size and drive
sidebar-does-unpin = removes the row; a distro default comes back hidden, yours is removed
sidebar-does-format = opens the formatting buffer already pointed at this target
files-restore = put back where it came from
# In a window the change is not in force anywhere: the file is the only road.
echo-setting-not-written = { $file } could not be saved ({ $why }), so nothing changed.

## file info editing — everything the info panel can change

files-remove-location = remove the place it was made
files-does-remove-location = asks first; then rewrites the file — or writes a copy beside it — without its GPS coordinate. Taken out of the file itself, it cannot be undone
# The archive rows of the file manager's menu (`ronoctua_files::menu`).
files-archive-test = test the archive
files-archive-convert = convert to another format…
files-archive-add = add what was copied to this archive
files-archive-extract = extract here, beside the archive
files-archive-remove = take out of the archive
files-does-archive-test = reads every member and checks it against its sums (7zz t, unrar t, bsdtar -t), through the queue
files-does-archive-convert = extracts into a hidden folder beside it and packs again in the format chosen on the sheet
files-does-archive-add = 7zz a: the copied files go in at the archive's top, encrypted with its password when it has one
files-does-archive-extract = takes the chosen members out into the archive's own folder; nothing there is replaced
files-does-archive-remove = 7zz d: rewrites the archive without them — it cannot be undone
sidebar-lock = lock
sidebar-does-lock = unmounts what came out of it and locks the container (Encrypted.Lock); the power stays on
# The file manager's menu row that checks a file with VirusTotal.
files-virustotal = check with VirusTotal
files-does-virustotal = sends its fingerprint (SHA-256) to VirusTotal, in the security centre's window; the file itself only if you say yes
# The file manager's menu row (`C-m`, `k`).
files-belongings = keep in the belongings, or stop
files-does-belongings = adds it to what the belongings keep in your repository and put back on a new machine; when it is kept already, takes it out — the file stays where it is
files-private-withheld-shut = /private is not open: open it, or resume it, with its passphrase to see what is inside.
files-private-withheld-silent = /private is not answering: it was suspended while in use, and its passphrase resumes it.
sidebar-write-installer = write the installer here
sidebar-does-write-installer = opens the installer stick on this drive: the whole drive is written, after a question whose first answer is no

## THE SECURITY CENTRE'S KEYS, and the passwordless window's count.

files-encrypt = encrypt…
files-does-encrypt = seals it with age beside the original — for you, with a passphrase or for someone's key; a folder becomes name.tar.age
files-decrypt = decrypt…
files-does-decrypt = opens the .age beside it, never over anything — with its passphrase, or with your belongings' key
