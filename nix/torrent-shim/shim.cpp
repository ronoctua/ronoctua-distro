// The C++ half of the border with libtorrent.
//
// Everything C++ stops here: exceptions are caught at every boundary, no
// object crosses, and the only things that leave are integers and bytes the
// caller already owns. See `shim.h` for why that shape was chosen.
//
// WHY A SEPARATE DERIVATION AND NOT A `cc` CRATE. `nix/rust.nix`'s file set
// copies `.rs`, `.wit` and `.wgsl` into the store and nothing else, so a
// `.cpp` next to the Rust would simply not be there inside the Nix build —
// the same trap the `.wgsl` comment in that file records. Building it on its
// own also keeps `build.rs` a pure reader of one environment variable, which
// is exactly what `crates/ronoctua-term/build.rs` already is.

#include "shim.h"

#include <libtorrent/add_torrent_params.hpp>
#include <libtorrent/bencode.hpp>
#include <libtorrent/create_torrent.hpp>
#include <libtorrent/alert_types.hpp>
#include <libtorrent/magnet_uri.hpp>
#include <libtorrent/read_resume_data.hpp>
#include <libtorrent/session.hpp>
#include <libtorrent/session_params.hpp>
#include <libtorrent/settings_pack.hpp>
#include <libtorrent/torrent_flags.hpp>
#include <libtorrent/torrent_handle.hpp>
#include <libtorrent/torrent_info.hpp>
#include <libtorrent/torrent_status.hpp>
#include <libtorrent/version.hpp>
#include <libtorrent/write_resume_data.hpp>

#include <cstring>
#include <filesystem>
#include <fstream>
#include <map>
#include <string>
#include <vector>

namespace lt = libtorrent;

namespace {

// Hex by hand, over the digest's own bytes.
//
// libtorrent's own `to_hex` lives under `aux/`, which upstream is explicit
// about being internal — and this is eight lines that will never change.
template <typename Digest> std::string hex_of(Digest const &d) {
  static char const *digits = "0123456789abcdef";
  std::string out;
  out.reserve(d.size() * 2);
  for (auto const byte : d) {
    auto const b = static_cast<unsigned char>(byte);
    out.push_back(digits[b >> 4]);
    out.push_back(digits[b & 0x0f]);
  }
  return out;
}

// THE NAME A TORRENT IS KNOWN BY HERE.
//
// The v2 hash when there is one, the v1 hash otherwise — never `get_best()`,
// which TRUNCATES a SHA-256 to twenty bytes so that v1 code paths can carry
// it. Truncated, two different v2 torrents can collide, and the thing being
// named is what a cancel deletes.
std::string name_of(lt::info_hash_t const &h) {
  if (h.has_v2()) {
    return hex_of(h.v2);
  }
  return hex_of(h.v1);
}

void put(char *dst, size_t room, std::string const &text) {
  std::memset(dst, 0, room);
  auto const n = text.size() < room - 1 ? text.size() : room - 1;
  std::memcpy(dst, text.data(), n);
}

int state_of(lt::torrent_status const &st) {
  if (st.errc) {
    return RN_ERRORED;
  }
  switch (st.state) {
  case lt::torrent_status::checking_files:
  case lt::torrent_status::checking_resume_data:
    return RN_CHECKING;
  case lt::torrent_status::downloading_metadata:
    return RN_METADATA;
  case lt::torrent_status::downloading:
    return RN_DOWNLOADING;
  case lt::torrent_status::finished:
    return RN_FINISHED;
  case lt::torrent_status::seeding:
    return RN_SEEDING;
  default:
    return RN_DOWNLOADING;
  }
}

} // namespace

struct rn_session {
  lt::session session;
  std::filesystem::path state;
  std::map<std::string, lt::torrent_handle> handles;

  explicit rn_session(lt::session_params params, std::filesystem::path dir)
      : session(std::move(params)), state(std::move(dir)) {}
};

