//! Which writer each container gets — the one table, so a door, the reader
//! and the writer can never disagree about it.
//!
//! # The three writers, and why the order is this one
//!
//! 1. **lofty**, in process, for every container it writes: it edits the
//!    tag and leaves every byte of sound and picture where it was (MP4's
//!    chunk offsets are moved with the `moov`; `stco`, `co64` and `tfhd` are
//!    rewritten by lofty itself).
//! 2. **`mkvpropedit`** for Matroska and WebM: lofty has no Matroska at all,
//!    and `mkvpropedit` edits in place without remuxing. It needs
//!    `mkvextract` beside it, because `--tags global:` REPLACES every global
//!    tag and the ones nobody touched have to be read first. Both come from
//!    `mkvtoolnix-cli`, which pulls Qt in, so they are installed only where
//!    the media profile already carries Qt — on the minimal profile a
//!    Matroska sheet says "mkvpropedit is not installed" and stays read-only.
//! 3. **the remux** — `ffmpeg -map 0 -c copy -map_metadata 0 -map_chapters
//!    0` into a new file — ONLY where the muxer is known to keep every
//!    stream. `-map 0` makes ffmpeg fail rather than drop a stream it cannot
//!    mux, so the danger is not a missing stream: it is what a muxer keeps
//!    beside the streams. The mov muxer drops Apple's `Keys` (a Live Photo's
//!    pairing) without `use_metadata_tags`, the AVI muxer rewrites the whole
//!    index, the transport stream muxer renumbers PIDs. Those containers are
//!    refused, not remuxed.
//!
//! What is left — AVI, transport streams, FLV, ASF — says [`Why::NoWriter`].

use crate::sheet::{Container, Program, Why, Writer};

impl Container {
    /// Who writes this container's fields and tags, or why nobody does.
    ///
    /// # Errors
    ///
    /// [`Why::NoWriter`] for the containers the module documentation lists.
    pub const fn fields_writer(self) -> Result<Writer, Why> {
        match self {
            Self::Mpeg
            | Self::Aac
            | Self::Flac
            | Self::OggVorbis
            | Self::Opus
            | Self::Speex
            | Self::Mp4
            | Self::Wav
            | Self::Aiff
            | Self::Ape
            | Self::Mpc
            | Self::WavPack => Ok(Writer::Lofty),
            Self::Matroska | Self::WebM => Ok(Writer::Matroska),
            Self::OggOther => Ok(Writer::Remux),
            Self::Avi | Self::MpegTs | Self::Flv | Self::Asf | Self::Other => {
                Err(Why::NoWriter { container: self })
            }
        }
    }

    /// Who writes its front cover, or why nobody does.
    ///
    /// The Ogg remux carries no picture: a Theora file's cover would be a
    /// Vorbis comment on the video stream, which no player reads.
    ///
    /// # Errors
    ///
    /// [`Why::NoWriter`] where [`Container::fields_writer`] refuses, and for
    /// the remux.
    pub const fn cover_writer(self) -> Result<Writer, Why> {
        match self.fields_writer() {
            Ok(Writer::Remux) => Err(Why::NoWriter { container: self }),
            other => other,
        }
    }

    /// Who writes a stream's language, title and marks, or why nobody does.
    ///
    /// Matroska only: lofty edits tags, not tracks, and the remux would
    /// have to rebuild the file for a two-letter change.
    ///
    /// # Errors
    ///
    /// [`Why::StreamsOnlyInMatroska`] everywhere else.
    pub const fn streams_writer(self) -> Result<Writer, Why> {
        match self {
            Self::Matroska | Self::WebM => Ok(Writer::Matroska),
            _ => Err(Why::StreamsOnlyInMatroska { container: self }),
        }
    }

    /// Who writes a chapter's title, or why nobody does.
    ///
    /// Matroska only: `mkvpropedit --chapters` hands its chapters back
    /// whole; lofty has none, and a remux would rebuild the file.
    ///
    /// # Errors
    ///
    /// [`Why::ChaptersOnlyInMatroska`] everywhere else.
    pub const fn chapters_writer(self) -> Result<Writer, Why> {
        match self {
            Self::Matroska | Self::WebM => Ok(Writer::Matroska),
            _ => Err(Why::ChaptersOnlyInMatroska { container: self }),
        }
    }

