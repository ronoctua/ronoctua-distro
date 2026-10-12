# THE HASH OF ONE OF THE PERSON'S BRAIN MODULES — the single implementation.
#
# An approval (`local/approved.json`, written through the sudo door) names the
# module by this hash, and `lib.machine` imports the module's NixOS half only
# while the folder still hashes to it (`nix/machine.nix`, `approvals`). The
# root helper that records an approval asks THIS file, through
# `nix eval --raw --expr '(import <distro>/nix/person-hash.nix) <folder>'`, so
# the number shown when approving and the number compared when building cannot
# come from two implementations that drift.
#
# # What it covers
#
# The whole folder: every file's name, its place and its SHA-256, and every
# folder's name. Moving, renaming, adding or editing anything changes it —
# the Rust half runs as the person, the NixOS half as root, and an approval is
# of the module and not of one file.
#
# NOT the location: the walk is relative to the folder, so the copy the person
# staged and the copy the machine flake carries into the store hash the same.
#
# # What it refuses
#
# A link and anything that is neither a file nor a folder (a module is text),
# and a name outside `[A-Za-z0-9._+-]` — a newline in a name would let two
# different folders write the same listing. A refusal is a `throw`: the machine
# never imports what it cannot hash, and the approval cannot name it.
#
# The answer is SRI (`sha256-<base64>`), the shape every hash in the person's
# files has.
folder:
let
  walk =
    relative: at:
    let
      entries = builtins.readDir at;
    in
    builtins.concatLists (
      map (
        name:
        let
          type = entries.${name};
          here = if relative == "" then name else "${relative}/${name}";
          path = at + "/${name}";
        in
        if builtins.match "[A-Za-z0-9._+-]+" name == null then
          throw "person-hash: `${here}` has a name a module may not use"
        else if type == "directory" then
          [ "directory ${here}" ] ++ walk here path
        else if type == "regular" then
          [ "file ${here} ${builtins.hashFile "sha256" path}" ]
        else
          throw "person-hash: `${here}` is a ${type}; a module holds files and folders only"
      ) (builtins.attrNames entries)
    );
  listing = builtins.concatStringsSep "\n" (walk "" folder) + "\n";
in
builtins.convertHash {
  hash = builtins.hashString "sha256" listing;
  hashAlgo = "sha256";
  toHashFormat = "sri";
}
