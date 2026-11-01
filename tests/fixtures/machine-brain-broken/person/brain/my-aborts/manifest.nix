# A MANIFEST THAT ABORTS: an error `tryEval` cannot catch. Root's check
# stops on it in a process of its own, and the machine leaves the folder out
# and never reads it (`a-folder-that-does-not-evaluate-stops-only-itself`).
abort "a person's manifest that aborts"
