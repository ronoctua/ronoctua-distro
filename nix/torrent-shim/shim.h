/* The C ABI the Rust binds — the whole of the border with libtorrent.
 *
 * THE SIX VERBS, AND NOT THE LIBRARY. libtorrent is 272 C++ headers and no C
 * ABI at all; binding it would mean binding C++, which has no stable ABI, name
 * mangling, exceptions and `std::shared_ptr`. What the queue actually needs is
 * add, pause, resume, remove and "where is everything" — about twenty
 * functions, all of which can be plain C.
 *
 * NOTHING CROSSING THIS LINE OWNS MEMORY. Every string that comes back is a
 * fixed-size array inside a struct the caller allocated, so there is no
 * `free` to forget and no lifetime to get wrong. That costs a copy per poll
 * and buys away an entire class of bug.
 *
 * `sizeof(rn_status)` IS THE CONTRACT. `#[repr(C)]` on the Rust side promises
 * to lay out what it was TOLD, not to check it against this file — a wrong
 * width is just a register with the wrong contents, and it produces no
 * diagnostic at all. `tests/checks.nix`'s `torrent-layout-matches-cxx`
 * compiles this header with the real C++ compiler and compares every offset
 * against the Rust's.
 */

#ifndef RONOCTUA_TORRENT_SHIM_H
#define RONOCTUA_TORRENT_SHIM_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* THE TWELVE THAT LEAVE, and nothing else.
 *
 * The library is compiled with `-fvisibility=hidden` so that the tens of
 * thousands of inline C++ symbols libtorrent's headers instantiate stay
 * private. That hides these too unless they say otherwise, and the failure is
 * a link error in the RUST — far from the flag that caused it. */
#define RN_EXPORT __attribute__((visibility("default")))

/* Room for a v2 info hash: SHA-256 is 32 bytes, 64 hex characters, plus the
 * terminator. A v1 hash uses the first 41 and leaves the rest zeroed. */
#define RN_HASH 65
#define RN_NAME 256
#define RN_TROUBLE 192

/* Where a torrent is, in this project's terms and not libtorrent's: the
 * queue has six stages and they are the same six for a copy, a download and
 * this. Translating here rather than in the Rust keeps the mapping next to the
 * header that defines what it is translating from. */
enum rn_state {
  RN_CHECKING = 0,
  RN_METADATA = 1,
  RN_DOWNLOADING = 2,
  RN_FINISHED = 3,
  RN_SEEDING = 4,
  RN_ERRORED = 5
};

/* Bits of `rn_status.flags`. */
#define RN_FLAG_PAUSED 1
#define RN_FLAG_HAS_V1 2
#define RN_FLAG_HAS_V2 4

typedef struct {
  char hash[RN_HASH];
  char name[RN_NAME];
  char trouble[RN_TROUBLE];
  uint64_t done;
  uint64_t total;
  uint64_t down_rate;
  uint64_t up_rate;
  int32_t state;
  int32_t flags;
  int32_t peers;
  int32_t seeds;
} rn_status;

typedef struct rn_session rn_session;

/* Starts a session listening on the given port range, restoring whatever the
 * state directory holds. NULL on failure. */
RN_EXPORT rn_session *rn_session_new(const char *state_dir, int port_low, int port_high);

/* Stops it, after one last save. */
RN_EXPORT void rn_session_free(rn_session *session);

/* Adds a magnet. `out_hash` must have room for RN_HASH bytes. 0 on success. */
RN_EXPORT int rn_add_magnet(rn_session *session, const char *uri, const char *save_path,
                  char *out_hash);

/* The same, from a `.torrent` file on disk. */
RN_EXPORT int rn_add_file(rn_session *session, const char *path, const char *save_path,
                char *out_hash);

RN_EXPORT int rn_pause(rn_session *session, const char *hash);
RN_EXPORT int rn_resume(rn_session *session, const char *hash);

/* Gives up on it. With `delete_files`, what was written goes too — which is
 * the promise a cancelled copy makes, and the reason cancel asks first. */
RN_EXPORT int rn_remove(rn_session *session, const char *hash, int delete_files);

/* Fills up to `max` entries and returns how many, or -1. */
RN_EXPORT int rn_poll(rn_session *session, rn_status *out, int max);

/* Writes resume data for everything, so a reboot does not re-check. */
RN_EXPORT int rn_save_resume(rn_session *session);

/* The library's version, for the log line that says what is actually linked. */
/* Makes a `.torrent` out of a directory.
 *
 * A real feature and not test scaffolding — a torrent client that cannot
 * create one is half a client — but it is also what lets `torrent-speaks-v2`
 * prove BEP 52 OFFLINE: it builds a purely-v2 torrent, adds it, and checks
 * that the session reports a v2 info hash and no v1 one. Without it, "we speak
 * v2" would be a claim resting on the library's changelog.
 *
 * `flavour`: 0 hybrid, 1 v1 only, 2 v2 only. */
RN_EXPORT int rn_make_torrent(const char *content, const char *out_file,
                              int flavour);

RN_EXPORT const char *rn_version(void);

#ifdef __cplusplus
}
#endif

#endif /* RONOCTUA_TORRENT_SHIM_H */