namespace {

// Everything a resume file is named after, so the two halves agree.
std::filesystem::path resume_file(std::filesystem::path const &dir,
                                  std::string const &hash) {
  return dir / (hash + ".resume");
}

void remember(rn_session *s, lt::torrent_handle const &h) {
  if (!h.is_valid()) {
    return;
  }
  s->handles[name_of(h.info_hashes())] = h;
}

lt::torrent_handle find(rn_session *s, char const *hash) {
  if (hash == nullptr) {
    return {};
  }
  auto const it = s->handles.find(std::string(hash));
  return it == s->handles.end() ? lt::torrent_handle{} : it->second;
}

// Drains the alert queue.
//
// NOT OPTIONAL, and not housekeeping: libtorrent buffers alerts and, once the
// buffer is full, starts DROPPING them — including the `save_resume_data`
// ones this file waits for. A session nobody pops from also grows without
// bound. Everything except resume data is discarded here, because the queue's
// screen is fed by `get_torrent_status` and not by alerts.
void drain(rn_session *s, bool keep_resume) {
  std::vector<lt::alert *> alerts;
  s->session.pop_alerts(&alerts);
  if (!keep_resume) {
    return;
  }
  for (auto *a : alerts) {
    if (auto *rd = lt::alert_cast<lt::save_resume_data_alert>(a)) {
      auto const hash = name_of(rd->handle.info_hashes());
      auto const buffer = lt::write_resume_data_buf(rd->params);
      std::ofstream out(resume_file(s->state, hash), std::ios::binary);
      out.write(buffer.data(), static_cast<std::streamsize>(buffer.size()));
    }
  }
}

} // namespace