    /// The ffmpeg muxer a remux writes with (`-f <muxer>`), where there is
    /// one.
    #[must_use]
    pub const fn remux_muxer(self) -> Option<&'static str> {
        match self {
            Self::OggOther => Some("ogg"),
            _ => None,
        }
    }

    /// Whether lofty reads this container.
    #[must_use]
    pub const fn lofty(self) -> bool {
        matches!(self.fields_writer(), Ok(Writer::Lofty))
    }

    /// Whether it is Matroska (or WebM).
    #[must_use]
    pub const fn matroska(self) -> bool {
        matches!(self, Self::Matroska | Self::WebM)
    }

    /// The containers that hold moving pictures as a rule — for deciding
    /// the kind when ffprobe could not be asked.
    #[must_use]
    pub const fn usually_video(self) -> bool {
        matches!(
            self,
            Self::Matroska | Self::WebM | Self::Avi | Self::MpegTs | Self::Flv | Self::OggOther
        )
    }
}

impl Writer {
    /// The programs this writer needs, all of them.
    #[must_use]
    pub const fn programs(self) -> &'static [Program] {
        match self {
            Self::Lofty => &[],
            // mkvmerge is not here: it adds flags and attachment names to a
            // sheet, and a write goes by track ORDER and attachment NAME,
            // which ffprobe already gave.
            Self::Matroska => &[Program::Mkvextract, Program::Mkvpropedit],
            Self::Remux => &[Program::Ffmpeg],
        }
    }
}

/// Every container, so a test can walk the table.
pub const EVERY_CONTAINER: [Container; 20] = [
    Container::Mpeg,
    Container::Aac,
    Container::Flac,
    Container::OggVorbis,
    Container::Opus,
    Container::Speex,
    Container::OggOther,
    Container::Mp4,
    Container::Wav,
    Container::Aiff,
    Container::Ape,
    Container::Mpc,
    Container::WavPack,
    Container::Matroska,
    Container::WebM,
    Container::Avi,
    Container::MpegTs,
    Container::Flv,
    Container::Asf,
    Container::Other,
];

#[cfg(test)]
mod tests {
    use super::EVERY_CONTAINER;
    use crate::sheet::{Container, Why, Writer};

    #[test]
    fn a_remux_is_offered_only_where_a_muxer_is_named() {
        for c in EVERY_CONTAINER {
            let remux = c.fields_writer() == Ok(Writer::Remux);
            assert_eq!(remux, c.remux_muxer().is_some(), "{c:?}");
        }
    }

    #[test]
    fn the_containers_that_would_lose_something_are_refused_by_name() {
        for c in [
            Container::Avi,
            Container::MpegTs,
            Container::Flv,
            Container::Asf,
            Container::Other,
        ] {
            assert_eq!(c.fields_writer(), Err(Why::NoWriter { container: c }));
            assert_eq!(c.cover_writer(), Err(Why::NoWriter { container: c }));
        }
    }

    #[test]
    fn matroska_goes_to_its_own_tools_and_nothing_else_edits_a_stream() {
        for c in EVERY_CONTAINER {
            let streams = c.streams_writer().is_ok();
            assert_eq!(streams, c.matroska(), "{c:?}");
            assert_eq!(c.chapters_writer().is_ok(), c.matroska(), "{c:?}");
            if c.matroska() {
                assert_eq!(c.fields_writer(), Ok(Writer::Matroska));
                assert_eq!(c.cover_writer(), Ok(Writer::Matroska));
            }
        }
    }

    #[test]
    fn a_remuxed_container_keeps_no_cover() {
        assert_eq!(Container::OggOther.fields_writer(), Ok(Writer::Remux));
        assert!(Container::OggOther.cover_writer().is_err());
    }
}