extern "C" {

rn_session *rn_session_new(const char *state_dir, int port_low, int port_high) {
  try {
    std::filesystem::path dir(state_dir == nullptr ? "." : state_dir);
    std::error_code ignored;
    std::filesystem::create_directories(dir, ignored);

    lt::settings_pack pack;
    // A FIXED RANGE, and no UPnP or NAT-PMP.
    //
    // Seeding means accepting connections, and a program that asks the router
    // to open a port on its own is a program that changed the machine's
    // exposure without saying so. The range is declared in the Nix module,
    // where a firewall rule can name the same numbers.
    pack.set_str(lt::settings_pack::listen_interfaces,
                 "0.0.0.0:" + std::to_string(port_low) + "-" +
                     std::to_string(port_high) + ",[::]:" +
                     std::to_string(port_low) + "-" +
                     std::to_string(port_high));
    pack.set_bool(lt::settings_pack::enable_upnp, false);
    pack.set_bool(lt::settings_pack::enable_natpmp, false);
    // Only what this file reads. A wider mask is buffer filled with alerts
    // nobody pops.
    pack.set_int(lt::settings_pack::alert_mask,
                 lt::alert_category::status | lt::alert_category::error);
    pack.set_str(lt::settings_pack::user_agent, "ronoctua/" LIBTORRENT_VERSION);

    lt::session_params params(pack);
    auto *s = new rn_session(std::move(params), dir);

    // WHAT WAS RUNNING BEFORE THE REBOOT.
    //
    // Without this the session comes up empty and every torrent has to be
    // added again by hand; with it — and only with it — a re-added torrent
    // skips the re-check of files already on disk, which for forty gibibytes
    // is minutes of reading.
    if (std::filesystem::is_directory(dir)) {
      for (auto const &entry : std::filesystem::directory_iterator(dir)) {
        if (entry.path().extension() != ".resume") {
          continue;
        }
        std::ifstream in(entry.path(), std::ios::binary);
        std::vector<char> buffer((std::istreambuf_iterator<char>(in)),
                                 std::istreambuf_iterator<char>());
        if (buffer.empty()) {
          continue;
        }
        lt::error_code ec;
        auto params = lt::read_resume_data(buffer, ec);
        if (ec) {
          continue;
        }
        remember(s, s->session.add_torrent(std::move(params), ec));
      }
    }
    return s;
  } catch (...) {
    return nullptr;
  }
}

void rn_session_free(rn_session *session) {
  if (session == nullptr) {
    return;
  }
  try {
    rn_save_resume(session);
  } catch (...) {
  }
  delete session;
}

int rn_add_magnet(rn_session *session, const char *uri, const char *save_path,
                  char *out_hash) {
  if (session == nullptr || uri == nullptr || out_hash == nullptr) {
    return -1;
  }
  try {
    lt::error_code ec;
    auto params = lt::parse_magnet_uri(uri, ec);
    if (ec) {
      return -1;
    }
    params.save_path = save_path == nullptr ? "." : save_path;
    auto handle = session->session.add_torrent(std::move(params), ec);
    if (ec || !handle.is_valid()) {
      return -1;
    }
    remember(session, handle);
    put(out_hash, RN_HASH, name_of(handle.info_hashes()));
    return 0;
  } catch (...) {
    return -1;
  }
}

int rn_add_file(rn_session *session, const char *path, const char *save_path,
                char *out_hash) {
  if (session == nullptr || path == nullptr || out_hash == nullptr) {
    return -1;
  }
  try {
    lt::error_code ec;
    auto info = std::make_shared<lt::torrent_info>(std::string(path), ec);
    if (ec) {
      return -1;
    }
    lt::add_torrent_params params;
    params.ti = std::move(info);
    params.save_path = save_path == nullptr ? "." : save_path;
    auto handle = session->session.add_torrent(std::move(params), ec);
    if (ec || !handle.is_valid()) {
      return -1;
    }
    remember(session, handle);
    put(out_hash, RN_HASH, name_of(handle.info_hashes()));
    return 0;
  } catch (...) {
    return -1;
  }
}

int rn_pause(rn_session *session, const char *hash) {
  if (session == nullptr) {
    return -1;
  }
  try {
    auto handle = find(session, hash);
    if (!handle.is_valid()) {
      return -1;
    }
    // `auto_managed(false)` FIRST, and it is not belt and braces: an
    // auto-managed torrent is one libtorrent's own queue may resume whenever
    // it likes, so pausing it alone produces a torrent that starts again by
    // itself a few seconds later.
    handle.unset_flags(lt::torrent_flags::auto_managed);
    handle.pause(lt::torrent_handle::graceful_pause);
    return 0;
  } catch (...) {
    return -1;
  }
}

int rn_resume(rn_session *session, const char *hash) {
  if (session == nullptr) {
    return -1;
  }
  try {
    auto handle = find(session, hash);
    if (!handle.is_valid()) {
      return -1;
    }
    handle.resume();
    return 0;
  } catch (...) {
    return -1;
  }
}

int rn_remove(rn_session *session, const char *hash, int delete_files) {
  if (session == nullptr || hash == nullptr) {
    return -1;
  }
  try {
    auto const key = std::string(hash);
    auto handle = find(session, hash);
    if (!handle.is_valid()) {
      return -1;
    }
    session->session.remove_torrent(
        handle, delete_files != 0 ? lt::session::delete_files
                                  : lt::remove_flags_t{});
    session->handles.erase(key);
    std::error_code ignored;
    std::filesystem::remove(resume_file(session->state, key), ignored);
    return 0;
  } catch (...) {
    return -1;
  }
}

int rn_poll(rn_session *session, rn_status *out, int max) {
  if (session == nullptr || out == nullptr || max <= 0) {
    return -1;
  }
  try {
    drain(session, true);
    auto const all = session->session.get_torrent_status(
        [](lt::torrent_status const &) { return true; });
    int n = 0;
    for (auto const &st : all) {
      if (n >= max) {
        break;
      }
      auto &row = out[n];
      std::memset(&row, 0, sizeof(row));
      put(row.hash, RN_HASH, name_of(st.info_hashes));
      put(row.name, RN_NAME, st.name);
      put(row.trouble, RN_TROUBLE, st.errc ? st.errc.message() : std::string());
      // `total_wanted` and NOT `total`: a torrent with files deselected is
      // done when the WANTED ones are, and a bar against the whole thing
      // would stop short of the end and stay there.
      row.done = static_cast<uint64_t>(st.total_wanted_done < 0
                                           ? 0
                                           : st.total_wanted_done);
      row.total =
          static_cast<uint64_t>(st.total_wanted < 0 ? 0 : st.total_wanted);
      row.down_rate = static_cast<uint64_t>(
          st.download_payload_rate < 0 ? 0 : st.download_payload_rate);
      row.up_rate = static_cast<uint64_t>(
          st.upload_payload_rate < 0 ? 0 : st.upload_payload_rate);
      row.state = state_of(st);
      row.flags = 0;
      if ((st.flags & lt::torrent_flags::paused) != lt::torrent_flags_t{}) {
        row.flags |= RN_FLAG_PAUSED;
      }
      if (st.info_hashes.has_v1()) {
        row.flags |= RN_FLAG_HAS_V1;
      }
      if (st.info_hashes.has_v2()) {
        row.flags |= RN_FLAG_HAS_V2;
      }
      row.peers = st.num_peers;
      row.seeds = st.num_seeds;
      ++n;
    }
    return n;
  } catch (...) {
    return -1;
  }
}

int rn_save_resume(rn_session *session) {
  if (session == nullptr) {
    return -1;
  }
  try {
    for (auto const &pair : session->handles) {
      if (pair.second.is_valid()) {
        pair.second.save_resume_data(lt::torrent_handle::save_info_dict |
                                     lt::torrent_handle::flush_disk_cache);
      }
    }
    // The alerts do not arrive in the same breath they were asked for. A short
    // bounded wait is what turns "asked" into "written", and it is bounded so
    // that shutting down cannot hang on a disk that is not answering.
    for (int round = 0; round < 40; ++round) {
      session->session.wait_for_alert(std::chrono::milliseconds(50));
      drain(session, true);
    }
    return 0;
  } catch (...) {
    return -1;
  }
}

int rn_make_torrent(const char *content, const char *out_file, int flavour) {
  if (content == nullptr || out_file == nullptr) {
    return -1;
  }
  try {
    lt::file_storage storage;
    lt::add_files(storage, std::string(content));
    if (storage.num_files() == 0) {
      return -1;
    }
    // THE FLAVOUR IS THE WHOLE POINT of this function existing.
    //
    // `v2_only` produces a torrent with `meta version 2`, a `file tree` and
    // `piece layers` and NO v1 info dictionary at all — which is what makes
    // "this build speaks BEP 52" checkable rather than assertable.
    lt::create_flags_t flags{};
    if (flavour == 1) {
      flags |= lt::create_torrent::v1_only;
    } else if (flavour == 2) {
      flags |= lt::create_torrent::v2_only;
    }
    // Sixteen kibibytes: the smallest piece size v2 allows, so a fixture of a
    // few hundred kilobytes still has several pieces to verify.
    lt::create_torrent maker(storage, 16 * 1024, flags);
    lt::error_code ec;
    auto const parent =
        std::filesystem::path(content).parent_path().string();
    lt::set_piece_hashes(maker, parent.empty() ? "." : parent, ec);
    if (ec) {
      return -1;
    }
    std::vector<char> buffer;
    lt::bencode(std::back_inserter(buffer), maker.generate());
    std::ofstream out(out_file, std::ios::binary);
    out.write(buffer.data(), static_cast<std::streamsize>(buffer.size()));
    return out ? 0 : -1;
  } catch (...) {
    return -1;
  }
}

const char *rn_version(void) { return LIBTORRENT_VERSION; }

} // extern "C"
