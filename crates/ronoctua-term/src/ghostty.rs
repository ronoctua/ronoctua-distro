//! The Ghostty engine, over `libghostty-vt` 1.4.0's C API — the engine the
//! terminal runs on.
//!
//! # Three things built on one library
//!
//! - [`Vt`] is the AUTHORITATIVE terminal, the one a holder keeps beside the
//!   PTY. It is the only one that answers the program: every query reply, every
//!   key, mouse, focus and paste encoding goes out through it, and everything
//!   the program says that is not screen content (the bell, the title, the
//!   pwd, a notification, OSC 7501's program status…) comes out of it as an
//!   [`Effect`].
//! - [`Mirror`] is a window's COPY, built only from a [`Vt::snapshot`] through
//!   a [`Decoder`] and then fed the same bytes. It can be scrolled, selected and
//!   searched, and it can NEVER write to the PTY: nothing in its API takes a
//!   callback, its one callback goes in through a private enum that has no PTY
//!   writer in it (`MirrorOption`), and a test asks the library itself whether
//!   one is installed.
//! - [`RenderState`] reads either of them for drawing, in the two phases the
//!   library offers so a lock is held only for the first.
//!
//! # The only file in the project that may write `unsafe`
//!
//! The crate root denies it; the attribute below is the exception, and it is
//! one line in one file so that finding every unsafe in the system is a grep
//! for this attribute rather than a reading of every crate. Every `unsafe`
//! block here is a call into the library or a dereference of a pointer the
//! library handed over for the duration of a call, and each type that holds a
//! library handle says why it may cross a thread.
//!
//! # EVERY NUMBER AND EVERY LAYOUT HERE WAS READ, NOT INFERRED
//!
//! The first binding of this library was written from the headers and it
//! compiled, linked and returned a blank grid: three struct layouts were wrong,
//! and none of the three produces a diagnostic — on a 64-bit ABI a wrong
//! integer width and a wrong struct size are both just registers with the
//! wrong contents. So nothing in `sys` is typed from memory:
//!
//! - the enum values were taken from the library's own type manifest
//!   (`ghostty_type_json()`), the structs from the headers of the pinned tag;
//! - every size and offset is a compile-time assertion beside its struct;
//! - `tests/checks.nix` (`ghostty-layout-matches-c`) compiles a C program
//!   against the pinned headers that prints every constant, size and offset
//!   declared in `sys` and compares them with these declarations;
//! - and a test here reads the LINKED library's manifest at run time and
//!   compares the same numbers, so a header from one build and a library from
//!   another cannot agree by accident.
//!
//! # Threads
//!
//! The headers' rule is "no concurrent use, no re-entry", and nothing ties a
//! terminal to the thread that made it. [`Vt`] is therefore `Send` and not
//! `Sync`: one owner at a time, any thread. A [`Mirror`] shares its terminal
//! with its [`Search`] and its [`Decoder`], and every call that touches that
//! terminal goes through one lock (`Shared::gate`), so the three can live on
//! different threads.
// THE ONLY PLACE IN THE PROJECT THAT MAY WRITE `unsafe`. See above.
#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, c_void};
use std::io::Read;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::{NonNull, null, null_mut};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The C declarations: the library's functions, the numbers its enums use
/// and the structs that cross the boundary, each with its measured layout.
///
/// Names follow the C ones with the `GHOSTTY_`/`Ghostty` prefix dropped
/// (`GHOSTTY_TERMINAL_OPT_WRITE_PTY` is `TERMINAL_OPT_WRITE_PTY`,
/// `GhosttyPoint` is `Point`) and fields keep their C names: that is what lets
/// the C probe in `tests/checks.nix` be generated from this module, so a
/// number declared here and not measured cannot exist.
//
// A whole enum is declared even where some of its values are only ever
// received, so the probe measures every value of it and an answer from the
// library never arrives as a number nobody named. Hence the `dead_code`.
#[allow(dead_code, missing_docs, clippy::upper_case_acronyms)]
mod sys {
    use std::ffi::{c_char, c_int, c_void};

    /// `struct GhosttyTerminalImpl*` and every other opaque handle: a pointer.
    pub type Handle = *mut c_void;

    /// A function the library calls back, as `ghostty_terminal_set` takes it:
    /// the callback's address in a `const void*`.
    pub type Callback = *const c_void;

    // Results (`GhosttyResult`).
    pub const SUCCESS: c_int = 0;
    pub const OUT_OF_MEMORY: c_int = -1;
    pub const INVALID_VALUE: c_int = -2;
    pub const OUT_OF_SPACE: c_int = -3;
    pub const NO_VALUE: c_int = -4;
    pub const IO_ERROR: c_int = -5;
    pub const LIMIT_EXCEEDED: c_int = -6;
    pub const REJECTED: c_int = -7;

    // Options of a terminal (`GhosttyTerminalOption`).
    pub const TERMINAL_OPT_USERDATA: c_int = 0;
    pub const TERMINAL_OPT_WRITE_PTY: c_int = 1;
    pub const TERMINAL_OPT_BELL: c_int = 2;
    pub const TERMINAL_OPT_XTVERSION: c_int = 4;
    pub const TERMINAL_OPT_TITLE_CHANGED: c_int = 5;
    pub const TERMINAL_OPT_SIZE: c_int = 6;
    pub const TERMINAL_OPT_COLOR_SCHEME: c_int = 7;
    pub const TERMINAL_OPT_DEVICE_ATTRIBUTES: c_int = 8;
    pub const TERMINAL_OPT_COLOR_FOREGROUND: c_int = 11;
    pub const TERMINAL_OPT_COLOR_BACKGROUND: c_int = 12;
    pub const TERMINAL_OPT_COLOR_CURSOR: c_int = 13;
    pub const TERMINAL_OPT_COLOR_PALETTE: c_int = 14;
    pub const TERMINAL_OPT_APC_MAX_BYTES_KITTY: c_int = 20;
    pub const TERMINAL_OPT_SELECTION: c_int = 21;
    pub const TERMINAL_OPT_DEFAULT_CURSOR_STYLE: c_int = 22;
    pub const TERMINAL_OPT_DEFAULT_CURSOR_BLINK: c_int = 23;

    // Cursor styles a terminal is set to (`GhosttyTerminalCursorStyle`).
    pub const TERMINAL_CURSOR_STYLE_BAR: c_int = 0;
    pub const TERMINAL_CURSOR_STYLE_BLOCK: c_int = 1;
    pub const TERMINAL_CURSOR_STYLE_UNDERLINE: c_int = 2;
    pub const TERMINAL_OPT_PWD_CHANGED: c_int = 25;
    pub const TERMINAL_OPT_CLIPBOARD_WRITE: c_int = 26;
    pub const TERMINAL_OPT_SCROLLBACK_MAX_BYTES: c_int = 27;
    pub const TERMINAL_OPT_SCROLLBACK_MAX_LINES: c_int = 28;
    pub const TERMINAL_OPT_DESKTOP_NOTIFICATION: c_int = 29;
    pub const TERMINAL_OPT_PROGRESS_REPORT: c_int = 30;
    pub const TERMINAL_OPT_CONTINUATION_MAX_BYTES: c_int = 31;
    pub const TERMINAL_OPT_MODE_DEFAULT: c_int = 33;
    pub const TERMINAL_OPT_MODE: c_int = 34;
    pub const TERMINAL_OPT_TERMINFO_NAME: c_int = 37;
    pub const TERMINAL_OPT_CLIPBOARD_READ: c_int = 38;
    pub const TERMINAL_OPT_RENDER_HOLD: c_int = 41;
    pub const TERMINAL_OPT_SEMANTIC_PROMPT: c_int = 42;
    pub const TERMINAL_OPT_RESET: c_int = 43;
    pub const TERMINAL_OPT_PROGRAM_STATUS: c_int = 46;
    pub const TERMINAL_OPT_RESIZE_WINDOW: c_int = 47;

    // What a terminal answers (`GhosttyTerminalData`).
    pub const TERMINAL_DATA_COLS: c_int = 1;
    pub const TERMINAL_DATA_ROWS: c_int = 2;
    pub const TERMINAL_DATA_CURSOR_X: c_int = 3;
    pub const TERMINAL_DATA_CURSOR_Y: c_int = 4;
    pub const TERMINAL_DATA_CURSOR_PENDING_WRAP: c_int = 5;
    pub const TERMINAL_DATA_CURSOR_VISIBLE: c_int = 7;
    pub const TERMINAL_DATA_KITTY_KEYBOARD_FLAGS: c_int = 8;
    pub const TERMINAL_DATA_SCROLLBAR: c_int = 9;
    pub const TERMINAL_DATA_MOUSE_TRACKING: c_int = 11;
    pub const TERMINAL_DATA_TITLE: c_int = 12;
    pub const TERMINAL_DATA_PWD: c_int = 13;
    pub const TERMINAL_DATA_KITTY_GRAPHICS: c_int = 30;
    pub const TERMINAL_DATA_SELECTION: c_int = 31;
    pub const TERMINAL_DATA_VIEWPORT_ACTIVE: c_int = 32;
    pub const TERMINAL_DATA_CONTINUATION_MAX_BYTES: c_int = 36;
    pub const TERMINAL_DATA_MODE: c_int = 37;

    // Points (`GhosttyPointTag`).
    pub const POINT_TAG_ACTIVE: c_int = 0;
    pub const POINT_TAG_VIEWPORT: c_int = 1;

    // Viewport scrolls (`GhosttyTerminalScrollViewportTag`).
    pub const SCROLL_VIEWPORT_TOP: c_int = 0;
    pub const SCROLL_VIEWPORT_BOTTOM: c_int = 1;
    pub const SCROLL_VIEWPORT_DELTA: c_int = 2;
    pub const SCROLL_VIEWPORT_ROW: c_int = 3;
    pub const SCROLL_VIEWPORT_DELTA_PROMPT: c_int = 4;

    // Compression (`GhosttyTerminalCompressionMode`).
    pub const TERMINAL_COMPRESSION_MODE_INCREMENTAL: c_int = 0;
    pub const TERMINAL_COMPRESSION_MODE_FULL: c_int = 1;

    // Compression results (`GhosttyTerminalCompressionResult`).
    pub const TERMINAL_COMPRESSION_RESULT_UNSUPPORTED: c_int = 0;
    pub const TERMINAL_COMPRESSION_RESULT_PENDING: c_int = 1;
    pub const TERMINAL_COMPRESSION_RESULT_COMPLETE: c_int = 2;

    // Cells (`GhosttyCellData`).
    pub const CELL_DATA_CONTENT_TAG: c_int = 2;
    pub const CELL_DATA_WIDE: c_int = 3;
    pub const CELL_DATA_HAS_TEXT: c_int = 4;
    pub const CELL_DATA_HAS_STYLING: c_int = 5;
    pub const CELL_DATA_COLOR_PALETTE: c_int = 10;
    pub const CELL_DATA_COLOR_RGB: c_int = 11;

    // Cell contents (`GhosttyCellContentTag`).
    pub const CELL_CONTENT_CODEPOINT: c_int = 0;
    pub const CELL_CONTENT_CODEPOINT_GRAPHEME: c_int = 1;
    pub const CELL_CONTENT_BG_COLOR_PALETTE: c_int = 2;
    pub const CELL_CONTENT_BG_COLOR_RGB: c_int = 3;

    // Cell widths (`GhosttyCellWide`).
    pub const CELL_WIDE_NARROW: c_int = 0;
    pub const CELL_WIDE_WIDE: c_int = 1;
    pub const CELL_WIDE_SPACER_TAIL: c_int = 2;
    pub const CELL_WIDE_SPACER_HEAD: c_int = 3;

    // Style colours (`GhosttyStyleColorTag`).
    pub const STYLE_COLOR_NONE: c_int = 0;
    pub const STYLE_COLOR_PALETTE: c_int = 1;
    pub const STYLE_COLOR_RGB: c_int = 2;

    // Underlines (`GhosttySgrUnderline`).
    pub const SGR_UNDERLINE_NONE: c_int = 0;
    pub const SGR_UNDERLINE_SINGLE: c_int = 1;
    pub const SGR_UNDERLINE_DOUBLE: c_int = 2;
    pub const SGR_UNDERLINE_CURLY: c_int = 3;
    pub const SGR_UNDERLINE_DOTTED: c_int = 4;
    pub const SGR_UNDERLINE_DASHED: c_int = 5;

    // Clipboards (`GhosttyClipboardLocation`).
    pub const CLIPBOARD_LOCATION_STANDARD: c_int = 0;
    pub const CLIPBOARD_LOCATION_SELECTION: c_int = 1;
    pub const CLIPBOARD_LOCATION_PRIMARY: c_int = 2;

    // Clipboard writes (`GhosttyClipboardWriteResult`).
    pub const CLIPBOARD_WRITE_RESULT_SUCCESS: c_int = 0;
    pub const CLIPBOARD_WRITE_RESULT_DENIED: c_int = 1;

    // Clipboard reads (`GhosttyClipboardReadResult`).
    pub const CLIPBOARD_READ_RESULT_SUCCESS: c_int = 0;
    pub const CLIPBOARD_READ_RESULT_DENIED: c_int = 1;

    // Colour schemes (`GhosttyColorScheme`).
    pub const COLOR_SCHEME_LIGHT: c_int = 0;
    pub const COLOR_SCHEME_DARK: c_int = 1;

    // Progress (`GhosttyTerminalProgressState`).
    pub const TERMINAL_PROGRESS_STATE_REMOVE: c_int = 0;
    pub const TERMINAL_PROGRESS_STATE_SET: c_int = 1;
    pub const TERMINAL_PROGRESS_STATE_ERROR: c_int = 2;
    pub const TERMINAL_PROGRESS_STATE_INDETERMINATE: c_int = 3;
    pub const TERMINAL_PROGRESS_STATE_PAUSE: c_int = 4;

    // Program status states (`GhosttyProgramStatusState`).
    pub const PROGRAM_STATUS_STATE_IDLE: c_int = 0;
    pub const PROGRAM_STATUS_STATE_WORKING: c_int = 1;
    pub const PROGRAM_STATUS_STATE_DONE: c_int = 2;
    pub const PROGRAM_STATUS_STATE_BLOCKED: c_int = 3;
    pub const PROGRAM_STATUS_STATE_ERROR: c_int = 4;
    pub const PROGRAM_STATUS_STATE_CLEAR: c_int = 5;

    // Program status kinds (`GhosttyProgramStatusKind`).
    pub const PROGRAM_STATUS_KIND_NONE: c_int = 0;
    pub const PROGRAM_STATUS_KIND_PERMISSION: c_int = 1;
    pub const PROGRAM_STATUS_KIND_QUESTION: c_int = 2;
    pub const PROGRAM_STATUS_KIND_AUTH: c_int = 3;

    // Shell steps (`GhosttySemanticPromptKind`).
    pub const SEMANTIC_PROMPT_INVALID: c_int = 0;
    pub const SEMANTIC_PROMPT_PROMPT_START: c_int = 1;
    pub const SEMANTIC_PROMPT_INPUT_START: c_int = 2;
    pub const SEMANTIC_PROMPT_OUTPUT_START: c_int = 3;
    pub const SEMANTIC_PROMPT_COMMAND_END: c_int = 4;

    // Prompts (`GhosttySemanticPromptPromptKind`).
    pub const SEMANTIC_PROMPT_PROMPT_PRIMARY: c_int = 0;
    pub const SEMANTIC_PROMPT_PROMPT_RIGHT: c_int = 1;
    pub const SEMANTIC_PROMPT_PROMPT_CONTINUATION: c_int = 2;
    pub const SEMANTIC_PROMPT_PROMPT_SECONDARY: c_int = 3;

    // Window resize requests (`GhosttyTerminalResizeWindowTag`).
    pub const TERMINAL_RESIZE_WINDOW_GRID: c_int = 0;

    // Paste sources (`GhosttyPasteSource`).
    pub const PASTE_SOURCE_CLIPBOARD: c_int = 0;
    pub const PASTE_SOURCE_TEXT: c_int = 1;

    // Formats (`GhosttyFormatterFormat`).
    pub const FORMATTER_FORMAT_PLAIN: c_int = 0;
    pub const FORMATTER_FORMAT_VT: c_int = 1;
    pub const FORMATTER_FORMAT_HTML: c_int = 2;

    // Snapshot decoder options (`GhosttySnapshotDecoderOption`).
    pub const SNAPSHOT_DECODER_OPT_MAX_CONTINUATION_BYTES: c_int = 0;
    pub const SNAPSHOT_DECODER_OPT_COMPRESS_HISTORY: c_int = 2;

    // Snapshot decoder data (`GhosttySnapshotDecoderData`).
    pub const SNAPSHOT_DECODER_DATA_PROGRESS_ROWS: c_int = 6;
    pub const SNAPSHOT_DECODER_DATA_PROGRESS_KIND: c_int = 11;
    pub const SNAPSHOT_DECODER_DATA_PROGRESS_IMAGE_ID: c_int = 12;
    pub const SNAPSHOT_DECODER_DATA_PROGRESS_IMAGE_BYTES: c_int = 14;

    // Snapshot progress (`GhosttySnapshotDecoderProgressKind`).
    pub const SNAPSHOT_DECODER_PROGRESS_KIND_PAGE: c_int = 0;
    pub const SNAPSHOT_DECODER_PROGRESS_KIND_IMAGE: c_int = 1;

    // Key actions (`GhosttyKeyAction`).
    pub const KEY_ACTION_RELEASE: c_int = 0;
    pub const KEY_ACTION_PRESS: c_int = 1;
    pub const KEY_ACTION_REPEAT: c_int = 2;

    // Mouse actions (`GhosttyMouseAction`).
    pub const MOUSE_ACTION_PRESS: c_int = 0;
    pub const MOUSE_ACTION_RELEASE: c_int = 1;
    pub const MOUSE_ACTION_MOTION: c_int = 2;

    // Mouse buttons (`GhosttyMouseButton`).
    pub const MOUSE_BUTTON_UNKNOWN: c_int = 0;
    pub const MOUSE_BUTTON_LEFT: c_int = 1;
    pub const MOUSE_BUTTON_RIGHT: c_int = 2;
    pub const MOUSE_BUTTON_MIDDLE: c_int = 3;
    pub const MOUSE_BUTTON_FOUR: c_int = 4;
    pub const MOUSE_BUTTON_FIVE: c_int = 5;
    pub const MOUSE_BUTTON_SIX: c_int = 6;
    pub const MOUSE_BUTTON_SEVEN: c_int = 7;
    pub const MOUSE_BUTTON_EIGHT: c_int = 8;
    pub const MOUSE_BUTTON_NINE: c_int = 9;
    pub const MOUSE_BUTTON_TEN: c_int = 10;
    pub const MOUSE_BUTTON_ELEVEN: c_int = 11;

    // Mouse encoder options (`GhosttyMouseEncoderOption`).
    pub const MOUSE_ENCODER_OPT_SIZE: c_int = 2;
    pub const MOUSE_ENCODER_OPT_ANY_BUTTON_PRESSED: c_int = 3;
    pub const MOUSE_ENCODER_OPT_TRACK_LAST_CELL: c_int = 4;

    // Focus (`GhosttyFocusEvent`).
    pub const FOCUS_GAINED: c_int = 0;
    pub const FOCUS_LOST: c_int = 1;

    // Render state data (`GhosttyRenderStateData`).
    pub const RENDER_STATE_DATA_COLS: c_int = 1;
    pub const RENDER_STATE_DATA_ROWS: c_int = 2;
    pub const RENDER_STATE_DATA_DIRTY: c_int = 3;
    pub const RENDER_STATE_DATA_ROW_ITERATOR: c_int = 4;
    pub const RENDER_STATE_DATA_CURSOR: c_int = 18;
    pub const RENDER_STATE_DATA_COLORS: c_int = 19;
    pub const RENDER_STATE_DATA_IMAGE_ITERATOR: c_int = 22;
    pub const RENDER_STATE_DATA_IMAGE_PLACEMENTS_GENERATION: c_int = 23;

    // Render dirtiness (`GhosttyRenderStateDirty`).
    pub const RENDER_STATE_DIRTY_FALSE: c_int = 0;
    pub const RENDER_STATE_DIRTY_PARTIAL: c_int = 1;
    pub const RENDER_STATE_DIRTY_FULL: c_int = 2;

    // Cursor shapes (`GhosttyRenderStateCursorVisualStyle`).
    pub const RENDER_STATE_CURSOR_VISUAL_STYLE_BAR: c_int = 0;
    pub const RENDER_STATE_CURSOR_VISUAL_STYLE_BLOCK: c_int = 1;
    pub const RENDER_STATE_CURSOR_VISUAL_STYLE_UNDERLINE: c_int = 2;
    pub const RENDER_STATE_CURSOR_VISUAL_STYLE_BLOCK_HOLLOW: c_int = 3;

    // Render rows (`GhosttyRenderStateRowData`).
    pub const RENDER_STATE_ROW_DATA_DIRTY: c_int = 1;
    pub const RENDER_STATE_ROW_DATA_CELLS: c_int = 3;
    pub const RENDER_STATE_ROW_DATA_SELECTION: c_int = 4;
    pub const RENDER_STATE_ROW_DATA_VIEWPORT_Y: c_int = 6;

    // Render cells (`GhosttyRenderStateRowCellsData`).
    pub const RENDER_STATE_ROW_CELLS_DATA_RAW: c_int = 1;
    pub const RENDER_STATE_ROW_CELLS_DATA_STYLE: c_int = 2;
    pub const RENDER_STATE_ROW_CELLS_DATA_BG_COLOR: c_int = 5;
    pub const RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_UTF8: c_int = 9;

    // Render images (`GhosttyRenderStateImageData`).
    pub const RENDER_STATE_IMAGE_DATA_PROTOCOL: c_int = 1;
    pub const RENDER_STATE_IMAGE_DATA_IMAGE_ID: c_int = 2;
    pub const RENDER_STATE_IMAGE_DATA_IMAGE_GENERATION: c_int = 3;
    pub const RENDER_STATE_IMAGE_DATA_LAYER: c_int = 4;
    pub const RENDER_STATE_IMAGE_DATA_Z: c_int = 5;
    pub const RENDER_STATE_IMAGE_DATA_VIEWPORT_COL: c_int = 6;
    pub const RENDER_STATE_IMAGE_DATA_VIEWPORT_ROW: c_int = 7;
    pub const RENDER_STATE_IMAGE_DATA_GRID_COLS: c_int = 8;
    pub const RENDER_STATE_IMAGE_DATA_GRID_ROWS: c_int = 9;
    pub const RENDER_STATE_IMAGE_DATA_CELL_OFFSET_X: c_int = 10;
    pub const RENDER_STATE_IMAGE_DATA_CELL_OFFSET_Y: c_int = 11;
    pub const RENDER_STATE_IMAGE_DATA_PIXEL_WIDTH: c_int = 12;
    pub const RENDER_STATE_IMAGE_DATA_PIXEL_HEIGHT: c_int = 13;
    pub const RENDER_STATE_IMAGE_DATA_SOURCE_X: c_int = 14;
    pub const RENDER_STATE_IMAGE_DATA_SOURCE_Y: c_int = 15;
    pub const RENDER_STATE_IMAGE_DATA_SOURCE_WIDTH: c_int = 16;
    pub const RENDER_STATE_IMAGE_DATA_SOURCE_HEIGHT: c_int = 17;

    // Image protocols (`GhosttyImageProtocol`).
    pub const IMAGE_PROTOCOL_KITTY: c_int = 0;

    // Image layers (`GhosttyRenderStateImageLayer`).
    pub const RENDER_STATE_IMAGE_LAYER_BELOW_BACKGROUND: c_int = 0;
    pub const RENDER_STATE_IMAGE_LAYER_BELOW_TEXT: c_int = 1;
    pub const RENDER_STATE_IMAGE_LAYER_ABOVE_TEXT: c_int = 2;

    // Kitty images (`GhosttyKittyGraphicsImageData`).
    pub const KITTY_IMAGE_DATA_WIDTH: c_int = 3;
    pub const KITTY_IMAGE_DATA_HEIGHT: c_int = 4;
    pub const KITTY_IMAGE_DATA_FORMAT: c_int = 5;
    pub const KITTY_IMAGE_DATA_DATA_PTR: c_int = 7;
    pub const KITTY_IMAGE_DATA_DATA_LEN: c_int = 8;
    pub const KITTY_IMAGE_DATA_GENERATION: c_int = 9;

    // Kitty pixel formats (`GhosttyKittyImageFormat`).
    pub const KITTY_IMAGE_FORMAT_RGB: c_int = 0;
    pub const KITTY_IMAGE_FORMAT_RGBA: c_int = 1;
    pub const KITTY_IMAGE_FORMAT_PNG: c_int = 2;
    pub const KITTY_IMAGE_FORMAT_GRAY_ALPHA: c_int = 3;
    pub const KITTY_IMAGE_FORMAT_GRAY: c_int = 4;

    // Selection gestures (`GhosttySelectionGestureEventType`).
    pub const SELECTION_GESTURE_EVENT_TYPE_PRESS: c_int = 0;
    pub const SELECTION_GESTURE_EVENT_TYPE_RELEASE: c_int = 1;
    pub const SELECTION_GESTURE_EVENT_TYPE_DRAG: c_int = 2;

    // Selection gesture options (`GhosttySelectionGestureEventOption`).
    pub const SELECTION_GESTURE_EVENT_OPT_REF: c_int = 0;
    pub const SELECTION_GESTURE_EVENT_OPT_POSITION: c_int = 1;
    pub const SELECTION_GESTURE_EVENT_OPT_REPEAT_DISTANCE: c_int = 2;
    pub const SELECTION_GESTURE_EVENT_OPT_TIME_NS: c_int = 3;
    pub const SELECTION_GESTURE_EVENT_OPT_REPEAT_INTERVAL_NS: c_int = 4;
    pub const SELECTION_GESTURE_EVENT_OPT_BEHAVIORS: c_int = 6;
    pub const SELECTION_GESTURE_EVENT_OPT_RECTANGLE: c_int = 7;
    pub const SELECTION_GESTURE_EVENT_OPT_GEOMETRY: c_int = 8;

    // Selection behaviours (`GhosttySelectionGestureBehavior`).
    pub const SELECTION_GESTURE_BEHAVIOR_CELL: c_int = 0;
    pub const SELECTION_GESTURE_BEHAVIOR_WORD: c_int = 1;
    pub const SELECTION_GESTURE_BEHAVIOR_LINE: c_int = 2;
    pub const SELECTION_GESTURE_BEHAVIOR_OUTPUT: c_int = 3;

    // Search data (`GhosttySearchData`).
    pub const SEARCH_DATA_STATUS: c_int = 0;
    pub const SEARCH_DATA_TOTAL_MATCHES: c_int = 2;
    pub const SEARCH_DATA_SELECTED_INDEX: c_int = 3;
    pub const SEARCH_DATA_SELECTED_MATCH: c_int = 4;
    pub const SEARCH_DATA_VIEWPORT_MATCHES: c_int = 6;

    // Search options (`GhosttySearchOption`).
    pub const SEARCH_OPT_NEEDLE: c_int = 0;
    pub const SEARCH_OPT_SELECT_NEXT: c_int = 1;
    pub const SEARCH_OPT_SELECT_PREV: c_int = 2;

    // Search status (`GhosttySearchStatus`).
    pub const SEARCH_STATUS_RUNNING: c_int = 0;
    pub const SEARCH_STATUS_FEED_REQUIRED: c_int = 1;
    pub const SEARCH_STATUS_COMPLETE: c_int = 2;

    // Build information (`GhosttyBuildInfo`).
    pub const BUILD_INFO_VERSION_STRING: c_int = 5;

    // Physical keys: the W3C `code` of each (`GhosttyKey`).
    pub const KEY_UNIDENTIFIED: c_int = 0;
    pub const KEY_BACKQUOTE: c_int = 1;
    pub const KEY_BACKSLASH: c_int = 2;
    pub const KEY_BRACKET_LEFT: c_int = 3;
    pub const KEY_BRACKET_RIGHT: c_int = 4;
    pub const KEY_COMMA: c_int = 5;
    pub const KEY_DIGIT_0: c_int = 6;
    pub const KEY_DIGIT_1: c_int = 7;
    pub const KEY_DIGIT_2: c_int = 8;
    pub const KEY_DIGIT_3: c_int = 9;
    pub const KEY_DIGIT_4: c_int = 10;
    pub const KEY_DIGIT_5: c_int = 11;
    pub const KEY_DIGIT_6: c_int = 12;
    pub const KEY_DIGIT_7: c_int = 13;
    pub const KEY_DIGIT_8: c_int = 14;
    pub const KEY_DIGIT_9: c_int = 15;
    pub const KEY_EQUAL: c_int = 16;
    pub const KEY_INTL_BACKSLASH: c_int = 17;
    pub const KEY_INTL_RO: c_int = 18;
    pub const KEY_INTL_YEN: c_int = 19;
    pub const KEY_A: c_int = 20;
    pub const KEY_B: c_int = 21;
    pub const KEY_C: c_int = 22;
    pub const KEY_D: c_int = 23;
    pub const KEY_E: c_int = 24;
    pub const KEY_F: c_int = 25;
    pub const KEY_G: c_int = 26;
    pub const KEY_H: c_int = 27;
    pub const KEY_I: c_int = 28;
    pub const KEY_J: c_int = 29;
    pub const KEY_K: c_int = 30;
    pub const KEY_L: c_int = 31;
    pub const KEY_M: c_int = 32;
    pub const KEY_N: c_int = 33;
    pub const KEY_O: c_int = 34;
    pub const KEY_P: c_int = 35;
    pub const KEY_Q: c_int = 36;
    pub const KEY_R: c_int = 37;
    pub const KEY_S: c_int = 38;
    pub const KEY_T: c_int = 39;
    pub const KEY_U: c_int = 40;
    pub const KEY_V: c_int = 41;
    pub const KEY_W: c_int = 42;
    pub const KEY_X: c_int = 43;
    pub const KEY_Y: c_int = 44;
    pub const KEY_Z: c_int = 45;
    pub const KEY_MINUS: c_int = 46;
    pub const KEY_PERIOD: c_int = 47;
    pub const KEY_QUOTE: c_int = 48;
    pub const KEY_SEMICOLON: c_int = 49;
    pub const KEY_SLASH: c_int = 50;
    pub const KEY_ALT_LEFT: c_int = 51;
    pub const KEY_ALT_RIGHT: c_int = 52;
    pub const KEY_BACKSPACE: c_int = 53;
    pub const KEY_CAPS_LOCK: c_int = 54;
    pub const KEY_CONTEXT_MENU: c_int = 55;
    pub const KEY_CONTROL_LEFT: c_int = 56;
    pub const KEY_CONTROL_RIGHT: c_int = 57;
    pub const KEY_ENTER: c_int = 58;
    pub const KEY_META_LEFT: c_int = 59;
    pub const KEY_META_RIGHT: c_int = 60;
    pub const KEY_SHIFT_LEFT: c_int = 61;
    pub const KEY_SHIFT_RIGHT: c_int = 62;
    pub const KEY_SPACE: c_int = 63;
    pub const KEY_TAB: c_int = 64;
    pub const KEY_CONVERT: c_int = 65;
    pub const KEY_KANA_MODE: c_int = 66;
    pub const KEY_NON_CONVERT: c_int = 67;
    pub const KEY_DELETE: c_int = 68;
    pub const KEY_END: c_int = 69;
    pub const KEY_HELP: c_int = 70;
    pub const KEY_HOME: c_int = 71;
    pub const KEY_INSERT: c_int = 72;
    pub const KEY_PAGE_DOWN: c_int = 73;
    pub const KEY_PAGE_UP: c_int = 74;
    pub const KEY_ARROW_DOWN: c_int = 75;
    pub const KEY_ARROW_LEFT: c_int = 76;
    pub const KEY_ARROW_RIGHT: c_int = 77;
    pub const KEY_ARROW_UP: c_int = 78;
    pub const KEY_NUM_LOCK: c_int = 79;
    pub const KEY_NUMPAD_0: c_int = 80;
    pub const KEY_NUMPAD_1: c_int = 81;
    pub const KEY_NUMPAD_2: c_int = 82;
    pub const KEY_NUMPAD_3: c_int = 83;
    pub const KEY_NUMPAD_4: c_int = 84;
    pub const KEY_NUMPAD_5: c_int = 85;
    pub const KEY_NUMPAD_6: c_int = 86;
    pub const KEY_NUMPAD_7: c_int = 87;
    pub const KEY_NUMPAD_8: c_int = 88;
    pub const KEY_NUMPAD_9: c_int = 89;
    pub const KEY_NUMPAD_ADD: c_int = 90;
    pub const KEY_NUMPAD_BACKSPACE: c_int = 91;
    pub const KEY_NUMPAD_CLEAR: c_int = 92;
    pub const KEY_NUMPAD_CLEAR_ENTRY: c_int = 93;
    pub const KEY_NUMPAD_COMMA: c_int = 94;
    pub const KEY_NUMPAD_DECIMAL: c_int = 95;
    pub const KEY_NUMPAD_DIVIDE: c_int = 96;
    pub const KEY_NUMPAD_ENTER: c_int = 97;
    pub const KEY_NUMPAD_EQUAL: c_int = 98;
    pub const KEY_NUMPAD_MEMORY_ADD: c_int = 99;
    pub const KEY_NUMPAD_MEMORY_CLEAR: c_int = 100;
    pub const KEY_NUMPAD_MEMORY_RECALL: c_int = 101;
    pub const KEY_NUMPAD_MEMORY_STORE: c_int = 102;
    pub const KEY_NUMPAD_MEMORY_SUBTRACT: c_int = 103;
    pub const KEY_NUMPAD_MULTIPLY: c_int = 104;
    pub const KEY_NUMPAD_PAREN_LEFT: c_int = 105;
    pub const KEY_NUMPAD_PAREN_RIGHT: c_int = 106;
    pub const KEY_NUMPAD_SUBTRACT: c_int = 107;
    pub const KEY_NUMPAD_SEPARATOR: c_int = 108;
    pub const KEY_NUMPAD_UP: c_int = 109;
    pub const KEY_NUMPAD_DOWN: c_int = 110;
    pub const KEY_NUMPAD_RIGHT: c_int = 111;
    pub const KEY_NUMPAD_LEFT: c_int = 112;
    pub const KEY_NUMPAD_BEGIN: c_int = 113;
    pub const KEY_NUMPAD_HOME: c_int = 114;
    pub const KEY_NUMPAD_END: c_int = 115;
    pub const KEY_NUMPAD_INSERT: c_int = 116;
    pub const KEY_NUMPAD_DELETE: c_int = 117;
    pub const KEY_NUMPAD_PAGE_UP: c_int = 118;
    pub const KEY_NUMPAD_PAGE_DOWN: c_int = 119;
    pub const KEY_ESCAPE: c_int = 120;
    pub const KEY_F1: c_int = 121;
    pub const KEY_F2: c_int = 122;
    pub const KEY_F3: c_int = 123;
    pub const KEY_F4: c_int = 124;
    pub const KEY_F5: c_int = 125;
    pub const KEY_F6: c_int = 126;
    pub const KEY_F7: c_int = 127;
    pub const KEY_F8: c_int = 128;
    pub const KEY_F9: c_int = 129;
    pub const KEY_F10: c_int = 130;
    pub const KEY_F11: c_int = 131;
    pub const KEY_F12: c_int = 132;
    pub const KEY_F13: c_int = 133;
    pub const KEY_F14: c_int = 134;
    pub const KEY_F15: c_int = 135;
    pub const KEY_F16: c_int = 136;
    pub const KEY_F17: c_int = 137;
    pub const KEY_F18: c_int = 138;
    pub const KEY_F19: c_int = 139;
    pub const KEY_F20: c_int = 140;
    pub const KEY_F21: c_int = 141;
    pub const KEY_F22: c_int = 142;
    pub const KEY_F23: c_int = 143;
    pub const KEY_F24: c_int = 144;
    pub const KEY_F25: c_int = 145;
    pub const KEY_FN: c_int = 146;
    pub const KEY_FN_LOCK: c_int = 147;
    pub const KEY_PRINT_SCREEN: c_int = 148;
    pub const KEY_SCROLL_LOCK: c_int = 149;
    pub const KEY_PAUSE: c_int = 150;
    pub const KEY_BROWSER_BACK: c_int = 151;
    pub const KEY_BROWSER_FAVORITES: c_int = 152;
    pub const KEY_BROWSER_FORWARD: c_int = 153;
    pub const KEY_BROWSER_HOME: c_int = 154;
    pub const KEY_BROWSER_REFRESH: c_int = 155;
    pub const KEY_BROWSER_SEARCH: c_int = 156;
    pub const KEY_BROWSER_STOP: c_int = 157;
    pub const KEY_EJECT: c_int = 158;
    pub const KEY_LAUNCH_APP_1: c_int = 159;
    pub const KEY_LAUNCH_APP_2: c_int = 160;
    pub const KEY_LAUNCH_MAIL: c_int = 161;
    pub const KEY_MEDIA_PLAY_PAUSE: c_int = 162;
    pub const KEY_MEDIA_SELECT: c_int = 163;
    pub const KEY_MEDIA_STOP: c_int = 164;
    pub const KEY_MEDIA_TRACK_NEXT: c_int = 165;
    pub const KEY_MEDIA_TRACK_PREVIOUS: c_int = 166;
    pub const KEY_POWER: c_int = 167;
    pub const KEY_SLEEP: c_int = 168;
    pub const KEY_AUDIO_VOLUME_DOWN: c_int = 169;
    pub const KEY_AUDIO_VOLUME_MUTE: c_int = 170;
    pub const KEY_AUDIO_VOLUME_UP: c_int = 171;
    pub const KEY_WAKE_UP: c_int = 172;
    pub const KEY_COPY: c_int = 173;
    pub const KEY_CUT: c_int = 174;
    pub const KEY_PASTE: c_int = 175;

    // The DEC private modes this crate names (`GHOSTTY_MODE_*`, packed by
    // `ghostty_mode_new(value, false)`: the number, bit 15 clear).
    pub const MODE_DECCKM: u16 = 1;
    pub const MODE_CURSOR_BLINKING: u16 = 12;
    pub const MODE_CURSOR_VISIBLE: u16 = 25;
    pub const MODE_NORMAL_MOUSE: u16 = 1000;
    pub const MODE_BUTTON_MOUSE: u16 = 1002;
    pub const MODE_ANY_MOUSE: u16 = 1003;
    pub const MODE_FOCUS_EVENT: u16 = 1004;
    pub const MODE_ALT_SCROLL: u16 = 1007;
    pub const MODE_ALT_SCREEN_SAVE: u16 = 1049;
    pub const MODE_BRACKETED_PASTE: u16 = 2004;
    pub const MODE_SYNC_OUTPUT: u16 = 2026;
    pub const MODE_GRAPHEME_CLUSTER: u16 = 2027;
    pub const MODE_COLOR_SCHEME_REPORT: u16 = 2031;
    pub const MODE_IN_BAND_RESIZE: u16 = 2048;

    // Device attributes (`device.h`).
    pub const DA_CONFORMANCE_VT220: u16 = 62;
    pub const DA_FEATURE_SIXEL: u16 = 4;
    pub const DA_FEATURE_ANSI_COLOR: u16 = 22;
    pub const DA_FEATURE_CLIPBOARD: u16 = 52;
    pub const DA_DEVICE_TYPE_VT220: u16 = 1;

    // Modifier bits (`key/event.h`).
    pub const MODS_SHIFT: u16 = 1;
    pub const MODS_CTRL: u16 = 2;
    pub const MODS_ALT: u16 = 4;
    pub const MODS_SUPER: u16 = 8;
    pub const MODS_CAPS_LOCK: u16 = 16;
    pub const MODS_NUM_LOCK: u16 = 32;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct String {
        pub ptr: *const u8,
        pub len: usize,
    }
    const _: () = assert!(size_of::<String>() == 16);
    const _: () = assert!(std::mem::offset_of!(String, len) == 8);

    #[repr(C)]
    pub struct Buffer {
        pub ptr: *mut u8,
        pub cap: usize,
        pub len: usize,
    }
    const _: () = assert!(size_of::<Buffer>() == 24);
    const _: () = assert!(std::mem::offset_of!(Buffer, len) == 16);

    #[repr(C)]
    #[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub struct ColorRgb {
        pub r: u8,
        pub g: u8,
        pub b: u8,
    }
    const _: () = assert!(size_of::<ColorRgb>() == 3);

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct PointCoordinate {
        pub x: u16,
        pub y: u32,
    }
    const _: () = assert!(size_of::<PointCoordinate>() == 8);
    const _: () = assert!(std::mem::offset_of!(PointCoordinate, y) == 4);

    /// A UNION, and the first binding's first mistake: sixteen bytes, of which
    /// the coordinate uses the first eight and the rest is room upstream kept
    /// for the ABI. A flat `{ tag, x, y }` shifts every field after the tag.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union PointValue {
        pub coordinate: PointCoordinate,
        pub _padding: [u64; 2],
    }
    const _: () = assert!(size_of::<PointValue>() == 16);

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Point {
        pub tag: c_int,
        pub value: PointValue,
    }
    const _: () = assert!(size_of::<Point>() == 24);
    const _: () = assert!(std::mem::offset_of!(Point, value) == 8);

    /// A SIZED struct: `size` says which version the caller was compiled
    /// against, and a zeroed one claims to be zero bytes long — the library
    /// believes it, and the result is a `SIGSEGV` with no message.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct GridRef {
        pub size: usize,
        pub node: *mut c_void,
        pub x: u16,
        pub y: u16,
    }
    const _: () = assert!(size_of::<GridRef>() == 24);
    const _: () = assert!(std::mem::offset_of!(GridRef, node) == 8);
    const _: () = assert!(std::mem::offset_of!(GridRef, x) == 16);
    const _: () = assert!(std::mem::offset_of!(GridRef, y) == 18);

    impl GridRef {
        pub const fn sized() -> Self {
            Self {
                size: size_of::<Self>(),
                node: std::ptr::null_mut(),
                x: 0,
                y: 0,
            }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union StyleColorValue {
        pub palette: u8,
        pub rgb: ColorRgb,
        pub _padding: u64,
    }
    const _: () = assert!(size_of::<StyleColorValue>() == 8);

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct StyleColor {
        pub tag: c_int,
        pub value: StyleColorValue,
    }
    const _: () = assert!(size_of::<StyleColor>() == 16);
    const _: () = assert!(std::mem::offset_of!(StyleColor, value) == 8);

    #[repr(C)]
    #[derive(Clone, Copy)]
    #[allow(clippy::struct_excessive_bools)]
    pub struct Style {
        pub size: usize,
        pub fg_color: StyleColor,
        pub bg_color: StyleColor,
        pub underline_color: StyleColor,
        pub bold: bool,
        pub italic: bool,
        pub faint: bool,
        pub blink: bool,
        pub inverse: bool,
        pub invisible: bool,
        pub strikethrough: bool,
        pub overline: bool,
        pub underline: c_int,
    }
    const _: () = assert!(size_of::<Style>() == 72);
    const _: () = assert!(std::mem::offset_of!(Style, fg_color) == 8);
    const _: () = assert!(std::mem::offset_of!(Style, bg_color) == 24);
    const _: () = assert!(std::mem::offset_of!(Style, underline_color) == 40);
    const _: () = assert!(std::mem::offset_of!(Style, bold) == 56);
    const _: () = assert!(std::mem::offset_of!(Style, overline) == 63);
    const _: () = assert!(std::mem::offset_of!(Style, underline) == 64);

    impl Style {
        pub const fn sized() -> Self {
            let none = StyleColor {
                tag: STYLE_COLOR_NONE,
                value: StyleColorValue { _padding: 0 },
            };
            Self {
                size: size_of::<Self>(),
                fg_color: none,
                bg_color: none,
                underline_color: none,
                bold: false,
                italic: false,
                faint: false,
                blink: false,
                inverse: false,
                invisible: false,
                strikethrough: false,
                overline: false,
                underline: SGR_UNDERLINE_NONE,
            }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct TerminalModeConfig {
        pub mode: u16,
        pub value: bool,
    }
    const _: () = assert!(size_of::<TerminalModeConfig>() == 4);
    const _: () = assert!(std::mem::offset_of!(TerminalModeConfig, value) == 2);

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct TerminalScrollbar {
        pub total: u64,
        pub offset: u64,
        pub len: u64,
    }
    const _: () = assert!(size_of::<TerminalScrollbar>() == 24);
    const _: () = assert!(std::mem::offset_of!(TerminalScrollbar, len) == 16);

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub union TerminalScrollViewportValue {
        pub delta: isize,
        pub row: usize,
        pub delta_prompt: isize,
        pub _padding: [u64; 2],
    }
    const _: () = assert!(size_of::<TerminalScrollViewportValue>() == 16);

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct TerminalScrollViewport {
        pub tag: c_int,
        pub value: TerminalScrollViewportValue,
    }
    const _: () = assert!(size_of::<TerminalScrollViewport>() == 24);
    const _: () = assert!(std::mem::offset_of!(TerminalScrollViewport, value) == 8);

    #[repr(C)]
    pub struct DeviceAttributesPrimary {
        pub conformance_level: u16,
        pub features: [u16; 64],
        pub num_features: usize,
    }
    const _: () = assert!(size_of::<DeviceAttributesPrimary>() == 144);
    const _: () = assert!(std::mem::offset_of!(DeviceAttributesPrimary, features) == 2);
    const _: () = assert!(std::mem::offset_of!(DeviceAttributesPrimary, num_features) == 136);

    #[repr(C)]
    pub struct DeviceAttributesSecondary {
        pub device_type: u16,
        pub firmware_version: u16,
        pub rom_cartridge: u16,
    }
    const _: () = assert!(size_of::<DeviceAttributesSecondary>() == 6);

    #[repr(C)]
    pub struct DeviceAttributesTertiary {
        pub unit_id: u32,
    }
    const _: () = assert!(size_of::<DeviceAttributesTertiary>() == 4);

    #[repr(C)]
    pub struct DeviceAttributes {
        pub primary: DeviceAttributesPrimary,
        pub secondary: DeviceAttributesSecondary,
        pub tertiary: DeviceAttributesTertiary,
    }
    const _: () = assert!(size_of::<DeviceAttributes>() == 160);
    const _: () = assert!(std::mem::offset_of!(DeviceAttributes, secondary) == 144);
    const _: () = assert!(std::mem::offset_of!(DeviceAttributes, tertiary) == 152);

    #[repr(C)]
    pub struct SizeReportSize {
        pub rows: u16,
        pub columns: u16,
        pub cell_width: u32,
        pub cell_height: u32,
    }
    const _: () = assert!(size_of::<SizeReportSize>() == 12);
    const _: () = assert!(std::mem::offset_of!(SizeReportSize, cell_width) == 4);
    const _: () = assert!(std::mem::offset_of!(SizeReportSize, cell_height) == 8);

    #[repr(C)]
    pub struct TerminalDesktopNotification {
        pub size: usize,
        pub title: String,
        pub body: String,
    }
    const _: () = assert!(size_of::<TerminalDesktopNotification>() == 40);
    const _: () = assert!(std::mem::offset_of!(TerminalDesktopNotification, body) == 24);

    #[repr(C)]
    pub struct TerminalProgressReport {
        pub size: usize,
        pub state: c_int,
        pub progress: i8,
    }
    const _: () = assert!(size_of::<TerminalProgressReport>() == 16);
    const _: () = assert!(std::mem::offset_of!(TerminalProgressReport, progress) == 12);

    #[repr(C)]
    pub struct TerminalProgramStatus {
        pub size: usize,
        pub state: c_int,
        pub kind: c_int,
        pub progress: i8,
        pub id: String,
        pub app: String,
        pub title: String,
        pub message: String,
    }
    const _: () = assert!(size_of::<TerminalProgramStatus>() == 88);
    const _: () = assert!(std::mem::offset_of!(TerminalProgramStatus, kind) == 12);
    const _: () = assert!(std::mem::offset_of!(TerminalProgramStatus, progress) == 16);
    const _: () = assert!(std::mem::offset_of!(TerminalProgramStatus, id) == 24);
    const _: () = assert!(std::mem::offset_of!(TerminalProgramStatus, app) == 40);
    const _: () = assert!(std::mem::offset_of!(TerminalProgramStatus, title) == 56);
    const _: () = assert!(std::mem::offset_of!(TerminalProgramStatus, message) == 72);

    #[repr(C)]
    pub struct TerminalSemanticPrompt {
        pub size: usize,
        pub kind: c_int,
        pub prompt_kind: c_int,
        pub has_exit_code: bool,
        pub exit_code: i32,
        pub command: String,
        pub error: String,
    }
    const _: () = assert!(size_of::<TerminalSemanticPrompt>() == 56);
    const _: () = assert!(std::mem::offset_of!(TerminalSemanticPrompt, has_exit_code) == 16);
    const _: () = assert!(std::mem::offset_of!(TerminalSemanticPrompt, exit_code) == 20);
    const _: () = assert!(std::mem::offset_of!(TerminalSemanticPrompt, command) == 24);
    const _: () = assert!(std::mem::offset_of!(TerminalSemanticPrompt, error) == 40);

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct TerminalResizeWindowGrid {
        pub rows: u16,
        pub columns: u16,
    }
    const _: () = assert!(size_of::<TerminalResizeWindowGrid>() == 4);

    #[repr(C)]
    pub union TerminalResizeWindowValue {
        pub grid: TerminalResizeWindowGrid,
        pub _padding: [u64; 2],
    }
    const _: () = assert!(size_of::<TerminalResizeWindowValue>() == 16);

    #[repr(C)]
    pub struct TerminalResizeWindow {
        pub tag: c_int,
        pub value: TerminalResizeWindowValue,
    }
    const _: () = assert!(size_of::<TerminalResizeWindow>() == 24);
    const _: () = assert!(std::mem::offset_of!(TerminalResizeWindow, value) == 8);

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct ClipboardContent {
        pub mime: String,
        pub data: String,
    }
    const _: () = assert!(size_of::<ClipboardContent>() == 32);
    const _: () = assert!(std::mem::offset_of!(ClipboardContent, data) == 16);

    #[repr(C)]
    pub struct ClipboardWriteReply {
        pub size: usize,
        pub result: c_int,
        pub remember: bool,
    }
    const _: () = assert!(size_of::<ClipboardWriteReply>() == 16);
    const _: () = assert!(std::mem::offset_of!(ClipboardWriteReply, remember) == 12);

    #[repr(C)]
    pub struct ClipboardWrite {
        pub size: usize,
        pub location: c_int,
        pub contents: *const ClipboardContent,
        pub contents_len: usize,
        pub name: String,
        pub granted: bool,
        pub can_remember: bool,
        pub ctx: *const c_void,
        pub reply: Option<unsafe extern "C" fn(*const ClipboardWrite, *const ClipboardWriteReply)>,
    }
    const _: () = assert!(size_of::<ClipboardWrite>() == 72);
    const _: () = assert!(std::mem::offset_of!(ClipboardWrite, contents) == 16);
    const _: () = assert!(std::mem::offset_of!(ClipboardWrite, contents_len) == 24);
    const _: () = assert!(std::mem::offset_of!(ClipboardWrite, name) == 32);
    const _: () = assert!(std::mem::offset_of!(ClipboardWrite, granted) == 48);
    const _: () = assert!(std::mem::offset_of!(ClipboardWrite, ctx) == 56);
    const _: () = assert!(std::mem::offset_of!(ClipboardWrite, reply) == 64);

    #[repr(C)]
    pub struct ClipboardReadReply {
        pub size: usize,
        pub result: c_int,
        pub contents: *const ClipboardContent,
        pub contents_len: usize,
        pub available: *const String,
        pub available_len: usize,
        pub remember: bool,
    }
    const _: () = assert!(size_of::<ClipboardReadReply>() == 56);
    const _: () = assert!(std::mem::offset_of!(ClipboardReadReply, contents) == 16);
    const _: () = assert!(std::mem::offset_of!(ClipboardReadReply, available) == 32);
    const _: () = assert!(std::mem::offset_of!(ClipboardReadReply, remember) == 48);

    #[repr(C)]
    pub struct ClipboardRead {
        pub size: usize,
        pub location: c_int,
        pub mimes: *const String,
        pub mimes_len: usize,
        pub list: bool,
        pub name: String,
        pub granted: bool,
        pub can_remember: bool,
        pub ctx: *const c_void,
        pub reply: Option<unsafe extern "C" fn(*const ClipboardRead, *const ClipboardReadReply)>,
    }
    const _: () = assert!(size_of::<ClipboardRead>() == 80);
    const _: () = assert!(std::mem::offset_of!(ClipboardRead, mimes) == 16);
    const _: () = assert!(std::mem::offset_of!(ClipboardRead, list) == 32);
    const _: () = assert!(std::mem::offset_of!(ClipboardRead, name) == 40);
    const _: () = assert!(std::mem::offset_of!(ClipboardRead, granted) == 56);
    const _: () = assert!(std::mem::offset_of!(ClipboardRead, ctx) == 64);
    const _: () = assert!(std::mem::offset_of!(ClipboardRead, reply) == 72);

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Writer {
        pub write: Option<unsafe extern "C" fn(*mut c_void, *const u8, usize) -> bool>,
        pub userdata: *mut c_void,
    }
    const _: () = assert!(size_of::<Writer>() == 16);
    const _: () = assert!(std::mem::offset_of!(Writer, userdata) == 8);

    #[repr(C)]
    pub struct Reader {
        pub read: Option<unsafe extern "C" fn(*mut c_void, *mut u8, usize, *mut usize) -> bool>,
        pub userdata: *mut c_void,
    }
    const _: () = assert!(size_of::<Reader>() == 16);
    const _: () = assert!(std::mem::offset_of!(Reader, userdata) == 8);

    #[repr(C)]
    pub struct MimeReader {
        pub read: Option<unsafe extern "C" fn(*mut c_void, String, Writer) -> bool>,
        pub userdata: *mut c_void,
    }
    const _: () = assert!(size_of::<MimeReader>() == 16);
    const _: () = assert!(std::mem::offset_of!(MimeReader, userdata) == 8);

    #[repr(C)]
    pub struct Paste {
        pub size: usize,
        pub location: c_int,
        pub source: c_int,
        pub mimes: *const String,
        pub mimes_len: usize,
        pub reader: MimeReader,
        pub allow_unsafe: bool,
    }
    const _: () = assert!(size_of::<Paste>() == 56);
    const _: () = assert!(std::mem::offset_of!(Paste, source) == 12);
    const _: () = assert!(std::mem::offset_of!(Paste, mimes) == 16);
    const _: () = assert!(std::mem::offset_of!(Paste, reader) == 32);
    const _: () = assert!(std::mem::offset_of!(Paste, allow_unsafe) == 48);

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Selection {
        pub size: usize,
        pub start: GridRef,
        pub end: GridRef,
        pub rectangle: bool,
    }
    const _: () = assert!(size_of::<Selection>() == 64);
    const _: () = assert!(std::mem::offset_of!(Selection, start) == 8);
    const _: () = assert!(std::mem::offset_of!(Selection, end) == 32);
    const _: () = assert!(std::mem::offset_of!(Selection, rectangle) == 56);

    impl Selection {
        pub const fn sized() -> Self {
            Self {
                size: size_of::<Self>(),
                start: GridRef::sized(),
                end: GridRef::sized(),
                rectangle: false,
            }
        }
    }

    #[repr(C)]
    pub struct SelectionBuffer {
        pub ptr: *mut Selection,
        pub cap: usize,
        pub len: usize,
    }
    const _: () = assert!(size_of::<SelectionBuffer>() == 24);
    const _: () = assert!(std::mem::offset_of!(SelectionBuffer, len) == 16);

    #[repr(C)]
    pub struct TerminalSelectionFormatOptions {
        pub size: usize,
        pub emit: c_int,
        pub unwrap: bool,
        pub trim: bool,
        pub selection: *const Selection,
    }
    const _: () = assert!(size_of::<TerminalSelectionFormatOptions>() == 24);
    const _: () = assert!(std::mem::offset_of!(TerminalSelectionFormatOptions, unwrap) == 12);
    const _: () = assert!(std::mem::offset_of!(TerminalSelectionFormatOptions, trim) == 13);
    const _: () = assert!(std::mem::offset_of!(TerminalSelectionFormatOptions, selection) == 16);

    #[repr(C)]
    pub struct SelectionGestureBehaviors {
        pub single_click: c_int,
        pub double_click: c_int,
        pub triple_click: c_int,
    }
    const _: () = assert!(size_of::<SelectionGestureBehaviors>() == 12);
    const _: () = assert!(std::mem::offset_of!(SelectionGestureBehaviors, triple_click) == 8);

    #[repr(C)]
    pub struct SelectionGestureGeometry {
        pub columns: u32,
        pub cell_width: u32,
        pub padding_left: u32,
        pub screen_height: u32,
    }
    const _: () = assert!(size_of::<SelectionGestureGeometry>() == 16);
    const _: () = assert!(std::mem::offset_of!(SelectionGestureGeometry, screen_height) == 12);

    #[repr(C)]
    pub struct SurfacePosition {
        pub x: f64,
        pub y: f64,
    }
    const _: () = assert!(size_of::<SurfacePosition>() == 16);
    const _: () = assert!(std::mem::offset_of!(SurfacePosition, y) == 8);

    #[repr(C)]
    pub struct FormatterScreenExtra {
        pub size: usize,
        pub cursor: bool,
        pub style: bool,
        pub hyperlink: bool,
        pub protection: bool,
        pub kitty_keyboard: bool,
        pub charsets: bool,
    }
    const _: () = assert!(size_of::<FormatterScreenExtra>() == 16);
    const _: () = assert!(std::mem::offset_of!(FormatterScreenExtra, cursor) == 8);
    const _: () = assert!(std::mem::offset_of!(FormatterScreenExtra, charsets) == 13);

    #[repr(C)]
    pub struct FormatterTerminalExtra {
        pub size: usize,
        pub palette: bool,
        pub modes: bool,
        pub scrolling_region: bool,
        pub tabstops: bool,
        pub pwd: bool,
        pub keyboard: bool,
        pub screen: FormatterScreenExtra,
    }
    const _: () = assert!(size_of::<FormatterTerminalExtra>() == 32);
    const _: () = assert!(std::mem::offset_of!(FormatterTerminalExtra, palette) == 8);
    const _: () = assert!(std::mem::offset_of!(FormatterTerminalExtra, keyboard) == 13);
    const _: () = assert!(std::mem::offset_of!(FormatterTerminalExtra, screen) == 16);

    #[repr(C)]
    pub struct FormatterTerminalOptions {
        pub size: usize,
        pub emit: c_int,
        pub unwrap: bool,
        pub trim: bool,
        pub extra: FormatterTerminalExtra,
        pub selection: *const Selection,
    }
    const _: () = assert!(size_of::<FormatterTerminalOptions>() == 56);
    const _: () = assert!(std::mem::offset_of!(FormatterTerminalOptions, emit) == 8);
    const _: () = assert!(std::mem::offset_of!(FormatterTerminalOptions, unwrap) == 12);
    const _: () = assert!(std::mem::offset_of!(FormatterTerminalOptions, extra) == 16);
    const _: () = assert!(std::mem::offset_of!(FormatterTerminalOptions, selection) == 48);

    #[repr(C)]
    pub struct MouseEncoderSize {
        pub size: usize,
        pub screen_width: u32,
        pub screen_height: u32,
        pub cell_width: u32,
        pub cell_height: u32,
        pub padding_top: u32,
        pub padding_bottom: u32,
        pub padding_right: u32,
        pub padding_left: u32,
    }
    const _: () = assert!(size_of::<MouseEncoderSize>() == 40);
    const _: () = assert!(std::mem::offset_of!(MouseEncoderSize, screen_width) == 8);
    const _: () = assert!(std::mem::offset_of!(MouseEncoderSize, cell_width) == 16);
    const _: () = assert!(std::mem::offset_of!(MouseEncoderSize, padding_top) == 24);
    const _: () = assert!(std::mem::offset_of!(MouseEncoderSize, padding_left) == 36);

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct MousePosition {
        pub x: f32,
        pub y: f32,
    }
    const _: () = assert!(size_of::<MousePosition>() == 8);
    const _: () = assert!(std::mem::offset_of!(MousePosition, y) == 4);

    #[repr(C)]
    pub struct RenderStateRowSelection {
        pub size: usize,
        pub start_x: u16,
        pub end_x: u16,
    }
    const _: () = assert!(size_of::<RenderStateRowSelection>() == 16);
    const _: () = assert!(std::mem::offset_of!(RenderStateRowSelection, start_x) == 8);
    const _: () = assert!(std::mem::offset_of!(RenderStateRowSelection, end_x) == 10);

    #[repr(C)]
    #[allow(clippy::struct_excessive_bools)]
    pub struct RenderStateCursor {
        pub size: usize,
        pub viewport_has_value: bool,
        pub viewport_x: u16,
        pub viewport_y: u16,
        pub wide_tail: bool,
        pub visible: bool,
        pub blinking: bool,
        pub password_input: bool,
        pub visual_style: c_int,
    }
    const _: () = assert!(size_of::<RenderStateCursor>() == 24);
    const _: () = assert!(std::mem::offset_of!(RenderStateCursor, viewport_has_value) == 8);
    const _: () = assert!(std::mem::offset_of!(RenderStateCursor, viewport_x) == 10);
    const _: () = assert!(std::mem::offset_of!(RenderStateCursor, viewport_y) == 12);
    const _: () = assert!(std::mem::offset_of!(RenderStateCursor, wide_tail) == 14);
    const _: () = assert!(std::mem::offset_of!(RenderStateCursor, visible) == 15);
    const _: () = assert!(std::mem::offset_of!(RenderStateCursor, blinking) == 16);
    const _: () = assert!(std::mem::offset_of!(RenderStateCursor, password_input) == 17);
    const _: () = assert!(std::mem::offset_of!(RenderStateCursor, visual_style) == 20);

    #[repr(C)]
    pub struct RenderStateColors {
        pub size: usize,
        pub background: ColorRgb,
        pub foreground: ColorRgb,
        pub cursor: ColorRgb,
        pub cursor_has_value: bool,
        pub palette: [ColorRgb; 256],
    }
    const _: () = assert!(size_of::<RenderStateColors>() == 792);
    const _: () = assert!(std::mem::offset_of!(RenderStateColors, background) == 8);
    const _: () = assert!(std::mem::offset_of!(RenderStateColors, foreground) == 11);
    const _: () = assert!(std::mem::offset_of!(RenderStateColors, cursor) == 14);
    const _: () = assert!(std::mem::offset_of!(RenderStateColors, cursor_has_value) == 17);
    const _: () = assert!(std::mem::offset_of!(RenderStateColors, palette) == 18);

    // The callbacks a terminal takes. `catch_unwind` in every trampoline:
    // a Rust panic unwinding into Zig is undefined behaviour.
    pub type WritePtyFn = unsafe extern "C" fn(Handle, *mut c_void, *const u8, usize);
    pub type BellFn = unsafe extern "C" fn(Handle, *mut c_void);
    pub type StringFn = unsafe extern "C" fn(Handle, *mut c_void) -> String;
    pub type SizeFn = unsafe extern "C" fn(Handle, *mut c_void, *mut SizeReportSize) -> bool;
    pub type ColorSchemeFn = unsafe extern "C" fn(Handle, *mut c_void, *mut c_int) -> bool;
    pub type DeviceAttributesFn =
        unsafe extern "C" fn(Handle, *mut c_void, *mut DeviceAttributes) -> bool;
    pub type ClipboardWriteFn = unsafe extern "C" fn(Handle, *mut c_void, *const ClipboardWrite);
    pub type ClipboardReadFn = unsafe extern "C" fn(Handle, *mut c_void, *const ClipboardRead);
    pub type NotificationFn =
        unsafe extern "C" fn(Handle, *mut c_void, *const TerminalDesktopNotification);
    pub type ProgressFn = unsafe extern "C" fn(Handle, *mut c_void, *const TerminalProgressReport);
    pub type ProgramStatusFn =
        unsafe extern "C" fn(Handle, *mut c_void, *const TerminalProgramStatus);
    pub type SemanticPromptFn =
        unsafe extern "C" fn(Handle, *mut c_void, *const TerminalSemanticPrompt);
    pub type RenderHoldFn = unsafe extern "C" fn(Handle, *mut c_void, bool);
    pub type ResizeWindowFn =
        unsafe extern "C" fn(Handle, *mut c_void, *const TerminalResizeWindow);

    unsafe extern "C" {
        // terminal.h
        pub fn ghostty_terminal_new(
            allocator: *const c_void,
            terminal: *mut Handle,
            cols: u16,
            rows: u16,
        ) -> c_int;
        pub fn ghostty_terminal_free(terminal: Handle);
        pub fn ghostty_terminal_resize(
            terminal: Handle,
            cols: u16,
            rows: u16,
            cell_width_px: u32,
            cell_height_px: u32,
        ) -> c_int;
        pub fn ghostty_terminal_set(terminal: Handle, option: c_int, value: *const c_void)
        -> c_int;
        pub fn ghostty_terminal_vt_write(terminal: Handle, data: *const u8, len: usize);
        pub fn ghostty_terminal_get(terminal: Handle, data: c_int, out: *mut c_void) -> c_int;
        pub fn ghostty_terminal_grid_ref(
            terminal: Handle,
            point: Point,
            out: *mut GridRef,
        ) -> c_int;
        pub fn ghostty_terminal_point_from_grid_ref(
            terminal: Handle,
            reference: *const GridRef,
            tag: c_int,
            out: *mut PointCoordinate,
        ) -> c_int;
        pub fn ghostty_terminal_scroll_viewport(terminal: Handle, behavior: TerminalScrollViewport);
        pub fn ghostty_terminal_compression_activity(terminal: Handle, out: *mut u64) -> c_int;
        pub fn ghostty_terminal_compress(terminal: Handle, mode: c_int, out: *mut c_int) -> c_int;
        // grid_ref.h, screen.h
        pub fn ghostty_grid_ref_cell(reference: *const GridRef, out: *mut u64) -> c_int;
        pub fn ghostty_grid_ref_graphemes(
            reference: *const GridRef,
            buf: *mut u32,
            buf_len: usize,
            out_len: *mut usize,
        ) -> c_int;
        pub fn ghostty_grid_ref_hyperlink_uri(
            reference: *const GridRef,
            buf: *mut u8,
            buf_len: usize,
            out_len: *mut usize,
        ) -> c_int;
        pub fn ghostty_grid_ref_style(reference: *const GridRef, out: *mut Style) -> c_int;
        pub fn ghostty_cell_get(cell: u64, data: c_int, out: *mut c_void) -> c_int;
        // paste.h, focus.h, color_scheme.h
        pub fn ghostty_terminal_paste(
            terminal: Handle,
            paste: *const Paste,
            out_written: *mut bool,
        ) -> c_int;
        pub fn ghostty_paste_is_safe(data: *const c_char, len: usize) -> bool;
        pub fn ghostty_focus_encode(
            event: c_int,
            buf: *mut c_char,
            buf_len: usize,
            out_written: *mut usize,
        ) -> c_int;
        pub fn ghostty_color_scheme_report_encode(
            scheme: c_int,
            buf: *mut c_char,
            buf_len: usize,
            out_written: *mut usize,
        ) -> c_int;
        // selection.h
        pub fn ghostty_terminal_select_all(terminal: Handle, out: *mut Selection) -> c_int;
        pub fn ghostty_terminal_selection_format_alloc(
            terminal: Handle,
            allocator: *const c_void,
            options: TerminalSelectionFormatOptions,
            out_ptr: *mut *mut u8,
            out_len: *mut usize,
        ) -> c_int;
        pub fn ghostty_selection_gesture_new(allocator: *const c_void, out: *mut Handle) -> c_int;
        pub fn ghostty_selection_gesture_free(gesture: Handle, terminal: Handle);
        pub fn ghostty_selection_gesture_event_new(
            allocator: *const c_void,
            out: *mut Handle,
            kind: c_int,
        ) -> c_int;
        pub fn ghostty_selection_gesture_event_free(event: Handle);
        pub fn ghostty_selection_gesture_event_set(
            event: Handle,
            option: c_int,
            value: *const c_void,
        ) -> c_int;
        pub fn ghostty_selection_gesture_event(
            gesture: Handle,
            terminal: Handle,
            event: Handle,
            out: *mut Selection,
        ) -> c_int;
        // snapshot.h, formatter.h, allocator.h, build_info.h
        pub fn ghostty_snapshot_encode(terminal: Handle, writer: Writer) -> c_int;
        pub fn ghostty_snapshot_decoder_new(
            allocator: *const c_void,
            decoder: *mut Handle,
            reader: Reader,
        ) -> c_int;
        pub fn ghostty_snapshot_decoder_free(decoder: Handle);
        pub fn ghostty_snapshot_decoder_set(
            decoder: Handle,
            option: c_int,
            value: *const c_void,
        ) -> c_int;
        pub fn ghostty_snapshot_decoder_ready(decoder: Handle, terminal: *mut Handle) -> c_int;
        pub fn ghostty_snapshot_decoder_next(decoder: Handle) -> c_int;
        pub fn ghostty_snapshot_decoder_get(
            decoder: Handle,
            data: c_int,
            out: *mut c_void,
        ) -> c_int;
        pub fn ghostty_formatter_terminal_new(
            allocator: *const c_void,
            formatter: *mut Handle,
            terminal: Handle,
            options: FormatterTerminalOptions,
        ) -> c_int;
        pub fn ghostty_formatter_format_alloc(
            formatter: Handle,
            allocator: *const c_void,
            out_ptr: *mut *mut u8,
            out_len: *mut usize,
        ) -> c_int;
        pub fn ghostty_formatter_free(formatter: Handle);
        pub fn ghostty_free(allocator: *const c_void, ptr: *mut u8, len: usize);
        pub fn ghostty_build_info(data: c_int, out: *mut c_void) -> c_int;
        pub fn ghostty_type_json() -> *const c_char;
        // key/encoder.h, key/event.h
        pub fn ghostty_key_encoder_new(allocator: *const c_void, out: *mut Handle) -> c_int;
        pub fn ghostty_key_encoder_free(encoder: Handle);
        pub fn ghostty_key_encoder_setopt_from_terminal(encoder: Handle, terminal: Handle);
        pub fn ghostty_key_encoder_encode(
            encoder: Handle,
            event: Handle,
            buf: *mut c_char,
            buf_len: usize,
            out_len: *mut usize,
        ) -> c_int;
        pub fn ghostty_key_event_new(allocator: *const c_void, out: *mut Handle) -> c_int;
        pub fn ghostty_key_event_free(event: Handle);
        pub fn ghostty_key_event_set_action(event: Handle, action: c_int);
        pub fn ghostty_key_event_set_key(event: Handle, key: c_int);
        pub fn ghostty_key_event_set_mods(event: Handle, mods: u16);
        pub fn ghostty_key_event_set_consumed_mods(event: Handle, mods: u16);
        pub fn ghostty_key_event_set_composing(event: Handle, composing: bool);
        pub fn ghostty_key_event_set_utf8(event: Handle, utf8: *const c_char, len: usize);
        pub fn ghostty_key_event_set_unshifted_codepoint(event: Handle, codepoint: u32);
        // mouse/encoder.h, mouse/event.h
        pub fn ghostty_mouse_encoder_new(allocator: *const c_void, out: *mut Handle) -> c_int;
        pub fn ghostty_mouse_encoder_free(encoder: Handle);
        pub fn ghostty_mouse_encoder_setopt(encoder: Handle, option: c_int, value: *const c_void);
        pub fn ghostty_mouse_encoder_setopt_from_terminal(encoder: Handle, terminal: Handle);
        pub fn ghostty_mouse_encoder_encode(
            encoder: Handle,
            event: Handle,
            buf: *mut c_char,
            buf_len: usize,
            out_len: *mut usize,
        ) -> c_int;
        pub fn ghostty_mouse_event_new(allocator: *const c_void, out: *mut Handle) -> c_int;
        pub fn ghostty_mouse_event_free(event: Handle);
        pub fn ghostty_mouse_event_set_action(event: Handle, action: c_int);
        pub fn ghostty_mouse_event_set_button(event: Handle, button: c_int);
        pub fn ghostty_mouse_event_clear_button(event: Handle);
        pub fn ghostty_mouse_event_set_mods(event: Handle, mods: u16);
        pub fn ghostty_mouse_event_set_position(event: Handle, position: MousePosition);
        // search.h
        pub fn ghostty_search_new(
            allocator: *const c_void,
            out: *mut Handle,
            terminal: Handle,
        ) -> c_int;
        pub fn ghostty_search_free(search: Handle);
        pub fn ghostty_search_tick(search: Handle, out_status: *mut c_int) -> c_int;
        pub fn ghostty_search_feed(search: Handle) -> c_int;
        pub fn ghostty_search_set(search: Handle, option: c_int, value: *const c_void) -> c_int;
        pub fn ghostty_search_get(search: Handle, data: c_int, out: *mut c_void) -> c_int;
        // kitty_graphics.h
        pub fn ghostty_kitty_graphics_image(graphics: Handle, image_id: u32) -> *const c_void;
        pub fn ghostty_kitty_graphics_image_get(
            image: *const c_void,
            data: c_int,
            out: *mut c_void,
        ) -> c_int;
        // render.h
        pub fn ghostty_render_state_new(allocator: *const c_void, out: *mut Handle) -> c_int;
        pub fn ghostty_render_state_free(state: Handle);
        pub fn ghostty_render_state_update(state: Handle, terminal: Handle) -> c_int;
        pub fn ghostty_render_state_begin_update(state: Handle, terminal: Handle) -> c_int;
        pub fn ghostty_render_state_end_update(state: Handle) -> c_int;
        pub fn ghostty_render_state_clean(state: Handle) -> c_int;
        pub fn ghostty_render_state_get(state: Handle, data: c_int, out: *mut c_void) -> c_int;
        pub fn ghostty_render_state_row_iterator_new(
            allocator: *const c_void,
            out: *mut Handle,
        ) -> c_int;
        pub fn ghostty_render_state_row_iterator_free(iterator: Handle);
        pub fn ghostty_render_state_row_iterator_next(iterator: Handle) -> bool;
        pub fn ghostty_render_state_row_get(
            iterator: Handle,
            data: c_int,
            out: *mut c_void,
        ) -> c_int;
        pub fn ghostty_render_state_row_cells_new(
            allocator: *const c_void,
            out: *mut Handle,
        ) -> c_int;
        pub fn ghostty_render_state_row_cells_free(cells: Handle);
        pub fn ghostty_render_state_row_cells_next(cells: Handle) -> bool;
        pub fn ghostty_render_state_row_cells_get(
            cells: Handle,
            data: c_int,
            out: *mut c_void,
        ) -> c_int;
        pub fn ghostty_render_state_row_cells_get_multi(
            cells: Handle,
            count: usize,
            keys: *const c_int,
            values: *mut *mut c_void,
            out_written: *mut usize,
        ) -> c_int;
        pub fn ghostty_render_state_image_iterator_new(
            allocator: *const c_void,
            out: *mut Handle,
        ) -> c_int;
        pub fn ghostty_render_state_image_iterator_free(iterator: Handle);
        pub fn ghostty_render_state_image_iterator_next(iterator: Handle) -> bool;
        pub fn ghostty_render_state_image_get_multi(
            iterator: Handle,
            count: usize,
            keys: *const c_int,
            values: *mut *mut c_void,
            out_written: *mut usize,
        ) -> c_int;
    }
}

// ===========================================================================
// What the safe API speaks in.
// ===========================================================================

/// A refusal from the library, naming what was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refused {
    /// What was being done, in words a log line can carry.
    pub what: &'static str,
    /// The `GhosttyResult` it answered.
    pub code: i32,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "libghostty-vt refused to {} (result {})",
            self.what, self.code
        )
    }
}

impl std::error::Error for Refused {}

/// `Ok` on success, a [`Refused`] naming `what` otherwise.
const fn check(code: c_int, what: &'static str) -> Result<(), Refused> {
    if code == sys::SUCCESS {
        Ok(())
    } else {
        Err(Refused { what, code })
    }
}

/// An 8-bit RGB colour, laid out as the library's `GhosttyColorRgb`.
pub type Rgb = sys::ColorRgb;

/// Whether the theme is light or dark, for `CSI ? 996 n` and mode 2031.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// Dark text on a light background.
    Light,
    /// Light text on a dark background.
    Dark,
}

impl Scheme {
    const fn raw(self) -> c_int {
        match self {
            Self::Light => sys::COLOR_SCHEME_LIGHT,
            Self::Dark => sys::COLOR_SCHEME_DARK,
        }
    }
}

/// The colours a terminal starts from, before any program changes them.
///
/// The window resolves these from the `terminal.*` roles of its page on every
/// theme change and hands them over; a program's OSC 4/10/11/12 then lays its
/// own over them, and an OSC query answers whatever is current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Colors {
    /// The default foreground.
    pub foreground: Rgb,
    /// The default background.
    pub background: Rgb,
    /// The cursor, or `None` for the library's own choice (the foreground).
    pub cursor: Option<Rgb>,
    /// The 256-colour palette.
    pub palette: Box<[Rgb; 256]>,
    /// Light or dark, for the programs that ask.
    pub scheme: Scheme,
}

// THE TESTS' COLOURS, and they are no table: this crate is what a machine
// runs, and a colour table written into it — even one only a test compiles —
// is a table to reach for instead of the page's roles (invariant 9). The
// differential's reference table lives in the differential's own crate
// (`ronoctua-term-differential`), which never reaches a machine.
#[cfg(test)]
impl Colors {
    /// The colours a test hands a terminal: the engine's OWN, read off a
    /// terminal nothing was set on, with every channel turned over
    /// (`255 - c`).
    ///
    /// Turned over so that no slot holds what the engine would answer had
    /// nothing been set: an answer that matches these can only have come
    /// from `set_colors`, never from a terminal that ignored them.
    pub(crate) fn of_a_test() -> Self {
        let vt = Vt::new(1, 1, &Config::for_version("9.9")).expect("a terminal");
        let state = RenderState::new().expect("a render state");
        state.update(&vt).expect("a frame");
        let own = state.frame().colors();
        let turn = |c: Rgb| Rgb {
            r: !c.r,
            g: !c.g,
            b: !c.b,
        };
        Self {
            foreground: turn(own.foreground),
            background: turn(own.background),
            // Without a cursor colour of its own the engine draws one in the
            // foreground, and then that is the slot turned over.
            cursor: Some(turn(own.cursor.unwrap_or(own.foreground))),
            palette: Box::new(own.palette.map(turn)),
            scheme: Scheme::Dark,
        }
    }
}

/// A terminal mode, packed as the library packs it: the number in bits 0–14,
/// and bit 15 set for an ANSI mode rather than a DEC private one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Mode(u16);

impl Mode {
    /// DECCKM (1): cursor keys send application sequences.
    pub const CURSOR_KEYS: Self = Self(sys::MODE_DECCKM);
    /// 12: the cursor blinks.
    pub const CURSOR_BLINKING: Self = Self(sys::MODE_CURSOR_BLINKING);
    /// DECTCEM (25): the cursor is shown.
    pub const CURSOR_VISIBLE: Self = Self(sys::MODE_CURSOR_VISIBLE);
    /// 1000: mouse presses and releases are reported.
    pub const NORMAL_MOUSE: Self = Self(sys::MODE_NORMAL_MOUSE);
    /// 1002: and motion while a button is held.
    pub const BUTTON_MOUSE: Self = Self(sys::MODE_BUTTON_MOUSE);
    /// 1003: and every motion.
    pub const ANY_MOUSE: Self = Self(sys::MODE_ANY_MOUSE);
    /// 1004: focus in and out are reported.
    pub const FOCUS_EVENT: Self = Self(sys::MODE_FOCUS_EVENT);
    /// 1007: the wheel scrolls an alternate screen with arrow keys.
    pub const ALT_SCROLL: Self = Self(sys::MODE_ALT_SCROLL);
    /// 1049: the alternate screen, with the cursor saved.
    pub const ALT_SCREEN_SAVE: Self = Self(sys::MODE_ALT_SCREEN_SAVE);
    /// 2004: a paste arrives between brackets.
    pub const BRACKETED_PASTE: Self = Self(sys::MODE_BRACKETED_PASTE);
    /// 2026: synchronized output, the program holding the screen.
    pub const SYNC_OUTPUT: Self = Self(sys::MODE_SYNC_OUTPUT);
    /// 2027: grapheme clusters take the width Unicode gives them.
    pub const GRAPHEME_CLUSTER: Self = Self(sys::MODE_GRAPHEME_CLUSTER);
    /// 2031: a change of colour scheme is reported.
    pub const COLOR_SCHEME_REPORT: Self = Self(sys::MODE_COLOR_SCHEME_REPORT);
    /// 2048: a resize is reported in band.
    pub const IN_BAND_RESIZE: Self = Self(sys::MODE_IN_BAND_RESIZE);

    /// A DEC private mode by its number (`CSI ? n h`).
    #[must_use]
    pub const fn dec(number: u16) -> Self {
        Self(number & 0x7fff)
    }

    /// An ANSI mode by its number (`CSI n h`).
    #[must_use]
    pub const fn ansi(number: u16) -> Self {
        Self((number & 0x7fff) | 0x8000)
    }
}

/// Modifier keys, as the library's bitmask (`GhosttyMods`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Mods(u16);

impl Mods {
    /// No modifier.
    pub const NONE: Self = Self(0);
    /// Shift.
    pub const SHIFT: Self = Self(sys::MODS_SHIFT);
    /// Control.
    pub const CTRL: Self = Self(sys::MODS_CTRL);
    /// Alt.
    pub const ALT: Self = Self(sys::MODS_ALT);
    /// Super.
    pub const SUPER: Self = Self(sys::MODS_SUPER);
    /// Caps Lock is on.
    pub const CAPS_LOCK: Self = Self(sys::MODS_CAPS_LOCK);
    /// Num Lock is on.
    pub const NUM_LOCK: Self = Self(sys::MODS_NUM_LOCK);

    /// Whether every bit of `other` is set here.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for Mods {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// The [`Key`] enum and its W3C names, out of ONE list.
///
/// A key reaches the holder as its W3C `code` (`KeyA`, `Enter`) — the wire is
/// the engine's business only here — and two lists, the enum and a name
/// table, would be free to drift one entry apart with nothing to say so.
macro_rules! keys {
    ($(#[$meta:meta])* pub enum $name:ident { $($variant:ident = $value:ident => $w3c:literal,)* }) => {
        $(#[$meta])*
        pub enum $name {
            $($variant = sys::$value,)*
        }

        impl $name {
            /// Every key with its W3C `code`, in the library's order.
            const NAMED: &'static [(Self, &'static str)] = &[$((Self::$variant, $w3c),)*];
        }
    };
}

keys! {
    /// A physical key, by the W3C UI Events `code` it is named after: the key in
    /// that position, whatever the layout prints on it.
    ///
    /// The variants mirror `GhosttyKey` one for one, each set from the checked
    /// constant in `sys`; the names are the W3C ones (the letters are `KeyA`…
    /// there, `A`… here), so a variant needs no documentation beyond its name.
    #[allow(missing_docs)]
    #[repr(i32)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum Key {
        Unidentified = KEY_UNIDENTIFIED => "Unidentified",
        Backquote = KEY_BACKQUOTE => "Backquote",
        Backslash = KEY_BACKSLASH => "Backslash",
        BracketLeft = KEY_BRACKET_LEFT => "BracketLeft",
        BracketRight = KEY_BRACKET_RIGHT => "BracketRight",
        Comma = KEY_COMMA => "Comma",
        Digit0 = KEY_DIGIT_0 => "Digit0",
        Digit1 = KEY_DIGIT_1 => "Digit1",
        Digit2 = KEY_DIGIT_2 => "Digit2",
        Digit3 = KEY_DIGIT_3 => "Digit3",
        Digit4 = KEY_DIGIT_4 => "Digit4",
        Digit5 = KEY_DIGIT_5 => "Digit5",
        Digit6 = KEY_DIGIT_6 => "Digit6",
        Digit7 = KEY_DIGIT_7 => "Digit7",
        Digit8 = KEY_DIGIT_8 => "Digit8",
        Digit9 = KEY_DIGIT_9 => "Digit9",
        Equal = KEY_EQUAL => "Equal",
        IntlBackslash = KEY_INTL_BACKSLASH => "IntlBackslash",
        IntlRo = KEY_INTL_RO => "IntlRo",
        IntlYen = KEY_INTL_YEN => "IntlYen",
        A = KEY_A => "KeyA",
        B = KEY_B => "KeyB",
        C = KEY_C => "KeyC",
        D = KEY_D => "KeyD",
        E = KEY_E => "KeyE",
        F = KEY_F => "KeyF",
        G = KEY_G => "KeyG",
        H = KEY_H => "KeyH",
        I = KEY_I => "KeyI",
        J = KEY_J => "KeyJ",
        K = KEY_K => "KeyK",
        L = KEY_L => "KeyL",
        M = KEY_M => "KeyM",
        N = KEY_N => "KeyN",
        O = KEY_O => "KeyO",
        P = KEY_P => "KeyP",
        Q = KEY_Q => "KeyQ",
        R = KEY_R => "KeyR",
        S = KEY_S => "KeyS",
        T = KEY_T => "KeyT",
        U = KEY_U => "KeyU",
        V = KEY_V => "KeyV",
        W = KEY_W => "KeyW",
        X = KEY_X => "KeyX",
        Y = KEY_Y => "KeyY",
        Z = KEY_Z => "KeyZ",
        Minus = KEY_MINUS => "Minus",
        Period = KEY_PERIOD => "Period",
        Quote = KEY_QUOTE => "Quote",
        Semicolon = KEY_SEMICOLON => "Semicolon",
        Slash = KEY_SLASH => "Slash",
        AltLeft = KEY_ALT_LEFT => "AltLeft",
        AltRight = KEY_ALT_RIGHT => "AltRight",
        Backspace = KEY_BACKSPACE => "Backspace",
        CapsLock = KEY_CAPS_LOCK => "CapsLock",
        ContextMenu = KEY_CONTEXT_MENU => "ContextMenu",
        ControlLeft = KEY_CONTROL_LEFT => "ControlLeft",
        ControlRight = KEY_CONTROL_RIGHT => "ControlRight",
        Enter = KEY_ENTER => "Enter",
        MetaLeft = KEY_META_LEFT => "MetaLeft",
        MetaRight = KEY_META_RIGHT => "MetaRight",
        ShiftLeft = KEY_SHIFT_LEFT => "ShiftLeft",
        ShiftRight = KEY_SHIFT_RIGHT => "ShiftRight",
        Space = KEY_SPACE => "Space",
        Tab = KEY_TAB => "Tab",
        Convert = KEY_CONVERT => "Convert",
        KanaMode = KEY_KANA_MODE => "KanaMode",
        NonConvert = KEY_NON_CONVERT => "NonConvert",
        Delete = KEY_DELETE => "Delete",
        End = KEY_END => "End",
        Help = KEY_HELP => "Help",
        Home = KEY_HOME => "Home",
        Insert = KEY_INSERT => "Insert",
        PageDown = KEY_PAGE_DOWN => "PageDown",
        PageUp = KEY_PAGE_UP => "PageUp",
        ArrowDown = KEY_ARROW_DOWN => "ArrowDown",
        ArrowLeft = KEY_ARROW_LEFT => "ArrowLeft",
        ArrowRight = KEY_ARROW_RIGHT => "ArrowRight",
        ArrowUp = KEY_ARROW_UP => "ArrowUp",
        NumLock = KEY_NUM_LOCK => "NumLock",
        Numpad0 = KEY_NUMPAD_0 => "Numpad0",
        Numpad1 = KEY_NUMPAD_1 => "Numpad1",
        Numpad2 = KEY_NUMPAD_2 => "Numpad2",
        Numpad3 = KEY_NUMPAD_3 => "Numpad3",
        Numpad4 = KEY_NUMPAD_4 => "Numpad4",
        Numpad5 = KEY_NUMPAD_5 => "Numpad5",
        Numpad6 = KEY_NUMPAD_6 => "Numpad6",
        Numpad7 = KEY_NUMPAD_7 => "Numpad7",
        Numpad8 = KEY_NUMPAD_8 => "Numpad8",
        Numpad9 = KEY_NUMPAD_9 => "Numpad9",
        NumpadAdd = KEY_NUMPAD_ADD => "NumpadAdd",
        NumpadBackspace = KEY_NUMPAD_BACKSPACE => "NumpadBackspace",
        NumpadClear = KEY_NUMPAD_CLEAR => "NumpadClear",
        NumpadClearEntry = KEY_NUMPAD_CLEAR_ENTRY => "NumpadClearEntry",
        NumpadComma = KEY_NUMPAD_COMMA => "NumpadComma",
        NumpadDecimal = KEY_NUMPAD_DECIMAL => "NumpadDecimal",
        NumpadDivide = KEY_NUMPAD_DIVIDE => "NumpadDivide",
        NumpadEnter = KEY_NUMPAD_ENTER => "NumpadEnter",
        NumpadEqual = KEY_NUMPAD_EQUAL => "NumpadEqual",
        NumpadMemoryAdd = KEY_NUMPAD_MEMORY_ADD => "NumpadMemoryAdd",
        NumpadMemoryClear = KEY_NUMPAD_MEMORY_CLEAR => "NumpadMemoryClear",
        NumpadMemoryRecall = KEY_NUMPAD_MEMORY_RECALL => "NumpadMemoryRecall",
        NumpadMemoryStore = KEY_NUMPAD_MEMORY_STORE => "NumpadMemoryStore",
        NumpadMemorySubtract = KEY_NUMPAD_MEMORY_SUBTRACT => "NumpadMemorySubtract",
        NumpadMultiply = KEY_NUMPAD_MULTIPLY => "NumpadMultiply",
        NumpadParenLeft = KEY_NUMPAD_PAREN_LEFT => "NumpadParenLeft",
        NumpadParenRight = KEY_NUMPAD_PAREN_RIGHT => "NumpadParenRight",
        NumpadSubtract = KEY_NUMPAD_SUBTRACT => "NumpadSubtract",
        NumpadSeparator = KEY_NUMPAD_SEPARATOR => "NumpadSeparator",
        NumpadUp = KEY_NUMPAD_UP => "NumpadUp",
        NumpadDown = KEY_NUMPAD_DOWN => "NumpadDown",
        NumpadRight = KEY_NUMPAD_RIGHT => "NumpadRight",
        NumpadLeft = KEY_NUMPAD_LEFT => "NumpadLeft",
        NumpadBegin = KEY_NUMPAD_BEGIN => "NumpadBegin",
        NumpadHome = KEY_NUMPAD_HOME => "NumpadHome",
        NumpadEnd = KEY_NUMPAD_END => "NumpadEnd",
        NumpadInsert = KEY_NUMPAD_INSERT => "NumpadInsert",
        NumpadDelete = KEY_NUMPAD_DELETE => "NumpadDelete",
        NumpadPageUp = KEY_NUMPAD_PAGE_UP => "NumpadPageUp",
        NumpadPageDown = KEY_NUMPAD_PAGE_DOWN => "NumpadPageDown",
        Escape = KEY_ESCAPE => "Escape",
        F1 = KEY_F1 => "F1",
        F2 = KEY_F2 => "F2",
        F3 = KEY_F3 => "F3",
        F4 = KEY_F4 => "F4",
        F5 = KEY_F5 => "F5",
        F6 = KEY_F6 => "F6",
        F7 = KEY_F7 => "F7",
        F8 = KEY_F8 => "F8",
        F9 = KEY_F9 => "F9",
        F10 = KEY_F10 => "F10",
        F11 = KEY_F11 => "F11",
        F12 = KEY_F12 => "F12",
        F13 = KEY_F13 => "F13",
        F14 = KEY_F14 => "F14",
        F15 = KEY_F15 => "F15",
        F16 = KEY_F16 => "F16",
        F17 = KEY_F17 => "F17",
        F18 = KEY_F18 => "F18",
        F19 = KEY_F19 => "F19",
        F20 = KEY_F20 => "F20",
        F21 = KEY_F21 => "F21",
        F22 = KEY_F22 => "F22",
        F23 = KEY_F23 => "F23",
        F24 = KEY_F24 => "F24",
        F25 = KEY_F25 => "F25",
        Fn = KEY_FN => "Fn",
        FnLock = KEY_FN_LOCK => "FnLock",
        PrintScreen = KEY_PRINT_SCREEN => "PrintScreen",
        ScrollLock = KEY_SCROLL_LOCK => "ScrollLock",
        Pause = KEY_PAUSE => "Pause",
        BrowserBack = KEY_BROWSER_BACK => "BrowserBack",
        BrowserFavorites = KEY_BROWSER_FAVORITES => "BrowserFavorites",
        BrowserForward = KEY_BROWSER_FORWARD => "BrowserForward",
        BrowserHome = KEY_BROWSER_HOME => "BrowserHome",
        BrowserRefresh = KEY_BROWSER_REFRESH => "BrowserRefresh",
        BrowserSearch = KEY_BROWSER_SEARCH => "BrowserSearch",
        BrowserStop = KEY_BROWSER_STOP => "BrowserStop",
        Eject = KEY_EJECT => "Eject",
        LaunchApp1 = KEY_LAUNCH_APP_1 => "LaunchApp1",
        LaunchApp2 = KEY_LAUNCH_APP_2 => "LaunchApp2",
        LaunchMail = KEY_LAUNCH_MAIL => "LaunchMail",
        MediaPlayPause = KEY_MEDIA_PLAY_PAUSE => "MediaPlayPause",
        MediaSelect = KEY_MEDIA_SELECT => "MediaSelect",
        MediaStop = KEY_MEDIA_STOP => "MediaStop",
        MediaTrackNext = KEY_MEDIA_TRACK_NEXT => "MediaTrackNext",
        MediaTrackPrevious = KEY_MEDIA_TRACK_PREVIOUS => "MediaTrackPrevious",
        Power = KEY_POWER => "Power",
        Sleep = KEY_SLEEP => "Sleep",
        AudioVolumeDown = KEY_AUDIO_VOLUME_DOWN => "AudioVolumeDown",
        AudioVolumeMute = KEY_AUDIO_VOLUME_MUTE => "AudioVolumeMute",
        AudioVolumeUp = KEY_AUDIO_VOLUME_UP => "AudioVolumeUp",
        WakeUp = KEY_WAKE_UP => "WakeUp",
        Copy = KEY_COPY => "Copy",
        Cut = KEY_CUT => "Cut",
        Paste = KEY_PASTE => "Paste",
    }
}

impl Key {
    /// The key's W3C UI Events `code`: `KeyA`, `Digit1`, `Enter`, `ArrowUp`.
    #[must_use]
    pub fn w3c(self) -> &'static str {
        Self::NAMED
            .iter()
            .find(|(k, _)| *k == self)
            .map_or("Unidentified", |(_, n)| n)
    }

    /// The key a W3C `code` names, or `None` for a name the library has no
    /// key for — which the holder sends as [`Key::Unidentified`] with its
    /// text, never as a guess.
    #[must_use]
    pub fn from_w3c(code: &str) -> Option<Self> {
        Self::NAMED
            .iter()
            .find(|(_, n)| *n == code)
            .map(|(k, _)| *k)
    }
}

/// What happened to a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// It went down.
    Press,
    /// It is being held, and the keyboard repeats it.
    Repeat,
    /// It came up — which the kitty protocol can report.
    Release,
}

/// One key event, as the window saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyInput {
    /// The physical key.
    pub key: Key,
    /// Press, repeat or release.
    pub action: KeyAction,
    /// The modifiers held.
    pub mods: Mods,
    /// The modifiers the layout spent producing `text` (Shift for `A`).
    pub consumed: Mods,
    /// What the layout produced, before Ctrl or Alt changed it. Empty for a
    /// key that produces no text.
    pub text: String,
    /// The character the key gives with no modifier, for kitty's
    /// alternate-key reports.
    pub unshifted: Option<char>,
    /// The key is part of a composition (dead keys, an input method).
    pub composing: bool,
}

/// A mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// The primary button.
    Left,
    /// The middle button, or the wheel pressed.
    Middle,
    /// The secondary button.
    Right,
    /// The wheel turned up (button 4).
    WheelUp,
    /// The wheel turned down (button 5).
    WheelDown,
    /// The wheel tilted left (button 6).
    WheelLeft,
    /// The wheel tilted right (button 7).
    WheelRight,
    /// Back (button 8).
    Back,
    /// Forward (button 9).
    Forward,
}

impl Button {
    const fn raw(self) -> c_int {
        match self {
            Self::Left => sys::MOUSE_BUTTON_LEFT,
            Self::Middle => sys::MOUSE_BUTTON_MIDDLE,
            Self::Right => sys::MOUSE_BUTTON_RIGHT,
            Self::WheelUp => sys::MOUSE_BUTTON_FOUR,
            Self::WheelDown => sys::MOUSE_BUTTON_FIVE,
            Self::WheelLeft => sys::MOUSE_BUTTON_SIX,
            Self::WheelRight => sys::MOUSE_BUTTON_SEVEN,
            Self::Back => sys::MOUSE_BUTTON_EIGHT,
            Self::Forward => sys::MOUSE_BUTTON_NINE,
        }
    }
}

/// What the mouse did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    /// A button went down.
    Press,
    /// A button came up.
    Release,
    /// The pointer moved.
    Motion,
}

/// Where the terminal is drawn, in surface pixels, so a pointer position can
/// become a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    /// The surface's width.
    pub width_px: u32,
    /// The surface's height.
    pub height_px: u32,
    /// A cell's width. Never zero.
    pub cell_width_px: u32,
    /// A cell's height. Never zero.
    pub cell_height_px: u32,
    /// The margin before the grid, at the top.
    pub padding_top_px: u32,
    /// At the bottom.
    pub padding_bottom_px: u32,
    /// At the left.
    pub padding_left_px: u32,
    /// At the right.
    pub padding_right_px: u32,
}

/// One mouse event, for a program that asked to be told.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseInput {
    /// Press, release or motion.
    pub action: MouseAction,
    /// The button, or `None` for motion with no button.
    pub button: Option<Button>,
    /// The modifiers held.
    pub mods: Mods,
    /// Where, in surface pixels.
    pub x_px: f32,
    /// Where, in surface pixels.
    pub y_px: f32,
    /// Whether any button is held, which decides what motion reports.
    pub any_button_held: bool,
}

/// Where a paste came from — it decides whether a program in kitty's paste
/// event mode is told about it, or simply receives the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteSource {
    /// The person pasted from a clipboard: a key, a menu, a middle click.
    Clipboard,
    /// Text inserted some other way: an input method's commit, a dropped
    /// file's path.
    Text,
}

/// Why a paste was not written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteRefused {
    /// The text could inject a command (a line break outside bracketed
    /// paste, or the bracket's own terminator). Nothing was written; ask the
    /// person, and paste again with `allow_unsafe`.
    Unsafe,
    /// The library refused for another reason.
    Library(Refused),
}

/// Whether `text` is safe to paste whatever the terminal's modes — the
/// conservative check, for the question asked BEFORE a paste is attempted.
#[must_use]
pub fn is_paste_safe(text: &str) -> bool {
    // SAFETY: a borrowed byte range, read during the call.
    unsafe { sys::ghostty_paste_is_safe(text.as_ptr().cast::<c_char>(), text.len()) }
}

/// Which clipboard a program means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clipboard {
    /// The ordinary clipboard.
    Standard,
    /// X11's "selection" (OSC 52 `s`).
    Selection,
    /// The primary selection (OSC 52 `p`).
    Primary,
}

impl Clipboard {
    const fn of(raw: c_int) -> Self {
        match raw {
            sys::CLIPBOARD_LOCATION_SELECTION => Self::Selection,
            sys::CLIPBOARD_LOCATION_PRIMARY => Self::Primary,
            _ => Self::Standard,
        }
    }
}

/// A program asking to read a clipboard (OSC 52 `?`, kitty's OSC 5522).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardAsk {
    /// Which clipboard.
    pub clipboard: Clipboard,
    /// The MIME types it wants, in order of preference.
    pub mimes: Vec<String>,
    /// The name the program gave, when the protocol carries one.
    pub program: String,
    /// The terminal already holds a grant for this program (kitty's
    /// passwords): a prompt would ask again what was answered.
    pub granted: bool,
}

/// What answers a clipboard read: the text, or `None` to refuse.
///
/// It is called while the VT stream is PAUSED — the library's read is
/// synchronous — so whatever asks the person blocks the terminal until they
/// answer. The holder bounds that with a deadline.
pub type ClipboardReader = Box<dyn FnMut(&ClipboardAsk) -> Option<String> + Send>;

/// The state of a progress report (OSC 9;4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    /// Take the indication away.
    Remove,
    /// Determinate progress.
    Set,
    /// The work failed.
    Error,
    /// Busy, with no percentage.
    Indeterminate,
    /// Paused.
    Pause,
}

/// What a program says it is doing (OSC 7501).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramState {
    /// At rest, waiting for the next instruction.
    Idle,
    /// Running on its own.
    Working,
    /// Finished; the result is ready to be looked at.
    Done,
    /// Cannot continue until the person does something.
    Blocked,
    /// Failed and stopped.
    Error,
    /// Not a state: remove the record and every record beneath it.
    Clear,
}

/// What a blocked program needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedOn {
    /// It did not say, or it is not blocked.
    Unsaid,
    /// An approval ("Apply these changes?").
    Permission,
    /// An answer to type.
    Question,
    /// A login, a password, a token.
    Auth,
}

/// One program status report (OSC 7501), as the library validated it.
///
/// The library keeps none of them: the holder keeps the records, by id, under
/// the specification's rules. `title` and `message` are decoded and free of
/// control characters, but still a program's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStatus {
    /// What it is doing.
    pub state: ProgramState,
    /// What it needs, when blocked.
    pub blocked_on: BlockedOn,
    /// How far along, 0–100, when it said.
    pub percent: Option<u8>,
    /// Which record; empty for the program itself, `/` makes a child.
    pub id: String,
    /// A stable name a machine can match on (`claude-code`).
    pub app: String,
    /// A short label for people.
    pub title: String,
    /// One line for people.
    pub message: String,
}

/// A step of a command, as shell integration (OSC 133) reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptStep {
    /// The shell started drawing a prompt.
    PromptStart,
    /// The prompt is drawn; the person can type.
    InputStart,
    /// The command was submitted and runs.
    OutputStart,
    /// The command finished.
    CommandEnd,
}

/// One shell integration event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// Which step.
    pub step: PromptStep,
    /// The command's exit code, when the shell reported one at its end.
    pub exit_code: Option<i32>,
    /// The command line about to run, when the shell sent it.
    pub command: String,
}

/// Something the program said that is not screen content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// BEL.
    Bell,
    /// OSC 0/2 changed the title.
    Title(String),
    /// OSC 7 (or 9, or 1337) changed the working directory: the raw bytes the
    /// shell sent, a `file://` URI for OSC 7.
    Pwd(String),
    /// OSC 9 or 777 asked for a desktop notification.
    Notification {
        /// The title, empty when the protocol has none.
        title: String,
        /// The body.
        body: String,
    },
    /// OSC 9;4 reported progress.
    Progress {
        /// The state.
        state: ProgressState,
        /// The percentage, when given.
        percent: Option<u8>,
    },
    /// OSC 7501 reported a program's status.
    ProgramStatus(ProgramStatus),
    /// OSC 133 reported a step of a command.
    Prompt(Prompt),
    /// OSC 52 (or 1337, or 5522) wrote a clipboard. Only reported while
    /// writes are accepted ([`Vt::accept_clipboard_writes`]).
    ClipboardWrite {
        /// Which clipboard.
        clipboard: Clipboard,
        /// The representations, `(mime, data)`, all of the same value. Empty
        /// means "clear the clipboard".
        contents: Vec<(String, Vec<u8>)>,
    },
    /// Synchronized output (mode 2026) began (`true`) or ended (`false`).
    RenderHold(bool),
    /// A full reset (RIS): the title and pwd are gone with everything else.
    Reset,
    /// `CSI 8 ; rows ; columns t` asked to resize the window — a request the
    /// embedder may ignore, and does unless the person opted in.
    ResizeRequest {
        /// Rows, or 0 to keep them.
        rows: u16,
        /// Columns, or 0 to keep them.
        columns: u16,
    },
}

/// What a [`Vt`] has to hand over since it was last asked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Effects {
    /// Bytes to write to the PTY, in order: query answers, mode reports and
    /// pastes.
    pub to_pty: Vec<u8>,
    /// Everything else, in order.
    pub events: Vec<Effect>,
}

/// The cursor, as the terminal keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// Column, from 0.
    pub x: u16,
    /// Row of the active area, from 0.
    pub y: u16,
    /// DECTCEM.
    pub visible: bool,
    /// The next character will wrap first.
    pub pending_wrap: bool,
}

/// The scrollable area, in rows: what a scrollbar draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Scrollbar {
    /// Rows in history and on screen together.
    pub total: u64,
    /// The first row of the viewport.
    pub offset: u64,
    /// Rows in the viewport.
    pub len: u64,
}

/// How the formatter writes a terminal out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// The text, without escapes.
    Plain,
    /// VT sequences that rebuild the screen, its colours, modes, cursor and
    /// the rest of its state — the form two terminals are compared in, and
    /// what a viewer of another library version is sent instead of a
    /// snapshot it could not read.
    Vt,
}

/// The largest piece of an unfinished escape sequence kept across writes.
///
/// It is the kitty APC limit (`apc.zig`: 65 MiB), set explicitly below for the
/// kitty protocol too, so the two cannot drift: a kitty image cut by the
/// snapshot boundary is the largest sequence a snapshot has to carry whole.
pub const CONTINUATION_LIMIT_BYTES: usize = 65 * 1024 * 1024;

/// How a [`Vt`] is set up before the first byte reaches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The most scrollback kept, in bytes (page granularity, about 400 KB).
    /// `None` keeps the library's default.
    pub scrollback_bytes: Option<usize>,
    /// The most scrollback kept, in lines. `None` for no line limit.
    pub scrollback_lines: Option<usize>,
    /// The distro's version, for XTVERSION.
    pub version: String,
    /// DA1 advertises sixel (feature 4). True where the holder's inbound
    /// filter turns sixel into kitty images; a terminal without that filter
    /// must not claim it, or programs send images that never appear.
    pub sixel: bool,
    /// The cursor it starts with, and goes back to when a program resets
    /// its own (DECSCUSR 0) — the person's (`terminal.json`). `None` keeps
    /// the library's: a block that does not blink.
    pub cursor: Option<CursorDefault>,
}

/// The cursor a terminal starts with, and that a program's reset (DECSCUSR
/// 0) brings back — Ghostty's `cursor-style` and `cursor-style-blink`. A
/// program that asks for another shape (DECSCUSR 1–6) has it until it resets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorDefault {
    /// Its shape: a block, a bar or an underline — never hollow, which is
    /// how an unfocused window draws whatever the shape is.
    pub shape: CursorShape,
    /// It blinks.
    pub blinks: bool,
}

impl CursorDefault {
    /// The library's style number of the shape; a hollow block is a block.
    const fn style(self) -> c_int {
        match self.shape {
            CursorShape::Bar => sys::TERMINAL_CURSOR_STYLE_BAR,
            CursorShape::Underline => sys::TERMINAL_CURSOR_STYLE_UNDERLINE,
            CursorShape::Block | CursorShape::Hollow => sys::TERMINAL_CURSOR_STYLE_BLOCK,
        }
    }
}

impl Config {
    /// The terminal a program runs in, at `version` of the distro.
    #[must_use]
    pub fn for_version(version: &str) -> Self {
        Self {
            scrollback_bytes: None,
            scrollback_lines: None,
            version: version.to_owned(),
            sixel: true,
            cursor: None,
        }
    }
}

/// The terminfo entry the terminal runs as, answered to XTGETTCAP `TN`: the
/// same name `pty.rs` puts in `TERM`, so a program asking either way hears one
/// answer.
pub const TERMINFO_NAME: &str = crate::pty::TERM_KIND;

/// The Ghostty release the library was built from (`1.4.0`), as the pin names
/// it: the derivation's version, which `build.rs` reads off the store path.
///
/// Not [`library_version`]: libghostty-vt numbers its own ABI apart from
/// Ghostty's releases, and says `0.1.0-dev` at the 1.4.0 tag — a number that
/// would tell a program asking XTVERSION nothing it could look up.
pub const RELEASE: &str = env!("GHOSTTY_VT_RELEASE");

/// The library this program was BUILT against, by its store path's name
/// (`<hash>-libghostty-vt-1.4.0`), which `build.rs` reads.
///
/// A window says it when it attaches; a holder of the same build answers with
/// a snapshot, one of another build — left running across a switch — with
/// the portable form, because the snapshot format carries no promise across
/// builds.
pub const ENGINE: &str = env!("GHOSTTY_VT_ENGINE");

/// The linked library's own version string — its ABI's, not Ghostty's
/// release ([`RELEASE`]).
#[must_use]
pub fn library_version() -> String {
    let mut s = sys::String {
        ptr: null(),
        len: 0,
    };
    // SAFETY: the out pointer is a `GhosttyString`, the documented type; the
    // bytes it points to live as long as the library.
    let r = unsafe { sys::ghostty_build_info(sys::BUILD_INFO_VERSION_STRING, (&raw mut s).cast()) };
    if r != sys::SUCCESS {
        return String::new();
    }
    // SAFETY: as above.
    String::from_utf8_lossy(unsafe { bytes_of(s) }).into_owned()
}

/// The bytes of a borrowed `GhosttyString`. Empty for a null pointer.
///
/// # Safety
///
/// `s` must come from the library and be read only while it is valid —
/// during the callback or before the next mutating call, as its source says.
unsafe fn bytes_of<'a>(s: sys::String) -> &'a [u8] {
    if s.ptr.is_null() || s.len == 0 {
        return &[];
    }
    // SAFETY: the caller's promise.
    unsafe { std::slice::from_raw_parts(s.ptr, s.len) }
}

/// A borrowed string as an owned one, lossily — it is a program's text.
///
/// # Safety
///
/// As [`bytes_of`].
unsafe fn text_of(s: sys::String) -> String {
    // SAFETY: the caller's promise.
    String::from_utf8_lossy(unsafe { bytes_of(s) }).into_owned()
}

/// A Rust string as a borrowed `GhosttyString`, valid while `s` is.
const fn borrowed(s: &[u8]) -> sys::String {
    sys::String {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

/// Runs `body`, and swallows a panic instead of letting it unwind into the
/// library — which is undefined behaviour. What a panicking callback was
/// recording is lost; the terminal goes on.
fn guarded<R>(fallback: R, body: impl FnOnce() -> R) -> R {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or_else(|_| {
        tracing::error!("a libghostty-vt callback panicked; its effect was dropped");
        fallback
    })
}

/// Reads one `ghostty_terminal_get` field of type `T`.
///
/// # Safety
///
/// `T` must be the output type the header documents for `data`.
unsafe fn get<T: Copy>(terminal: sys::Handle, data: c_int, initial: T) -> Option<T> {
    let mut out = initial;
    // SAFETY: the caller's promise about `T`.
    let r = unsafe { sys::ghostty_terminal_get(terminal, data, (&raw mut out).cast()) };
    (r == sys::SUCCESS).then_some(out)
}

/// The owned result of an `_alloc` call, freed with `ghostty_free`.
///
/// # Safety
///
/// `ptr`/`len` must come from a libghostty-vt `_alloc` with the default
/// allocator.
unsafe fn take_allocated(ptr: *mut u8, len: usize) -> Vec<u8> {
    if ptr.is_null() {
        return Vec::new();
    }
    // SAFETY: the caller's promise; copied before it is freed.
    let out = unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec();
    // SAFETY: the same pointer, length and (default) allocator.
    unsafe { sys::ghostty_free(null(), ptr, len) };
    out
}

/// A writer callback that appends to the `Vec<u8>` behind `userdata`.
unsafe extern "C" fn write_into_vec(userdata: *mut c_void, data: *const u8, len: usize) -> bool {
    guarded(false, || {
        // SAFETY: `userdata` is the `&mut Vec<u8>` the caller passed for this
        // call only, and `data` is valid for `len` bytes during it.
        let out = unsafe { &mut *userdata.cast::<Vec<u8>>() };
        out.extend_from_slice(unsafe { std::slice::from_raw_parts(data, len) });
        true
    })
}

/// Formats a terminal with the formatter, every extra on for `Format::Vt`.
///
/// # Safety
///
/// `terminal` must be alive and not used concurrently during the call.
unsafe fn format_terminal(terminal: sys::Handle, format: Format) -> Result<Vec<u8>, Refused> {
    let every = format == Format::Vt;
    let options = sys::FormatterTerminalOptions {
        size: size_of::<sys::FormatterTerminalOptions>(),
        emit: match format {
            Format::Plain => sys::FORMATTER_FORMAT_PLAIN,
            Format::Vt => sys::FORMATTER_FORMAT_VT,
        },
        unwrap: false,
        trim: !every,
        extra: sys::FormatterTerminalExtra {
            size: size_of::<sys::FormatterTerminalExtra>(),
            palette: every,
            modes: every,
            scrolling_region: every,
            tabstops: every,
            pwd: every,
            keyboard: every,
            screen: sys::FormatterScreenExtra {
                size: size_of::<sys::FormatterScreenExtra>(),
                cursor: every,
                style: every,
                hyperlink: every,
                protection: every,
                kitty_keyboard: every,
                charsets: every,
            },
        },
        selection: null(),
    };
    let mut formatter: sys::Handle = null_mut();
    // SAFETY: the options are fully initialised and sized; the terminal is
    // the caller's promise.
    check(
        unsafe {
            sys::ghostty_formatter_terminal_new(null(), &raw mut formatter, terminal, options)
        },
        "create a formatter",
    )?;
    let (mut ptr, mut len) = (null_mut::<u8>(), 0usize);
    // SAFETY: a live formatter; out pointers are ours.
    let r = unsafe {
        sys::ghostty_formatter_format_alloc(formatter, null(), &raw mut ptr, &raw mut len)
    };
    // SAFETY: freed once, after its last use.
    unsafe { sys::ghostty_formatter_free(formatter) };
    check(r, "format the terminal")?;
    // SAFETY: an `_alloc` result with the default allocator.
    Ok(unsafe { take_allocated(ptr, len) })
}

// ===========================================================================
// The authoritative terminal.
// ===========================================================================

/// Where a [`Vt`]'s callbacks write: the one `userdata` the library passes to
/// every one of them.
///
/// Heap-allocated once and never moved, so the pointer the library holds
/// stays valid for the terminal's life; the callbacks only RECORD into it,
/// because they run in the middle of a `vt_write` and may not call back in.
struct Sink {
    effects: Effects,
    /// The XTVERSION answer, kept here so the bytes outlive the callback
    /// that hands the library a pointer to them.
    xtversion: Vec<u8>,
    /// The size the SIZE callback reports (CSI 14/16/18/19 t, mode 2048).
    rows: u16,
    columns: u16,
    cell_width_px: u32,
    cell_height_px: u32,
    /// The scheme `CSI ? 996 n` answers, once the window said.
    scheme: Option<Scheme>,
    /// DA1 advertises sixel.
    sixel: bool,
    /// OSC 52 writes are accepted (a window is attached to receive them).
    clipboard_writes: bool,
    /// What answers an OSC 52 read, when anything does.
    clipboard_reader: Option<ClipboardReader>,
    /// Synchronized output holds the screen right now.
    held: bool,
}

impl Sink {
    /// The sink behind a callback's `userdata`.
    ///
    /// # Safety
    ///
    /// `userdata` must be the pointer [`Vt::new`] installed, and no other
    /// reference to the sink may be alive — true during a callback, which runs
    /// inside a library call made with no borrow of the sink outstanding.
    unsafe fn of<'a>(userdata: *mut c_void) -> &'a mut Self {
        // SAFETY: the caller's promise.
        unsafe { &mut *userdata.cast::<Self>() }
    }
}

unsafe extern "C" fn on_write_pty(_: sys::Handle, ud: *mut c_void, data: *const u8, len: usize) {
    guarded((), || {
        // SAFETY: see `Sink::of`; `data` is valid for `len` bytes during the call.
        let sink = unsafe { Sink::of(ud) };
        if !data.is_null() {
            sink.effects
                .to_pty
                .extend_from_slice(unsafe { std::slice::from_raw_parts(data, len) });
        }
    });
}

unsafe extern "C" fn on_bell(_: sys::Handle, ud: *mut c_void) {
    // SAFETY: see `Sink::of`.
    guarded((), || {
        unsafe { Sink::of(ud) }.effects.events.push(Effect::Bell)
    });
}

unsafe extern "C" fn on_xtversion(_: sys::Handle, ud: *mut c_void) -> sys::String {
    let empty = sys::String {
        ptr: null(),
        len: 0,
    };
    // SAFETY: see `Sink::of`. The bytes live in the sink, past the call.
    guarded(empty, || borrowed(&unsafe { Sink::of(ud) }.xtversion))
}

unsafe extern "C" fn on_title(terminal: sys::Handle, ud: *mut c_void) {
    guarded((), || {
        // The title is already set when this runs (`stream_terminal.zig`
        // sets it, then calls); reading it here keeps it in order with the
        // other effects.
        let empty = sys::String {
            ptr: null(),
            len: 0,
        };
        // SAFETY: TITLE's documented type; the string is read before the
        // callback returns.
        let title = unsafe { get(terminal, sys::TERMINAL_DATA_TITLE, empty) }
            .map(|s| unsafe { text_of(s) })
            .unwrap_or_default();
        // SAFETY: see `Sink::of`.
        unsafe { Sink::of(ud) }
            .effects
            .events
            .push(Effect::Title(title));
    });
}

unsafe extern "C" fn on_pwd(terminal: sys::Handle, ud: *mut c_void) {
    guarded((), || {
        let empty = sys::String {
            ptr: null(),
            len: 0,
        };
        // SAFETY: PWD's documented type, read inside the callback as the
        // header says to.
        let pwd = unsafe { get(terminal, sys::TERMINAL_DATA_PWD, empty) }
            .map(|s| unsafe { text_of(s) })
            .unwrap_or_default();
        // SAFETY: see `Sink::of`.
        unsafe { Sink::of(ud) }
            .effects
            .events
            .push(Effect::Pwd(pwd));
    });
}

unsafe extern "C" fn on_size(
    _: sys::Handle,
    ud: *mut c_void,
    out: *mut sys::SizeReportSize,
) -> bool {
    guarded(false, || {
        // SAFETY: see `Sink::of`; `out` is the library's, written whole.
        let sink = unsafe { Sink::of(ud) };
        unsafe {
            out.write(sys::SizeReportSize {
                rows: sink.rows,
                columns: sink.columns,
                cell_width: sink.cell_width_px,
                cell_height: sink.cell_height_px,
            });
        }
        true
    })
}

unsafe extern "C" fn on_color_scheme(_: sys::Handle, ud: *mut c_void, out: *mut c_int) -> bool {
    guarded(false, || {
        // SAFETY: see `Sink::of`.
        let Some(scheme) = unsafe { Sink::of(ud) }.scheme else {
            // Nobody said yet: silence beats a guess a program would act on.
            return false;
        };
        // SAFETY: `out` is the library's `GhosttyColorScheme`.
        unsafe { out.write(scheme.raw()) };
        true
    })
}

unsafe extern "C" fn on_device_attributes(
    _: sys::Handle,
    ud: *mut c_void,
    out: *mut sys::DeviceAttributes,
) -> bool {
    guarded(false, || {
        // SAFETY: see `Sink::of`.
        let sixel = unsafe { Sink::of(ud) }.sixel;
        // VT220 conformance, with colour and clipboard access, like Ghostty
        // (`termio/stream_handler.zig`: `62;22;52`); sixel only where the
        // holder's filter makes it true. DA2 answers Ghostty's own `1;10;0`,
        // which programs already know what to make of.
        let mut features = [0u16; 64];
        let mut n = 0;
        let mut add = |f: u16| {
            features[n] = f;
            n += 1;
        };
        if sixel {
            add(sys::DA_FEATURE_SIXEL);
        }
        add(sys::DA_FEATURE_ANSI_COLOR);
        add(sys::DA_FEATURE_CLIPBOARD);
        // SAFETY: `out` is the library's (left uninitialised), written whole.
        unsafe {
            out.write(sys::DeviceAttributes {
                primary: sys::DeviceAttributesPrimary {
                    conformance_level: sys::DA_CONFORMANCE_VT220,
                    features,
                    num_features: n,
                },
                secondary: sys::DeviceAttributesSecondary {
                    device_type: sys::DA_DEVICE_TYPE_VT220,
                    firmware_version: 10,
                    rom_cartridge: 0,
                },
                tertiary: sys::DeviceAttributesTertiary { unit_id: 0 },
            });
        }
        true
    })
}

unsafe extern "C" fn on_clipboard_write(
    _: sys::Handle,
    ud: *mut c_void,
    write: *const sys::ClipboardWrite,
) {
    guarded((), || {
        // SAFETY: see `Sink::of`; `write` is valid for the call, and its
        // size says which fields exist.
        let sink = unsafe { Sink::of(ud) };
        let write_ref = unsafe { &*write };
        let accepted = sink.clipboard_writes && write_ref.size >= size_of::<sys::ClipboardWrite>();
        if accepted {
            let contents = if write_ref.contents.is_null() {
                &[][..]
            } else {
                // SAFETY: `contents_len` entries, borrowed for the call.
                unsafe { std::slice::from_raw_parts(write_ref.contents, write_ref.contents_len) }
            };
            let contents = contents
                .iter()
                // SAFETY: borrowed for the call, copied now.
                .map(|c| unsafe { (text_of(c.mime), bytes_of(c.data).to_vec()) })
                .collect();
            sink.effects.events.push(Effect::ClipboardWrite {
                clipboard: Clipboard::of(write_ref.location),
                contents,
            });
        }
        let reply = sys::ClipboardWriteReply {
            size: size_of::<sys::ClipboardWriteReply>(),
            result: if accepted {
                sys::CLIPBOARD_WRITE_RESULT_SUCCESS
            } else {
                sys::CLIPBOARD_WRITE_RESULT_DENIED
            },
            remember: false,
        };
        if let Some(answer) = write_ref.reply {
            // SAFETY: answered inside the callback, as the contract says.
            unsafe { answer(write, &raw const reply) };
        }
    });
}

unsafe extern "C" fn on_clipboard_read(
    _: sys::Handle,
    ud: *mut c_void,
    read: *const sys::ClipboardRead,
) {
    guarded((), || {
        // SAFETY: see `Sink::of`; `read` is valid for the call.
        let sink = unsafe { Sink::of(ud) };
        let read_ref = unsafe { &*read };
        if read_ref.size < size_of::<sys::ClipboardRead>() {
            return;
        }
        let mimes = if read_ref.mimes.is_null() {
            &[][..]
        } else {
            // SAFETY: `mimes_len` entries, borrowed for the call.
            unsafe { std::slice::from_raw_parts(read_ref.mimes, read_ref.mimes_len) }
        };
        let ask = ClipboardAsk {
            clipboard: Clipboard::of(read_ref.location),
            // SAFETY: borrowed for the call, copied now.
            mimes: mimes.iter().map(|m| unsafe { text_of(*m) }).collect(),
            program: unsafe { text_of(read_ref.name) },
            granted: read_ref.granted,
        };
        let text = sink
            .clipboard_reader
            .as_mut()
            .and_then(|reader| reader(&ask));
        let mime = b"text/plain";
        let content = text.as_ref().map(|t| sys::ClipboardContent {
            mime: borrowed(mime),
            data: borrowed(t.as_bytes()),
        });
        let reply = sys::ClipboardReadReply {
            size: size_of::<sys::ClipboardReadReply>(),
            result: if content.is_some() {
                sys::CLIPBOARD_READ_RESULT_SUCCESS
            } else {
                sys::CLIPBOARD_READ_RESULT_DENIED
            },
            contents: content.as_ref().map_or(null(), std::ptr::from_ref),
            contents_len: usize::from(content.is_some()),
            available: null(),
            available_len: 0,
            remember: false,
        };
        if let Some(answer) = read_ref.reply {
            // SAFETY: answered inside the callback; `text` and `content`
            // outlive the call.
            unsafe { answer(read, &raw const reply) };
        }
    });
}

unsafe extern "C" fn on_notification(
    _: sys::Handle,
    ud: *mut c_void,
    note: *const sys::TerminalDesktopNotification,
) {
    guarded((), || {
        // SAFETY: valid for the call; its size says which fields exist.
        let note = unsafe { &*note };
        if note.size < size_of::<sys::TerminalDesktopNotification>() {
            return;
        }
        // SAFETY: see `Sink::of`; the strings are copied now.
        unsafe { Sink::of(ud) }
            .effects
            .events
            .push(Effect::Notification {
                title: unsafe { text_of(note.title) },
                body: unsafe { text_of(note.body) },
            });
    });
}

/// A percentage the library sends as `-1` when there is none.
fn percent(raw: i8) -> Option<u8> {
    u8::try_from(raw).ok().filter(|p| *p <= 100)
}

unsafe extern "C" fn on_progress(
    _: sys::Handle,
    ud: *mut c_void,
    report: *const sys::TerminalProgressReport,
) {
    guarded((), || {
        // SAFETY: valid for the call.
        let report = unsafe { &*report };
        if report.size < size_of::<sys::TerminalProgressReport>() {
            return;
        }
        let state = match report.state {
            sys::TERMINAL_PROGRESS_STATE_REMOVE => ProgressState::Remove,
            sys::TERMINAL_PROGRESS_STATE_SET => ProgressState::Set,
            sys::TERMINAL_PROGRESS_STATE_ERROR => ProgressState::Error,
            sys::TERMINAL_PROGRESS_STATE_INDETERMINATE => ProgressState::Indeterminate,
            sys::TERMINAL_PROGRESS_STATE_PAUSE => ProgressState::Pause,
            // A state a later library added: not one this code can show.
            _ => return,
        };
        // SAFETY: see `Sink::of`.
        unsafe { Sink::of(ud) }
            .effects
            .events
            .push(Effect::Progress {
                state,
                percent: percent(report.progress),
            });
    });
}

unsafe extern "C" fn on_program_status(
    _: sys::Handle,
    ud: *mut c_void,
    report: *const sys::TerminalProgramStatus,
) {
    guarded((), || {
        // SAFETY: valid for the call.
        let report = unsafe { &*report };
        if report.size < size_of::<sys::TerminalProgramStatus>() {
            return;
        }
        let state = match report.state {
            sys::PROGRAM_STATUS_STATE_IDLE => ProgramState::Idle,
            sys::PROGRAM_STATUS_STATE_WORKING => ProgramState::Working,
            sys::PROGRAM_STATUS_STATE_DONE => ProgramState::Done,
            sys::PROGRAM_STATUS_STATE_BLOCKED => ProgramState::Blocked,
            sys::PROGRAM_STATUS_STATE_ERROR => ProgramState::Error,
            sys::PROGRAM_STATUS_STATE_CLEAR => ProgramState::Clear,
            _ => return,
        };
        let blocked_on = match report.kind {
            sys::PROGRAM_STATUS_KIND_PERMISSION => BlockedOn::Permission,
            sys::PROGRAM_STATUS_KIND_QUESTION => BlockedOn::Question,
            sys::PROGRAM_STATUS_KIND_AUTH => BlockedOn::Auth,
            _ => BlockedOn::Unsaid,
        };
        // SAFETY: the strings are borrowed for the call and copied now.
        let status = unsafe {
            ProgramStatus {
                state,
                blocked_on,
                percent: percent(report.progress),
                id: text_of(report.id),
                app: text_of(report.app),
                title: text_of(report.title),
                message: text_of(report.message),
            }
        };
        // SAFETY: see `Sink::of`.
        unsafe { Sink::of(ud) }
            .effects
            .events
            .push(Effect::ProgramStatus(status));
    });
}

unsafe extern "C" fn on_semantic_prompt(
    _: sys::Handle,
    ud: *mut c_void,
    event: *const sys::TerminalSemanticPrompt,
) {
    guarded((), || {
        // SAFETY: valid for the call.
        let event = unsafe { &*event };
        if event.size < size_of::<sys::TerminalSemanticPrompt>() {
            return;
        }
        let step = match event.kind {
            sys::SEMANTIC_PROMPT_PROMPT_START => PromptStep::PromptStart,
            sys::SEMANTIC_PROMPT_INPUT_START => PromptStep::InputStart,
            sys::SEMANTIC_PROMPT_OUTPUT_START => PromptStep::OutputStart,
            sys::SEMANTIC_PROMPT_COMMAND_END => PromptStep::CommandEnd,
            // "More kinds may be added in later versions, so ignore any kind
            // you don't handle."
            _ => return,
        };
        let prompt = Prompt {
            step,
            exit_code: event.has_exit_code.then_some(event.exit_code),
            // SAFETY: borrowed for the call, copied now.
            command: unsafe { text_of(event.command) },
        };
        // SAFETY: see `Sink::of`.
        unsafe { Sink::of(ud) }
            .effects
            .events
            .push(Effect::Prompt(prompt));
    });
}

unsafe extern "C" fn on_render_hold(_: sys::Handle, ud: *mut c_void, held: bool) {
    guarded((), || {
        // SAFETY: see `Sink::of`.
        let sink = unsafe { Sink::of(ud) };
        sink.held = held;
        sink.effects.events.push(Effect::RenderHold(held));
    });
}

unsafe extern "C" fn on_reset(_: sys::Handle, ud: *mut c_void) {
    // SAFETY: see `Sink::of`.
    guarded((), || {
        unsafe { Sink::of(ud) }.effects.events.push(Effect::Reset)
    });
}

unsafe extern "C" fn on_resize_window(
    _: sys::Handle,
    ud: *mut c_void,
    request: *const sys::TerminalResizeWindow,
) {
    guarded((), || {
        // SAFETY: valid for the call; the tag says which arm is filled.
        let request = unsafe { &*request };
        if request.tag != sys::TERMINAL_RESIZE_WINDOW_GRID {
            return;
        }
        // SAFETY: the GRID arm, per the tag.
        let grid = unsafe { request.value.grid };
        // SAFETY: see `Sink::of`.
        unsafe { Sink::of(ud) }
            .effects
            .events
            .push(Effect::ResizeRequest {
                rows: grid.rows,
                columns: grid.columns,
            });
    });
}

/// The key encoder and its reusable event: made once, re-armed per key.
struct KeyEncoder {
    encoder: sys::Handle,
    event: sys::Handle,
}

/// The mouse encoder and its reusable event.
struct MouseEncoder {
    encoder: sys::Handle,
    event: sys::Handle,
}

/// The authoritative terminal: the one beside the PTY, the only one that
/// writes to it.
pub struct Vt {
    terminal: NonNull<c_void>,
    /// Owned through a raw pointer, not a `Box`: the library holds the same
    /// pointer, and a `Box` would claim the uniqueness the callbacks share.
    sink: NonNull<Sink>,
    keys: KeyEncoder,
    mouse: MouseEncoder,
    columns: u16,
    rows: u16,
}

// SAFETY: the headers' rule is "no concurrent use, no re-entry" — nothing ties
// a terminal (or its encoders, or the sink its callbacks write) to the thread
// that made it. `Vt` is not `Sync`, and every method that reaches the library
// takes `&mut self` or happens between writes on `&self`, so one owner at a
// time is what the rule asks.
unsafe impl Send for Vt {}

impl Vt {
    /// A terminal of the given size, set up by `config` before any byte can
    /// reach it.
    ///
    /// # Errors
    ///
    /// The library refused to allocate it or to take one of its settings.
    pub fn new(columns: u16, rows: u16, config: &Config) -> Result<Self, Refused> {
        // At least one cell, and the SAME size everywhere it is kept: the
        // terminal, the size reports and `size()`/`grid()` must not disagree
        // about a terminal asked for at zero.
        let (columns, rows) = (columns.max(1), rows.max(1));
        let mut raw: sys::Handle = null_mut();
        // SAFETY: a NULL allocator is the library's default, documented.
        check(
            unsafe { sys::ghostty_terminal_new(null(), &raw mut raw, columns, rows) },
            "create a terminal",
        )?;
        let terminal = NonNull::new(raw).ok_or(Refused {
            what: "create a terminal",
            code: sys::OUT_OF_MEMORY,
        })?;
        let sink = Box::new(Sink {
            effects: Effects::default(),
            xtversion: format!("ronoctua {} (libghostty-vt {RELEASE})", config.version)
                .into_bytes(),
            rows,
            columns,
            cell_width_px: 0,
            cell_height_px: 0,
            scheme: None,
            sixel: config.sixel,
            clipboard_writes: false,
            clipboard_reader: None,
            held: false,
        });
        let sink = NonNull::from(Box::leak(sink));
        // From here on `Drop` owns the cleanup, even if a setting is refused.
        let mut vt = Self {
            terminal,
            sink,
            keys: KeyEncoder {
                encoder: null_mut(),
                event: null_mut(),
            },
            mouse: MouseEncoder {
                encoder: null_mut(),
                event: null_mut(),
            },
            columns,
            rows,
        };
        vt.configure(config)?;
        vt.make_encoders()?;
        Ok(vt)
    }

    fn raw(&self) -> sys::Handle {
        self.terminal.as_ptr()
    }

    /// The sink, between library calls.
    fn sink(&mut self) -> &mut Sink {
        // SAFETY: no library call is in progress (we hold `&mut self`), so no
        // callback holds a reference to it.
        unsafe { self.sink.as_mut() }
    }

    fn set(
        &mut self,
        option: c_int,
        value: *const c_void,
        what: &'static str,
    ) -> Result<(), Refused> {
        // SAFETY: each caller passes the input type its option documents.
        check(
            unsafe { sys::ghostty_terminal_set(self.raw(), option, value) },
            what,
        )
    }

    /// Every setting, in the order that matters: continuation tracking BEFORE
    /// the first write, because a sequence begun while tracking was off can
    /// never be reconstructed, and the snapshot handed to a window would lose
    /// it.
    fn configure(&mut self, config: &Config) -> Result<(), Refused> {
        let userdata = self.sink.as_ptr().cast::<c_void>().cast_const();
        self.set(sys::TERMINAL_OPT_USERDATA, userdata, "take its userdata")?;
        let limit = CONTINUATION_LIMIT_BYTES;
        self.set(
            sys::TERMINAL_OPT_CONTINUATION_MAX_BYTES,
            (&raw const limit).cast(),
            "track unfinished sequences",
        )?;
        self.set(
            sys::TERMINAL_OPT_APC_MAX_BYTES_KITTY,
            (&raw const limit).cast(),
            "bound kitty's APC",
        )?;
        // Grapheme clustering ON by default — Ghostty's own default — and as
        // the RESET default too, so `ESC c` does not take it away.
        let graphemes = sys::TerminalModeConfig {
            mode: sys::MODE_GRAPHEME_CLUSTER,
            value: true,
        };
        self.set(
            sys::TERMINAL_OPT_MODE_DEFAULT,
            (&raw const graphemes).cast(),
            "default to grapheme clustering",
        )?;
        let name = borrowed(TERMINFO_NAME.as_bytes());
        self.set(
            sys::TERMINAL_OPT_TERMINFO_NAME,
            (&raw const name).cast(),
            "take its terminfo name",
        )?;
        if let Some(bytes) = config.scrollback_bytes {
            self.set(
                sys::TERMINAL_OPT_SCROLLBACK_MAX_BYTES,
                (&raw const bytes).cast(),
                "limit its scrollback in bytes",
            )?;
        }
        if let Some(lines) = config.scrollback_lines {
            self.set(
                sys::TERMINAL_OPT_SCROLLBACK_MAX_LINES,
                (&raw const lines).cast(),
                "limit its scrollback in lines",
            )?;
        }
        if let Some(cursor) = config.cursor {
            // THE DEFAULT IS THE CURSOR OF NOW TOO, for a terminal no program
            // has written to yet: a snapshot of it carries the person's
            // cursor, and a program's reset (DECSCUSR 0) comes back to it.
            // SAFETY: a live terminal, serialised by `&mut self`.
            unsafe { default_cursor_of(self.raw(), cursor) }?;
        }
        let callbacks: [(c_int, sys::Callback, &'static str); 15] = [
            (
                sys::TERMINAL_OPT_WRITE_PTY,
                on_write_pty as sys::WritePtyFn as sys::Callback,
                "take the PTY writer",
            ),
            (
                sys::TERMINAL_OPT_BELL,
                on_bell as sys::BellFn as sys::Callback,
                "take the bell",
            ),
            (
                sys::TERMINAL_OPT_XTVERSION,
                on_xtversion as sys::StringFn as sys::Callback,
                "take XTVERSION",
            ),
            (
                sys::TERMINAL_OPT_TITLE_CHANGED,
                on_title as sys::BellFn as sys::Callback,
                "take the title",
            ),
            (
                sys::TERMINAL_OPT_SIZE,
                on_size as sys::SizeFn as sys::Callback,
                "take the size",
            ),
            (
                sys::TERMINAL_OPT_COLOR_SCHEME,
                on_color_scheme as sys::ColorSchemeFn as sys::Callback,
                "take the colour scheme",
            ),
            (
                sys::TERMINAL_OPT_DEVICE_ATTRIBUTES,
                on_device_attributes as sys::DeviceAttributesFn as sys::Callback,
                "take the device attributes",
            ),
            (
                sys::TERMINAL_OPT_PWD_CHANGED,
                on_pwd as sys::BellFn as sys::Callback,
                "take the pwd",
            ),
            (
                sys::TERMINAL_OPT_CLIPBOARD_WRITE,
                on_clipboard_write as sys::ClipboardWriteFn as sys::Callback,
                "take clipboard writes",
            ),
            (
                sys::TERMINAL_OPT_DESKTOP_NOTIFICATION,
                on_notification as sys::NotificationFn as sys::Callback,
                "take notifications",
            ),
            (
                sys::TERMINAL_OPT_PROGRESS_REPORT,
                on_progress as sys::ProgressFn as sys::Callback,
                "take progress reports",
            ),
            (
                sys::TERMINAL_OPT_RENDER_HOLD,
                on_render_hold as sys::RenderHoldFn as sys::Callback,
                "take render holds",
            ),
            (
                sys::TERMINAL_OPT_SEMANTIC_PROMPT,
                on_semantic_prompt as sys::SemanticPromptFn as sys::Callback,
                "take shell integration",
            ),
            (
                sys::TERMINAL_OPT_RESET,
                on_reset as sys::BellFn as sys::Callback,
                "take resets",
            ),
            // With this callback set the library ANSWERS `OSC 7501;?` itself
            // (through WRITE_PTY), so a program that asks before reporting
            // learns the protocol is spoken here.
            (
                sys::TERMINAL_OPT_PROGRAM_STATUS,
                on_program_status as sys::ProgramStatusFn as sys::Callback,
                "take program status",
            ),
        ];
        for (option, callback, what) in callbacks {
            self.set(option, callback, what)?;
        }
        self.set(
            sys::TERMINAL_OPT_RESIZE_WINDOW,
            on_resize_window as sys::ResizeWindowFn as sys::Callback,
            "take resize requests",
        )
    }

    fn make_encoders(&mut self) -> Result<(), Refused> {
        // SAFETY: NULL allocator; out pointers are ours. Each handle is freed
        // in `Drop`, NULL ones included (the free functions accept NULL).
        unsafe {
            check(
                sys::ghostty_key_encoder_new(null(), &raw mut self.keys.encoder),
                "create a key encoder",
            )?;
            check(
                sys::ghostty_key_event_new(null(), &raw mut self.keys.event),
                "create a key event",
            )?;
            check(
                sys::ghostty_mouse_encoder_new(null(), &raw mut self.mouse.encoder),
                "create a mouse encoder",
            )?;
            check(
                sys::ghostty_mouse_event_new(null(), &raw mut self.mouse.event),
                "create a mouse event",
            )?;
        }
        Ok(())
    }

    /// Feeds the program's output.
    pub fn feed(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        // SAFETY: a live terminal and a borrowed slice; callbacks run during
        // the call and only write the sink, which nothing else borrows now.
        unsafe { sys::ghostty_terminal_vt_write(self.raw(), bytes.as_ptr(), bytes.len()) };
    }

    /// Resizes, in cells and in the pixels of one cell.
    ///
    /// The pixels are what kitty images are scaled by and what `CSI 14 t`
    /// and mode 2048 report; it also ends a synchronized-output hold, as the
    /// specification allows, so a resize shows at once.
    ///
    /// # Errors
    ///
    /// The library refused the size.
    pub fn resize(
        &mut self,
        columns: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<(), Refused> {
        let (columns, rows) = (columns.max(1), rows.max(1));
        {
            // The SIZE callback may run during the resize (mode 2048), and it
            // must report the NEW size.
            let sink = self.sink();
            sink.columns = columns;
            sink.rows = rows;
            sink.cell_width_px = cell_width_px;
            sink.cell_height_px = cell_height_px;
        }
        // SAFETY: a live terminal.
        check(
            unsafe {
                sys::ghostty_terminal_resize(
                    self.raw(),
                    columns,
                    rows,
                    cell_width_px,
                    cell_height_px,
                )
            },
            "resize",
        )?;
        self.columns = columns;
        self.rows = rows;
        Ok(())
    }

    /// The terminal's size in cells: `(columns, rows)`.
    #[must_use]
    pub const fn size(&self) -> (u16, u16) {
        (self.columns, self.rows)
    }

    /// Sets the default colours. A program's OSC overrides stay on top of
    /// them; a change of scheme is reported to a program that asked (2031).
    ///
    /// # Errors
    ///
    /// The library refused a colour.
    pub fn set_colors(&mut self, colors: &Colors) -> Result<(), Refused> {
        // SAFETY (all four): the input types the options document.
        unsafe { set_colors(self.raw(), colors) }?;
        let changed = self.sink().scheme.replace(colors.scheme) != Some(colors.scheme);
        if changed && self.mode(Mode::COLOR_SCHEME_REPORT) == Some(true) {
            let mut buf = [0u8; 32];
            let mut n = 0usize;
            // SAFETY: a buffer of its stated length.
            let r = unsafe {
                sys::ghostty_color_scheme_report_encode(
                    colors.scheme.raw(),
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                    &raw mut n,
                )
            };
            if r == sys::SUCCESS {
                self.sink()
                    .effects
                    .to_pty
                    .extend_from_slice(&buf[..n.min(buf.len())]);
            }
        }
        Ok(())
    }

    /// Sets a mode now, without changing what a reset restores.
    ///
    /// Ending a synchronized-output hold this way (the holder's one-second
    /// deadline) does NOT invoke the hold callback, so the hold is ended here
    /// too.
    ///
    /// # Errors
    ///
    /// The library does not know the mode.
    pub fn set_mode(&mut self, mode: Mode, on: bool) -> Result<(), Refused> {
        // SAFETY: `TerminalModeConfig` is MODE's input type.
        unsafe { set_mode(self.raw(), mode, on) }?;
        if mode == Mode::SYNC_OUTPUT && !on {
            self.sink().held = false;
        }
        Ok(())
    }

    /// A mode's current value, or `None` for a mode the library does not know.
    #[must_use]
    pub fn mode(&self, mode: Mode) -> Option<bool> {
        // SAFETY: a live terminal.
        unsafe { mode_of(self.raw(), mode) }
    }

    /// Whether a program holds the screen (synchronized output) right now.
    #[must_use]
    pub fn held(&self) -> bool {
        // SAFETY: a read between library calls.
        unsafe { self.sink.as_ref() }.held
    }

    /// Accepts OSC 52 writes (`true`) or refuses them. The holder accepts only
    /// while a window is attached to put them on the clipboard.
    pub fn accept_clipboard_writes(&mut self, accept: bool) {
        self.sink().clipboard_writes = accept;
    }

    /// Who answers OSC 52 reads, or `None` to refuse them all.
    ///
    /// A reader is also what kitty's paste events (mode 5522) need: with one
    /// installed, a program that turned that mode on is told a paste happened
    /// instead of receiving its text, and reads it back through the reader
    /// with its grant already set (paste.h). Without one, it gets the text.
    ///
    /// # Errors
    ///
    /// The library refused the callback.
    pub fn set_clipboard_reader(&mut self, reader: Option<ClipboardReader>) -> Result<(), Refused> {
        let callback: sys::Callback = if reader.is_some() {
            on_clipboard_read as sys::ClipboardReadFn as sys::Callback
        } else {
            null()
        };
        self.sink().clipboard_reader = reader;
        self.set(
            sys::TERMINAL_OPT_CLIPBOARD_READ,
            callback,
            "take clipboard reads",
        )
    }

    /// Encodes a key for the program, against the terminal's current modes
    /// (cursor keys, keypad, kitty flags, modifyOtherKeys, Alt as ESC).
    ///
    /// Empty when the key sends nothing (a bare modifier, a release the
    /// protocol does not report).
    pub fn encode_key(&mut self, key: &KeyInput) -> Vec<u8> {
        let action = match key.action {
            KeyAction::Press => sys::KEY_ACTION_PRESS,
            KeyAction::Repeat => sys::KEY_ACTION_REPEAT,
            KeyAction::Release => sys::KEY_ACTION_RELEASE,
        };
        // The text must be the UNMODIFIED character, and never a C0 control:
        // the header says to pass none and let the encoder use the key.
        let text = (!key.text.chars().any(char::is_control)).then_some(key.text.as_str());
        let (encoder, event) = (self.keys.encoder, self.keys.event);
        // SAFETY: live encoder, event and terminal; the text pointer is valid
        // until `encode` returns, which is as long as the event reads it.
        unsafe {
            sys::ghostty_key_encoder_setopt_from_terminal(encoder, self.raw());
            sys::ghostty_key_event_set_action(event, action);
            sys::ghostty_key_event_set_key(event, key.key as c_int);
            sys::ghostty_key_event_set_mods(event, key.mods.0);
            sys::ghostty_key_event_set_consumed_mods(event, key.consumed.0);
            sys::ghostty_key_event_set_composing(event, key.composing);
            match text {
                Some(t) if !t.is_empty() => {
                    sys::ghostty_key_event_set_utf8(event, t.as_ptr().cast(), t.len());
                }
                _ => sys::ghostty_key_event_set_utf8(event, null(), 0),
            }
            sys::ghostty_key_event_set_unshifted_codepoint(
                event,
                key.unshifted.map_or(0, u32::from),
            );
        }
        // SAFETY: as above; `encode_into` hands a buffer of its stated size.
        let out = encode_into(|buf, len, written| unsafe {
            sys::ghostty_key_encoder_encode(encoder, event, buf, len, written)
        });
        // The event borrows the text pointer; it must not outlive this call.
        // SAFETY: a live event.
        unsafe { sys::ghostty_key_event_set_utf8(event, null(), 0) };
        out
    }

    /// Encodes a mouse event for a program that tracks the mouse, in the
    /// format it asked for (X10, normal, button, any; SGR, urxvt, SGR-pixels).
    ///
    /// Empty when the program does not track this event.
    pub fn encode_mouse(&mut self, mouse: &MouseInput, geometry: &Geometry) -> Vec<u8> {
        let size = sys::MouseEncoderSize {
            size: size_of::<sys::MouseEncoderSize>(),
            screen_width: geometry.width_px,
            screen_height: geometry.height_px,
            cell_width: geometry.cell_width_px.max(1),
            cell_height: geometry.cell_height_px.max(1),
            padding_top: geometry.padding_top_px,
            padding_bottom: geometry.padding_bottom_px,
            padding_right: geometry.padding_right_px,
            padding_left: geometry.padding_left_px,
        };
        let action = match mouse.action {
            MouseAction::Press => sys::MOUSE_ACTION_PRESS,
            MouseAction::Release => sys::MOUSE_ACTION_RELEASE,
            MouseAction::Motion => sys::MOUSE_ACTION_MOTION,
        };
        let (encoder, event) = (self.mouse.encoder, self.mouse.event);
        let held = mouse.any_button_held;
        let track = true;
        // SAFETY: live handles; each option's value has its documented type.
        unsafe {
            sys::ghostty_mouse_encoder_setopt_from_terminal(encoder, self.raw());
            sys::ghostty_mouse_encoder_setopt(
                encoder,
                sys::MOUSE_ENCODER_OPT_SIZE,
                (&raw const size).cast(),
            );
            sys::ghostty_mouse_encoder_setopt(
                encoder,
                sys::MOUSE_ENCODER_OPT_ANY_BUTTON_PRESSED,
                (&raw const held).cast(),
            );
            // Motion inside one cell is reported once, as Ghostty does.
            sys::ghostty_mouse_encoder_setopt(
                encoder,
                sys::MOUSE_ENCODER_OPT_TRACK_LAST_CELL,
                (&raw const track).cast(),
            );
            sys::ghostty_mouse_event_set_action(event, action);
            match mouse.button {
                Some(b) => sys::ghostty_mouse_event_set_button(event, b.raw()),
                None => sys::ghostty_mouse_event_clear_button(event),
            }
            sys::ghostty_mouse_event_set_mods(event, mouse.mods.0);
            sys::ghostty_mouse_event_set_position(
                event,
                sys::MousePosition {
                    x: mouse.x_px,
                    y: mouse.y_px,
                },
            );
        }
        // SAFETY: as above.
        encode_into(|buf, len, written| unsafe {
            sys::ghostty_mouse_encoder_encode(encoder, event, buf, len, written)
        })
    }

    /// Encodes a focus change — or nothing, when the program did not ask to
    /// be told (mode 1004 off): the encoder does not look at the mode.
    #[must_use]
    pub fn encode_focus(&self, gained: bool) -> Vec<u8> {
        if self.mode(Mode::FOCUS_EVENT) != Some(true) {
            return Vec::new();
        }
        let event = if gained {
            sys::FOCUS_GAINED
        } else {
            sys::FOCUS_LOST
        };
        // SAFETY: a buffer of its stated size.
        encode_into(|buf, len, written| unsafe {
            sys::ghostty_focus_encode(event, buf, len, written)
        })
    }

    /// Pastes text into the program, as the terminal's modes say: bracketed
    /// (2004) or with line breaks turned into returns, control bytes replaced,
    /// or as a kitty paste event (5522) when a clipboard reader is installed.
    ///
    /// The encoded bytes go to [`Effects::to_pty`], in order with the replies.
    /// `Ok(false)` means there was nothing to paste.
    ///
    /// # Errors
    ///
    /// [`PasteRefused::Unsafe`] when the text could inject a command and
    /// `allow_unsafe` is false — nothing was written.
    pub fn paste(
        &mut self,
        text: &str,
        source: PasteSource,
        allow_unsafe: bool,
    ) -> Result<bool, PasteRefused> {
        let mime = borrowed(b"text/plain");
        let mut data = text.as_bytes();
        let paste = sys::Paste {
            size: size_of::<sys::Paste>(),
            location: sys::CLIPBOARD_LOCATION_STANDARD,
            source: match source {
                PasteSource::Clipboard => sys::PASTE_SOURCE_CLIPBOARD,
                PasteSource::Text => sys::PASTE_SOURCE_TEXT,
            },
            mimes: &raw const mime,
            mimes_len: 1,
            reader: sys::MimeReader {
                read: Some(read_paste),
                userdata: (&raw mut data).cast(),
            },
            allow_unsafe,
        };
        let mut written = false;
        // SAFETY: every pointer in `paste` outlives the call; the output
        // streams through WRITE_PTY into the sink.
        let r =
            unsafe { sys::ghostty_terminal_paste(self.raw(), &raw const paste, &raw mut written) };
        match r {
            sys::SUCCESS => Ok(written),
            sys::REJECTED => Err(PasteRefused::Unsafe),
            code => Err(PasteRefused::Library(Refused {
                what: "paste",
                code,
            })),
        }
    }

    /// The whole terminal as a snapshot: what a window decodes into a
    /// [`Mirror`] before the live bytes that follow it.
    ///
    /// Taken between two feeds; an escape sequence cut by the boundary is in
    /// it whole, because continuation tracking was on from the first byte.
    ///
    /// # Errors
    ///
    /// The library could not encode it.
    pub fn snapshot(&self) -> Result<Vec<u8>, Refused> {
        let mut out = Vec::new();
        let writer = sys::Writer {
            write: Some(write_into_vec),
            userdata: (&raw mut out).cast(),
        };
        // SAFETY: the writer's userdata is `out`, alive for the call.
        check(
            unsafe { sys::ghostty_snapshot_encode(self.raw(), writer) },
            "encode a snapshot",
        )?;
        Ok(out)
    }

    /// The terminal written out by the formatter.
    ///
    /// # Errors
    ///
    /// The library could not format it.
    pub fn format(&self, format: Format) -> Result<Vec<u8>, Refused> {
        // SAFETY: a live terminal, used by nobody else (`Vt` is not `Sync`).
        unsafe { format_terminal(self.raw(), format) }
    }

    /// What the program said since the last call: the bytes owed to the PTY
    /// and the effects, both in order.
    pub fn take_effects(&mut self) -> Effects {
        std::mem::take(&mut self.sink().effects)
    }

    /// The cursor.
    #[must_use]
    pub fn cursor(&self) -> Cursor {
        // SAFETY: CURSOR_X/Y are `uint16_t`, VISIBLE and PENDING_WRAP `bool`.
        unsafe { cursor_of(self.raw()) }
    }

    /// The title a program set (OSC 0/2), empty if none.
    #[must_use]
    pub fn title(&self) -> String {
        // SAFETY: a live terminal.
        unsafe { string_of(self.raw(), sys::TERMINAL_DATA_TITLE) }
    }

    /// The working directory a shell reported (OSC 7), as it sent it.
    #[must_use]
    pub fn pwd(&self) -> String {
        // SAFETY: a live terminal.
        unsafe { string_of(self.raw(), sys::TERMINAL_DATA_PWD) }
    }

    /// The kitty keyboard flags the program pushed.
    #[must_use]
    pub fn kitty_keyboard_flags(&self) -> u8 {
        // SAFETY: `GhosttyKittyKeyFlags` is a `uint8_t`.
        unsafe { get(self.raw(), sys::TERMINAL_DATA_KITTY_KEYBOARD_FLAGS, 0u8) }.unwrap_or(0)
    }

    /// Whether the program tracks the mouse (any of X10, normal, button, any).
    #[must_use]
    pub fn mouse_tracking(&self) -> bool {
        // SAFETY: a `bool`.
        unsafe { get(self.raw(), sys::TERMINAL_DATA_MOUSE_TRACKING, false) }.unwrap_or(false)
    }

    /// The scrollable area.
    #[must_use]
    pub fn scrollbar(&self) -> Scrollbar {
        // SAFETY: a live terminal.
        unsafe { scrollbar_of(self.raw()) }
    }

    /// The token that changes when anything compression cares about changed:
    /// the holder restarts its idle timer when it moves.
    #[must_use]
    pub fn compression_activity(&self) -> u64 {
        let mut out = 0u64;
        // SAFETY: an out pointer of the documented type.
        unsafe { sys::ghostty_terminal_compression_activity(self.raw(), &raw mut out) };
        out
    }

    /// One bounded step of scrollback compression; `true` while more remains.
    pub fn compress_step(&mut self) -> bool {
        let mut result = sys::TERMINAL_COMPRESSION_RESULT_COMPLETE;
        // SAFETY: a live terminal, serialised by `&mut self`.
        let r = unsafe {
            sys::ghostty_terminal_compress(
                self.raw(),
                sys::TERMINAL_COMPRESSION_MODE_INCREMENTAL,
                &raw mut result,
            )
        };
        r == sys::SUCCESS && result == sys::TERMINAL_COMPRESSION_RESULT_PENDING
    }
}

impl Drop for Vt {
    fn drop(&mut self) {
        // SAFETY: each handle is freed once, the terminal before the sink its
        // callbacks write — nothing can call back after the terminal is gone.
        unsafe {
            sys::ghostty_key_event_free(self.keys.event);
            sys::ghostty_key_encoder_free(self.keys.encoder);
            sys::ghostty_mouse_event_free(self.mouse.event);
            sys::ghostty_mouse_encoder_free(self.mouse.encoder);
            sys::ghostty_terminal_free(self.raw());
            drop(Box::from_raw(self.sink.as_ptr()));
        }
    }
}

/// The paste's MIME reader: writes the text through the writer it is given.
unsafe extern "C" fn read_paste(
    userdata: *mut c_void,
    _mime: sys::String,
    writer: sys::Writer,
) -> bool {
    guarded(false, || {
        // SAFETY: `userdata` is the `&[u8]` `Vt::paste` set, alive for the call.
        let data: &[u8] = unsafe { *userdata.cast::<&[u8]>() };
        if data.is_empty() {
            return true;
        }
        let Some(write) = writer.write else {
            return false;
        };
        // SAFETY: the library's writer, called during the read as documented.
        unsafe { write(writer.userdata, data.as_ptr(), data.len()) }
    })
}

/// Runs an encoder that writes into a caller buffer, retrying once with the
/// size it asks for when the first one is short.
fn encode_into(mut encode: impl FnMut(*mut c_char, usize, *mut usize) -> c_int) -> Vec<u8> {
    let mut buf = vec![0u8; 128];
    let mut written = 0usize;
    let mut r = encode(buf.as_mut_ptr().cast(), buf.len(), &raw mut written);
    if r == sys::OUT_OF_SPACE && written > buf.len() {
        buf.resize(written, 0);
        r = encode(buf.as_mut_ptr().cast(), buf.len(), &raw mut written);
    }
    if r != sys::SUCCESS {
        return Vec::new();
    }
    buf.truncate(written.min(buf.len()));
    buf
}

/// Sets the default colours of a terminal.
///
/// # Safety
///
/// `terminal` must be alive and not used concurrently.
unsafe fn set_colors(terminal: sys::Handle, colors: &Colors) -> Result<(), Refused> {
    let palette: &[Rgb; 256] = &colors.palette;
    // SAFETY: the caller's promise; each value has its option's input type
    // (`GhosttyColorRgb*`, or `GhosttyColorRgb[256]*`); NULL clears the cursor.
    unsafe {
        check(
            sys::ghostty_terminal_set(
                terminal,
                sys::TERMINAL_OPT_COLOR_FOREGROUND,
                (&raw const colors.foreground).cast(),
            ),
            "set the foreground",
        )?;
        check(
            sys::ghostty_terminal_set(
                terminal,
                sys::TERMINAL_OPT_COLOR_BACKGROUND,
                (&raw const colors.background).cast(),
            ),
            "set the background",
        )?;
        let cursor = colors
            .cursor
            .as_ref()
            .map_or(null(), |c| std::ptr::from_ref(c).cast());
        check(
            sys::ghostty_terminal_set(terminal, sys::TERMINAL_OPT_COLOR_CURSOR, cursor),
            "set the cursor colour",
        )?;
        check(
            sys::ghostty_terminal_set(
                terminal,
                sys::TERMINAL_OPT_COLOR_PALETTE,
                std::ptr::from_ref(palette).cast(),
            ),
            "set the palette",
        )
    }
}

/// # Safety
///
/// `terminal` must be alive and not used concurrently.
unsafe fn set_mode(terminal: sys::Handle, mode: Mode, on: bool) -> Result<(), Refused> {
    let config = sys::TerminalModeConfig {
        mode: mode.0,
        value: on,
    };
    // SAFETY: the caller's promise; MODE's input type.
    check(
        unsafe {
            sys::ghostty_terminal_set(terminal, sys::TERMINAL_OPT_MODE, (&raw const config).cast())
        },
        "set a mode",
    )
}

/// Makes `cursor` what the terminal's DECSCUSR 0 goes back to: the two
/// options the reset reads, and nothing about the cursor of now.
///
/// # Safety
///
/// `terminal` must be alive and not used concurrently.
unsafe fn default_cursor_of(terminal: sys::Handle, cursor: CursorDefault) -> Result<(), Refused> {
    let style = cursor.style();
    let blinks = cursor.blinks;
    // SAFETY: the caller's promise; each value has its option's input type
    // (`GhosttyTerminalCursorStyle*`, `bool*`).
    unsafe {
        check(
            sys::ghostty_terminal_set(
                terminal,
                sys::TERMINAL_OPT_DEFAULT_CURSOR_STYLE,
                (&raw const style).cast(),
            ),
            "take the default cursor",
        )?;
        check(
            sys::ghostty_terminal_set(
                terminal,
                sys::TERMINAL_OPT_DEFAULT_CURSOR_BLINK,
                (&raw const blinks).cast(),
            ),
            "take the default cursor's blink",
        )
    }
}

/// # Safety
///
/// `terminal` must be alive and not used concurrently.
unsafe fn mode_of(terminal: sys::Handle, mode: Mode) -> Option<bool> {
    let config = sys::TerminalModeConfig {
        mode: mode.0,
        value: false,
    };
    // SAFETY: the caller's promise; MODE is an in/out `TerminalModeConfig`.
    unsafe { get(terminal, sys::TERMINAL_DATA_MODE, config) }.map(|c| c.value)
}

/// # Safety
///
/// `terminal` must be alive and not used concurrently.
unsafe fn cursor_of(terminal: sys::Handle) -> Cursor {
    // SAFETY: the caller's promise and the documented types.
    unsafe {
        Cursor {
            x: get(terminal, sys::TERMINAL_DATA_CURSOR_X, 0u16).unwrap_or(0),
            y: get(terminal, sys::TERMINAL_DATA_CURSOR_Y, 0u16).unwrap_or(0),
            visible: get(terminal, sys::TERMINAL_DATA_CURSOR_VISIBLE, true).unwrap_or(true),
            pending_wrap: get(terminal, sys::TERMINAL_DATA_CURSOR_PENDING_WRAP, false)
                .unwrap_or(false),
        }
    }
}

/// # Safety
///
/// `terminal` must be alive and not used concurrently; `data` must be a
/// field whose output type is `GhosttyString`.
unsafe fn string_of(terminal: sys::Handle, data: c_int) -> String {
    let empty = sys::String {
        ptr: null(),
        len: 0,
    };
    // SAFETY: the caller's promise; the string is copied before any other
    // call can invalidate it.
    unsafe {
        get(terminal, data, empty)
            .map(|s| text_of(s))
            .unwrap_or_default()
    }
}

/// # Safety
///
/// `terminal` must be alive and not used concurrently.
unsafe fn scrollbar_of(terminal: sys::Handle) -> Scrollbar {
    // SAFETY: SCROLLBAR's documented type.
    let s = unsafe {
        get(
            terminal,
            sys::TERMINAL_DATA_SCROLLBAR,
            sys::TerminalScrollbar::default(),
        )
    }
    .unwrap_or_default();
    Scrollbar {
        total: s.total,
        offset: s.offset,
        len: s.len,
    }
}

// ===========================================================================
// A cell as the program wrote it.
// ===========================================================================

/// A colour as the program asked for it, before any palette resolves it.
///
/// The renderer wants the RESOLVED colour ([`Cell::fg`]); a comparison of two
/// engines wants the program's own word. `SGR 31` is palette entry 1 in every
/// engine, while what entry 1 looks like is whatever palette each was handed —
/// so resolved colours would make two engines with different palettes
/// disagree about a cell they read identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AskedColour {
    /// No colour was asked for: the terminal's own foreground or background.
    #[default]
    Default,
    /// An entry of the 256-colour palette.
    Palette(u8),
    /// A direct colour.
    Rgb(Rgb),
}

/// One cell of the active area as the program left it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Written {
    /// The grapheme cluster: a base character and whatever combines with it.
    /// Empty for a cell with no text — an erased one, a wide character's
    /// spacer.
    pub text: String,
    /// Its attributes.
    pub style: CellStyle,
    /// The foreground, as asked.
    pub fg: AskedColour,
    /// The background, as asked. A cell erased under a background colour and
    /// holding no text carries it too.
    pub bg: AskedColour,
    /// How it takes columns.
    pub width: CellWidth,
}

impl Vt {
    /// What the program left at `column`, `row` of the active area; `None`
    /// outside it.
    ///
    /// A question about the TERMINAL, asked through a grid ref, and not about
    /// a frame: a render state answers in resolved colours only, and the
    /// questions this answers — the differential's second opinion
    /// (`ronoctua-term-differential`) and the tests of this binding — are
    /// about what the program asked for.
    #[must_use]
    pub fn written(&self, column: u16, row: u16) -> Option<Written> {
        if column >= self.columns || row >= self.rows {
            return None;
        }
        let point = sys::Point {
            tag: sys::POINT_TAG_ACTIVE,
            value: sys::PointValue {
                coordinate: sys::PointCoordinate {
                    x: column,
                    y: u32::from(row),
                },
            },
        };
        let mut reference = sys::GridRef::sized();
        // SAFETY: a live terminal; the ref is read before any mutation.
        if unsafe { sys::ghostty_terminal_grid_ref(self.raw(), point, &raw mut reference) }
            != sys::SUCCESS
        {
            return None;
        }
        let mut cell = 0u64;
        // SAFETY: a ref just obtained.
        if unsafe { sys::ghostty_grid_ref_cell(&raw const reference, &raw mut cell) }
            != sys::SUCCESS
        {
            return None;
        }
        // SAFETY: each field's documented output type.
        let (tag, wide, has_text, styled) = unsafe {
            (
                cell_field(
                    cell,
                    sys::CELL_DATA_CONTENT_TAG,
                    sys::CELL_CONTENT_CODEPOINT,
                ),
                cell_field(cell, sys::CELL_DATA_WIDE, sys::CELL_WIDE_NARROW),
                cell_field(cell, sys::CELL_DATA_HAS_TEXT, false),
                cell_field(cell, sys::CELL_DATA_HAS_STYLING, false),
            )
        };
        let text = if has_text {
            // SAFETY: the same ref, still valid.
            unsafe { graphemes_at(&reference) }
        } else {
            String::new()
        };
        let mut written = Written {
            text,
            width: cell_width(wide),
            ..Written::default()
        };
        if styled {
            let mut style = sys::Style::sized();
            // SAFETY: the same ref; a sized style.
            if unsafe { sys::ghostty_grid_ref_style(&raw const reference, &raw mut style) }
                == sys::SUCCESS
            {
                written.style = cell_style(&style);
                written.fg = asked(style.fg_color);
                written.bg = asked(style.bg_color);
            }
        }
        // A cell erased with a background and no text carries the colour in
        // its CONTENT, not in a style.
        // SAFETY: the documented output types of the two colour fields.
        match tag {
            sys::CELL_CONTENT_BG_COLOR_PALETTE => {
                written.bg = AskedColour::Palette(unsafe {
                    cell_field(cell, sys::CELL_DATA_COLOR_PALETTE, 0u8)
                });
            }
            sys::CELL_CONTENT_BG_COLOR_RGB => {
                written.bg = AskedColour::Rgb(unsafe {
                    cell_field(cell, sys::CELL_DATA_COLOR_RGB, Rgb::default())
                });
            }
            _ => {}
        }
        Some(written)
    }
}

/// One field of a packed cell.
///
/// # Safety
///
/// `T` must be the output type the header documents for `data`.
unsafe fn cell_field<T: Copy>(cell: u64, data: c_int, initial: T) -> T {
    let mut out = initial;
    // SAFETY: the caller's promise about `T`.
    if unsafe { sys::ghostty_cell_get(cell, data, (&raw mut out).cast()) } == sys::SUCCESS {
        out
    } else {
        initial
    }
}

/// The grapheme at a grid ref, as text.
///
/// # Safety
///
/// `reference` must be a ref obtained since the terminal's last mutation.
unsafe fn graphemes_at(reference: &sys::GridRef) -> String {
    let mut buf = [0u32; 16];
    let mut len = 0usize;
    // SAFETY: the caller's promise; a buffer of its stated length.
    let mut r = unsafe {
        sys::ghostty_grid_ref_graphemes(reference, buf.as_mut_ptr(), buf.len(), &raw mut len)
    };
    let mut long = Vec::new();
    let codepoints: &[u32] = if r == sys::OUT_OF_SPACE {
        long.resize(len, 0);
        // SAFETY: retried with the size the library asked for.
        r = unsafe {
            sys::ghostty_grid_ref_graphemes(reference, long.as_mut_ptr(), long.len(), &raw mut len)
        };
        &long[..len.min(long.len())]
    } else {
        &buf[..len.min(buf.len())]
    };
    if r != sys::SUCCESS {
        return String::new();
    }
    codepoints
        .iter()
        .filter_map(|c| char::from_u32(*c))
        .collect()
}

/// A style colour as the program asked for it.
fn asked(c: sys::StyleColor) -> AskedColour {
    match c.tag {
        // SAFETY: the tag says which arm of the union is filled.
        sys::STYLE_COLOR_PALETTE => AskedColour::Palette(unsafe { c.value.palette }),
        // SAFETY: as above.
        sys::STYLE_COLOR_RGB => AskedColour::Rgb(unsafe { c.value.rgb }),
        _ => AskedColour::Default,
    }
}

/// The attributes of a library style. One reading for a frame's cells and for
/// [`Vt::written`], so the two cannot disagree about which bit is which.
const fn cell_style(style: &sys::Style) -> CellStyle {
    CellStyle {
        bold: style.bold,
        italic: style.italic,
        faint: style.faint,
        blink: style.blink,
        inverse: style.inverse,
        invisible: style.invisible,
        strikethrough: style.strikethrough,
        overline: style.overline,
        underline: match style.underline {
            sys::SGR_UNDERLINE_SINGLE => Underline::Single,
            sys::SGR_UNDERLINE_DOUBLE => Underline::Double,
            sys::SGR_UNDERLINE_CURLY => Underline::Curly,
            sys::SGR_UNDERLINE_DOTTED => Underline::Dotted,
            sys::SGR_UNDERLINE_DASHED => Underline::Dashed,
            _ => Underline::None,
        },
    }
}

/// A cell's width, from the library's `CELL_DATA_WIDE`.
const fn cell_width(wide: c_int) -> CellWidth {
    match wide {
        sys::CELL_WIDE_WIDE => CellWidth::Wide,
        sys::CELL_WIDE_SPACER_TAIL => CellWidth::SpacerTail,
        sys::CELL_WIDE_SPACER_HEAD => CellWidth::SpacerHead,
        _ => CellWidth::Narrow,
    }
}

// ===========================================================================
// The window's copy.
// ===========================================================================

/// Where a mirror's ONE callback writes.
struct MirrorSink {
    /// Synchronized output holds the screen.
    held: bool,
    /// The render state to capture the held frame into, when a window asked
    /// (`Mirror::capture_holds`).
    capture: Option<Arc<RenderCell>>,
}

/// A mirror's terminal, shared with its [`Search`] and its [`Decoder`].
///
/// `gate` serialises every call that touches the terminal, so the three may
/// sit on different threads; the library's own rule ("no concurrent use") is
/// then kept by the lock rather than by hope.
struct Shared {
    terminal: NonNull<c_void>,
    sink: NonNull<MirrorSink>,
    gate: Mutex<()>,
}

// SAFETY: every access to `terminal` takes `gate`; the sink is written only
// from the callback, which runs inside a gated call, and read under the gate.
unsafe impl Send for Shared {}
// SAFETY: as above — `&Shared` gives access only through the gate.
unsafe impl Sync for Shared {}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, ()> {
        self.gate.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn raw(&self) -> sys::Handle {
        self.terminal.as_ptr()
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        // SAFETY: the last owner; the terminal goes before the sink its
        // callback writes.
        unsafe {
            sys::ghostty_terminal_free(self.raw());
            drop(Box::from_raw(self.sink.as_ptr()));
        }
    }
}

unsafe extern "C" fn on_mirror_hold(terminal: sys::Handle, ud: *mut c_void, held: bool) {
    guarded((), || {
        // SAFETY: `ud` is the mirror's sink, borrowed by nothing else during
        // the gated call this runs inside.
        let sink = unsafe { &mut *ud.cast::<MirrorSink>() };
        sink.held = held;
        // CAPTURE the frame the program wants left on screen: nothing after
        // the start of the hold has been processed yet, and the header says
        // to update the render state from inside this callback. Waiting until
        // the write returns would capture the half-drawn frame the hold is
        // there to hide.
        if held && let Some(cell) = &sink.capture {
            let inner = cell.lock();
            // SAFETY: the render state is ours, locked; the terminal is the
            // one calling us, so it is alive and ours for the call.
            unsafe { sys::ghostty_render_state_update(inner.state, terminal) };
        }
    });
}

/// The callbacks a mirror may take — and NOT the PTY writer.
///
/// This enum is the type-level half of the promise that a window never writes
/// to the PTY: every callback a [`Mirror`] installs is a variant here, there
/// is no variant for `WRITE_PTY` or any other callback that answers the
/// program, and nothing public takes one. The other half is a test that asks
/// the LIBRARY whether a writer is installed
/// (`a_mirror_has_no_way_to_write_to_the_pty`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MirrorOption {
    /// The render-hold callback, which only captures a frame.
    RenderHold,
}

/// A window's copy of a terminal: decoded from a snapshot, then fed the same
/// bytes the holder's [`Vt`] was fed after it.
///
/// It draws, scrolls, selects and searches; it never answers the program —
/// queries reaching it fall on the floor, because the [`Vt`] already answered
/// them.
pub struct Mirror {
    shared: Arc<Shared>,
    gesture: Option<Gesture>,
    columns: u16,
    rows: u16,
}

impl Mirror {
    /// Takes the decoded terminal and installs the mirror's callback — the
    /// only place one is installed, and it goes through `MirrorOption`.
    fn from_decoded(shared: Arc<Shared>) -> Result<Self, Refused> {
        // SAFETY: COLS/ROWS are `uint16_t`, read under the gate.
        let (columns, rows) = {
            let _gate = shared.lock();
            unsafe {
                (
                    get(shared.raw(), sys::TERMINAL_DATA_COLS, 0u16).unwrap_or(1),
                    get(shared.raw(), sys::TERMINAL_DATA_ROWS, 0u16).unwrap_or(1),
                )
            }
        };
        let mirror = Self {
            shared,
            gesture: None,
            columns,
            rows,
        };
        mirror.install(MirrorOption::RenderHold)?;
        Ok(mirror)
    }

    fn install(&self, option: MirrorOption) -> Result<(), Refused> {
        let _gate = self.shared.lock();
        let raw = self.shared.raw();
        let userdata = self.shared.sink.as_ptr().cast::<c_void>().cast_const();
        // SAFETY: the input types the options document, under the gate.
        unsafe {
            check(
                sys::ghostty_terminal_set(raw, sys::TERMINAL_OPT_USERDATA, userdata),
                "take the mirror's userdata",
            )?;
            match option {
                MirrorOption::RenderHold => check(
                    sys::ghostty_terminal_set(
                        raw,
                        sys::TERMINAL_OPT_RENDER_HOLD,
                        on_mirror_hold as sys::RenderHoldFn as sys::Callback,
                    ),
                    "take render holds",
                ),
            }
        }
    }

    /// Runs `f` on the mirror's sink. The caller holds the gate (`_gate`
    /// proves it), so no callback is running and nothing else borrows it.
    fn with_sink<R>(&self, _gate: &MutexGuard<'_, ()>, f: impl FnOnce(&mut MirrorSink) -> R) -> R {
        // SAFETY: as above — the gate serialises every library call, and the
        // callback is the sink's only other user.
        f(unsafe { &mut *self.shared.sink.as_ptr() })
    }

    /// Feeds the bytes the holder's engine was fed.
    pub fn feed(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let _gate = self.shared.lock();
        // SAFETY: a live terminal under the gate.
        unsafe { sys::ghostty_terminal_vt_write(self.shared.raw(), bytes.as_ptr(), bytes.len()) };
    }

    /// Resizes — when the holder said it resized, at that point of the
    /// stream, never on the window's own account.
    ///
    /// # Errors
    ///
    /// The library refused the size.
    pub fn resize(
        &mut self,
        columns: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<(), Refused> {
        let (columns, rows) = (columns.max(1), rows.max(1));
        let _gate = self.shared.lock();
        // SAFETY: a live terminal under the gate.
        check(
            unsafe {
                sys::ghostty_terminal_resize(
                    self.shared.raw(),
                    columns,
                    rows,
                    cell_width_px,
                    cell_height_px,
                )
            },
            "resize the mirror",
        )?;
        self.columns = columns;
        self.rows = rows;
        Ok(())
    }

    /// The mirror's size in cells: `(columns, rows)`.
    #[must_use]
    pub const fn size(&self) -> (u16, u16) {
        (self.columns, self.rows)
    }

    /// Sets the default colours, as the holder did.
    ///
    /// # Errors
    ///
    /// The library refused a colour.
    pub fn set_colors(&mut self, colors: &Colors) -> Result<(), Refused> {
        let _gate = self.shared.lock();
        // SAFETY: a live terminal under the gate.
        unsafe { set_colors(self.shared.raw(), colors) }
    }

    /// Sets a mode, as the holder did (the end of a hold it timed out).
    ///
    /// # Errors
    ///
    /// The library does not know the mode.
    pub fn set_mode(&mut self, mode: Mode, on: bool) -> Result<(), Refused> {
        let gate = self.shared.lock();
        // SAFETY: a live terminal under the gate.
        unsafe { set_mode(self.shared.raw(), mode, on) }?;
        // Setting the mode does not call the hold callback; the hold ends here.
        if mode == Mode::SYNC_OUTPUT && !on {
            self.with_sink(&gate, |sink| sink.held = false);
        }
        Ok(())
    }

    /// A mode's current value.
    #[must_use]
    pub fn mode(&self, mode: Mode) -> Option<bool> {
        let _gate = self.shared.lock();
        // SAFETY: a live terminal under the gate.
        unsafe { mode_of(self.shared.raw(), mode) }
    }

    /// Whether the program holds the screen; the window keeps drawing the
    /// frame captured when the hold began.
    #[must_use]
    pub fn held(&self) -> bool {
        let gate = self.shared.lock();
        self.with_sink(&gate, |sink| sink.held)
    }

    /// Captures the held frame into `state` from now on: when a program
    /// begins synchronized output, the frame it wants left on screen is
    /// written into `state` at that very byte, and [`RenderState::begin`]
    /// leaves it alone until the hold ends.
    pub fn capture_holds(&mut self, state: &RenderState) {
        let gate = self.shared.lock();
        let cell = Arc::clone(&state.cell);
        self.with_sink(&gate, |sink| sink.capture = Some(cell));
    }

    /// The cursor.
    #[must_use]
    pub fn cursor(&self) -> Cursor {
        let _gate = self.shared.lock();
        // SAFETY: a live terminal under the gate.
        unsafe { cursor_of(self.shared.raw()) }
    }

    /// The title, as the snapshot and the bytes since left it.
    #[must_use]
    pub fn title(&self) -> String {
        let _gate = self.shared.lock();
        // SAFETY: a live terminal under the gate.
        unsafe { string_of(self.shared.raw(), sys::TERMINAL_DATA_TITLE) }
    }

    /// Whether the program tracks the mouse — the same answer the holder
    /// gives, because the modes are the same bytes' result.
    #[must_use]
    pub fn mouse_tracking(&self) -> bool {
        let _gate = self.shared.lock();
        // SAFETY: a `bool`, under the gate.
        unsafe { get(self.shared.raw(), sys::TERMINAL_DATA_MOUSE_TRACKING, false) }.unwrap_or(false)
    }

    /// Scrolls the viewport. The window's own business: the holder's screen
    /// does not move.
    pub fn scroll(&mut self, scroll: Scroll) {
        let (tag, value) = match scroll {
            Scroll::Top => (
                sys::SCROLL_VIEWPORT_TOP,
                sys::TerminalScrollViewportValue { _padding: [0; 2] },
            ),
            Scroll::Bottom => (
                sys::SCROLL_VIEWPORT_BOTTOM,
                sys::TerminalScrollViewportValue { _padding: [0; 2] },
            ),
            Scroll::Lines(delta) => (
                sys::SCROLL_VIEWPORT_DELTA,
                sys::TerminalScrollViewportValue { delta },
            ),
            Scroll::Row(row) => (
                sys::SCROLL_VIEWPORT_ROW,
                sys::TerminalScrollViewportValue { row },
            ),
            Scroll::Prompts(delta_prompt) => (
                sys::SCROLL_VIEWPORT_DELTA_PROMPT,
                sys::TerminalScrollViewportValue { delta_prompt },
            ),
        };
        let _gate = self.shared.lock();
        // SAFETY: a live terminal under the gate; the tag names the arm.
        unsafe {
            sys::ghostty_terminal_scroll_viewport(
                self.shared.raw(),
                sys::TerminalScrollViewport { tag, value },
            );
        }
    }

    /// The scrollable area, for the scrollbar.
    #[must_use]
    pub fn scrollbar(&self) -> Scrollbar {
        let _gate = self.shared.lock();
        // SAFETY: a live terminal under the gate.
        unsafe { scrollbar_of(self.shared.raw()) }
    }

    /// Whether the viewport follows the live screen (not scrolled back).
    #[must_use]
    pub fn at_bottom(&self) -> bool {
        let _gate = self.shared.lock();
        // SAFETY: a `bool`, under the gate.
        unsafe { get(self.shared.raw(), sys::TERMINAL_DATA_VIEWPORT_ACTIVE, true) }.unwrap_or(true)
    }

    /// The terminal written out by the formatter.
    ///
    /// # Errors
    ///
    /// The library could not format it.
    pub fn format(&self, format: Format) -> Result<Vec<u8>, Refused> {
        let _gate = self.shared.lock();
        // SAFETY: a live terminal under the gate.
        unsafe { format_terminal(self.shared.raw(), format) }
    }

    /// The OSC 8 link of the cell at a viewport position, if it has one.
    #[must_use]
    pub fn link_at(&self, column: u16, row: u16) -> Option<String> {
        let _gate = self.shared.lock();
        // SAFETY: a live terminal under the gate.
        let reference = unsafe { viewport_ref(self.shared.raw(), column, row) }?;
        let mut buf = vec![0u8; 256];
        let mut len = 0usize;
        // SAFETY: the ref was just made under the same gate; a buffer of its
        // stated length, retried once with the length asked for.
        let mut r = unsafe {
            sys::ghostty_grid_ref_hyperlink_uri(
                &raw const reference,
                buf.as_mut_ptr(),
                buf.len(),
                &raw mut len,
            )
        };
        if r == sys::OUT_OF_SPACE {
            buf.resize(len, 0);
            r = unsafe {
                sys::ghostty_grid_ref_hyperlink_uri(
                    &raw const reference,
                    buf.as_mut_ptr(),
                    buf.len(),
                    &raw mut len,
                )
            };
        }
        if r != sys::SUCCESS || len == 0 {
            return None;
        }
        buf.truncate(len);
        Some(String::from_utf8_lossy(&buf).into_owned())
    }

    /// A kitty image's pixels and shape, by id.
    #[must_use]
    pub fn image(&self, id: u32) -> Option<Image> {
        let _gate = self.shared.lock();
        // SAFETY: a live terminal under the gate.
        unsafe { image_of(self.shared.raw(), id) }
    }
}

impl Drop for Mirror {
    fn drop(&mut self) {
        if let Some(g) = self.gesture.take() {
            let _gate = self.shared.lock();
            // SAFETY: the gesture's tracked refs are released against the
            // terminal that made them, alive (we hold it) and gated.
            unsafe { g.free(self.shared.raw()) };
        }
    }
}

/// Where to scroll the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scroll {
    /// The top of the scrollback.
    Top,
    /// The live screen.
    Bottom,
    /// By rows, up negative.
    Lines(isize),
    /// To a row of the scrollbar's space ([`Scrollbar::offset`]).
    Row(usize),
    /// By prompts (OSC 133), back negative.
    Prompts(isize),
}

/// A grid ref at a viewport position.
///
/// # Safety
///
/// `terminal` must be alive and not used concurrently; the ref is valid only
/// until the next mutation.
unsafe fn viewport_ref(terminal: sys::Handle, column: u16, row: u16) -> Option<sys::GridRef> {
    let point = sys::Point {
        tag: sys::POINT_TAG_VIEWPORT,
        value: sys::PointValue {
            coordinate: sys::PointCoordinate {
                x: column,
                y: u32::from(row),
            },
        },
    };
    let mut reference = sys::GridRef::sized();
    // SAFETY: the caller's promise.
    (unsafe { sys::ghostty_terminal_grid_ref(terminal, point, &raw mut reference) } == sys::SUCCESS)
        .then_some(reference)
}

/// How a kitty image stores its pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// Three bytes a pixel.
    Rgb,
    /// Four bytes a pixel.
    Rgba,
    /// Two bytes a pixel.
    GrayAlpha,
    /// One byte a pixel.
    Gray,
}

/// A kitty image, read out of the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Its id.
    pub id: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// How the pixels are laid out.
    pub format: PixelFormat,
    /// Changes whenever the pixels may have; never reused, never 0.
    pub generation: u64,
    /// The decoded pixels — `None` while a snapshot's pixels are still on
    /// their way.
    pub pixels: Option<Vec<u8>>,
}

/// # Safety
///
/// `terminal` must be alive and not used concurrently.
unsafe fn image_of(terminal: sys::Handle, id: u32) -> Option<Image> {
    // SAFETY (all): the caller's promise, and each field's documented type;
    // the pixels are copied before any call could invalidate them.
    unsafe {
        let graphics = get(
            terminal,
            sys::TERMINAL_DATA_KITTY_GRAPHICS,
            null_mut::<c_void>(),
        )?;
        if graphics.is_null() {
            return None;
        }
        let image = sys::ghostty_kitty_graphics_image(graphics, id);
        if image.is_null() {
            return None;
        }
        let field = |data: c_int| -> u32 {
            let mut out = 0u32;
            sys::ghostty_kitty_graphics_image_get(image, data, (&raw mut out).cast());
            out
        };
        let mut format = sys::KITTY_IMAGE_FORMAT_RGBA;
        sys::ghostty_kitty_graphics_image_get(
            image,
            sys::KITTY_IMAGE_DATA_FORMAT,
            (&raw mut format).cast(),
        );
        let mut generation = 0u64;
        sys::ghostty_kitty_graphics_image_get(
            image,
            sys::KITTY_IMAGE_DATA_GENERATION,
            (&raw mut generation).cast(),
        );
        let mut data = null::<u8>();
        let mut len = 0usize;
        let pixels = (sys::ghostty_kitty_graphics_image_get(
            image,
            sys::KITTY_IMAGE_DATA_DATA_PTR,
            (&raw mut data).cast(),
        ) == sys::SUCCESS
            && sys::ghostty_kitty_graphics_image_get(
                image,
                sys::KITTY_IMAGE_DATA_DATA_LEN,
                (&raw mut len).cast(),
            ) == sys::SUCCESS
            && !data.is_null())
        .then(|| std::slice::from_raw_parts(data, len).to_vec());
        Some(Image {
            id,
            width: field(sys::KITTY_IMAGE_DATA_WIDTH),
            height: field(sys::KITTY_IMAGE_DATA_HEIGHT),
            format: match format {
                sys::KITTY_IMAGE_FORMAT_RGB => PixelFormat::Rgb,
                sys::KITTY_IMAGE_FORMAT_GRAY_ALPHA => PixelFormat::GrayAlpha,
                sys::KITTY_IMAGE_FORMAT_GRAY => PixelFormat::Gray,
                // PNG is decoded to RGBA before it is stored ("never PNG").
                _ => PixelFormat::Rgba,
            },
            generation,
            pixels,
        })
    }
}

// ===========================================================================
// Selection.
// ===========================================================================

/// The library's gesture state machine and its three reusable events.
struct Gesture {
    gesture: sys::Handle,
    press: sys::Handle,
    drag: sys::Handle,
    release: sys::Handle,
}

// SAFETY: the gesture and its events are reached only through `&mut Mirror`,
// and every call that also takes the terminal holds the mirror's gate; the
// library ties none of them to a thread.
unsafe impl Send for Gesture {}

impl Gesture {
    fn new() -> Result<Self, Refused> {
        let mut g = Self {
            gesture: null_mut(),
            press: null_mut(),
            drag: null_mut(),
            release: null_mut(),
        };
        // SAFETY: NULL allocator; out pointers are ours. A failure part-way
        // frees what was made (every free accepts NULL).
        let made = unsafe {
            check(
                sys::ghostty_selection_gesture_new(null(), &raw mut g.gesture),
                "create a gesture",
            )
            .and_then(|()| {
                check(
                    sys::ghostty_selection_gesture_event_new(
                        null(),
                        &raw mut g.press,
                        sys::SELECTION_GESTURE_EVENT_TYPE_PRESS,
                    ),
                    "create a press",
                )
            })
            .and_then(|()| {
                check(
                    sys::ghostty_selection_gesture_event_new(
                        null(),
                        &raw mut g.drag,
                        sys::SELECTION_GESTURE_EVENT_TYPE_DRAG,
                    ),
                    "create a drag",
                )
            })
            .and_then(|()| {
                check(
                    sys::ghostty_selection_gesture_event_new(
                        null(),
                        &raw mut g.release,
                        sys::SELECTION_GESTURE_EVENT_TYPE_RELEASE,
                    ),
                    "create a release",
                )
            })
        };
        if let Err(e) = made {
            // SAFETY: nothing has touched a terminal yet; NULL terminal is
            // the documented way to free a gesture with no tracked state.
            unsafe { g.free(null_mut()) };
            return Err(e);
        }
        Ok(g)
    }

    /// # Safety
    ///
    /// `terminal` is the one the gesture was last used with, alive and gated
    /// (or NULL when it never was).
    unsafe fn free(self, terminal: sys::Handle) {
        // SAFETY: the caller's promise; each handle freed once.
        unsafe {
            sys::ghostty_selection_gesture_event_free(self.press);
            sys::ghostty_selection_gesture_event_free(self.drag);
            sys::ghostty_selection_gesture_event_free(self.release);
            sys::ghostty_selection_gesture_free(self.gesture, terminal);
        }
    }
}

/// A pointer event for a selection, in the viewport and in surface pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    /// The cell's column.
    pub column: u16,
    /// The cell's viewport row.
    pub row: u16,
    /// Surface pixels.
    pub x_px: f64,
    /// Surface pixels.
    pub y_px: f64,
}

/// How clicks become a double or a triple click — the person's settings, not
/// the library's guess.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Repeat {
    /// The farthest, in pixels, a second click may be from the first.
    pub distance_px: f64,
    /// The longest, in nanoseconds, between them.
    pub interval_ns: u64,
}

impl Mirror {
    fn gesture(&mut self) -> Result<&Gesture, Refused> {
        let g = match self.gesture.take() {
            Some(g) => g,
            None => Gesture::new()?,
        };
        Ok(self.gesture.insert(g))
    }

    /// A button press: one click starts a cell selection (or, with
    /// `command_output`, selects the output of the command under it — OSC
    /// 133), two select a word, three a line. Returns whether something is
    /// selected now.
    ///
    /// # Errors
    ///
    /// The library refused the gesture.
    pub fn select_press(
        &mut self,
        at: Pointer,
        time_ns: u64,
        repeat: Repeat,
        command_output: bool,
    ) -> Result<bool, Refused> {
        let behaviors = sys::SelectionGestureBehaviors {
            single_click: if command_output {
                sys::SELECTION_GESTURE_BEHAVIOR_OUTPUT
            } else {
                sys::SELECTION_GESTURE_BEHAVIOR_CELL
            },
            double_click: sys::SELECTION_GESTURE_BEHAVIOR_WORD,
            triple_click: sys::SELECTION_GESTURE_BEHAVIOR_LINE,
        };
        let position = sys::SurfacePosition {
            x: at.x_px,
            y: at.y_px,
        };
        let (gesture, press) = {
            let g = self.gesture()?;
            (g.gesture, g.press)
        };
        let _gate = self.shared.lock();
        let raw = self.shared.raw();
        // SAFETY: a live terminal under the gate; every option value has its
        // documented type and is copied by the library when set.
        unsafe {
            let reference = viewport_ref(raw, at.column, at.row).ok_or(Refused {
                what: "find the pressed cell",
                code: sys::INVALID_VALUE,
            })?;
            let set = |option: c_int, value: *const c_void| {
                check(
                    sys::ghostty_selection_gesture_event_set(press, option, value),
                    "arm a press",
                )
            };
            set(
                sys::SELECTION_GESTURE_EVENT_OPT_REF,
                (&raw const reference).cast(),
            )?;
            set(
                sys::SELECTION_GESTURE_EVENT_OPT_POSITION,
                (&raw const position).cast(),
            )?;
            set(
                sys::SELECTION_GESTURE_EVENT_OPT_TIME_NS,
                (&raw const time_ns).cast(),
            )?;
            set(
                sys::SELECTION_GESTURE_EVENT_OPT_REPEAT_DISTANCE,
                (&raw const repeat.distance_px).cast(),
            )?;
            set(
                sys::SELECTION_GESTURE_EVENT_OPT_REPEAT_INTERVAL_NS,
                (&raw const repeat.interval_ns).cast(),
            )?;
            set(
                sys::SELECTION_GESTURE_EVENT_OPT_BEHAVIORS,
                (&raw const behaviors).cast(),
            )?;
            apply_gesture(raw, gesture, press, true)
        }
    }

    /// The pointer moved with the button held: the selection follows it,
    /// rectangular with `rectangle` (Ctrl+Alt in the window).
    ///
    /// # Errors
    ///
    /// The library refused the gesture.
    pub fn select_drag(
        &mut self,
        at: Pointer,
        rectangle: bool,
        geometry: &Geometry,
    ) -> Result<bool, Refused> {
        let shape = sys::SelectionGestureGeometry {
            columns: u32::from(self.columns.max(1)),
            cell_width: geometry.cell_width_px.max(1),
            padding_left: geometry.padding_left_px,
            screen_height: geometry.height_px.max(1),
        };
        let position = sys::SurfacePosition {
            x: at.x_px,
            y: at.y_px,
        };
        let (gesture, drag) = {
            let g = self.gesture()?;
            (g.gesture, g.drag)
        };
        let _gate = self.shared.lock();
        let raw = self.shared.raw();
        // SAFETY: as in `select_press`.
        unsafe {
            let Some(reference) = viewport_ref(raw, at.column, at.row) else {
                return Ok(false);
            };
            let set = |option: c_int, value: *const c_void| {
                check(
                    sys::ghostty_selection_gesture_event_set(drag, option, value),
                    "arm a drag",
                )
            };
            set(
                sys::SELECTION_GESTURE_EVENT_OPT_REF,
                (&raw const reference).cast(),
            )?;
            set(
                sys::SELECTION_GESTURE_EVENT_OPT_POSITION,
                (&raw const position).cast(),
            )?;
            set(
                sys::SELECTION_GESTURE_EVENT_OPT_RECTANGLE,
                (&raw const rectangle).cast(),
            )?;
            set(
                sys::SELECTION_GESTURE_EVENT_OPT_GEOMETRY,
                (&raw const shape).cast(),
            )?;
            apply_gesture(raw, gesture, drag, false)
        }
    }

    /// The button came up, over a cell or (`None`) outside the grid.
    ///
    /// # Errors
    ///
    /// The library refused the gesture.
    pub fn select_release(&mut self, at: Option<(u16, u16)>) -> Result<(), Refused> {
        let (gesture, release) = {
            let g = self.gesture()?;
            (g.gesture, g.release)
        };
        let _gate = self.shared.lock();
        let raw = self.shared.raw();
        // SAFETY: as in `select_press`; a NULL value clears the option.
        unsafe {
            let reference = at.and_then(|(c, r)| viewport_ref(raw, c, r));
            let value = reference
                .as_ref()
                .map_or(null(), |r| std::ptr::from_ref(r).cast());
            check(
                sys::ghostty_selection_gesture_event_set(
                    release,
                    sys::SELECTION_GESTURE_EVENT_OPT_REF,
                    value,
                ),
                "arm a release",
            )?;
            // A release produces no selection (NO_VALUE); it only closes the
            // gesture's click.
            let r = sys::ghostty_selection_gesture_event(gesture, raw, release, null_mut());
            if r != sys::SUCCESS && r != sys::NO_VALUE {
                return Err(Refused {
                    what: "release a gesture",
                    code: r,
                });
            }
        }
        Ok(())
    }

    /// Moves the end of the selection to a cell, keeping its start (Shift+click).
    /// With nothing selected, selects that one cell.
    ///
    /// # Errors
    ///
    /// The library refused the selection.
    pub fn select_extend(&mut self, column: u16, row: u16) -> Result<bool, Refused> {
        let _gate = self.shared.lock();
        let raw = self.shared.raw();
        // SAFETY: a live terminal under the gate; the refs are used before
        // any mutation.
        unsafe {
            let Some(end) = viewport_ref(raw, column, row) else {
                return Ok(false);
            };
            let mut selection = get(raw, sys::TERMINAL_DATA_SELECTION, sys::Selection::sized())
                .unwrap_or(sys::Selection {
                    start: end,
                    ..sys::Selection::sized()
                });
            selection.end = end;
            check(
                sys::ghostty_terminal_set(
                    raw,
                    sys::TERMINAL_OPT_SELECTION,
                    (&raw const selection).cast(),
                ),
                "extend the selection",
            )?;
        }
        Ok(true)
    }

    /// Selects everything (Ctrl+Shift+A).
    ///
    /// # Errors
    ///
    /// The library refused the selection.
    pub fn select_all(&mut self) -> Result<bool, Refused> {
        let _gate = self.shared.lock();
        let raw = self.shared.raw();
        let mut selection = sys::Selection::sized();
        // SAFETY: a live terminal under the gate.
        unsafe {
            match sys::ghostty_terminal_select_all(raw, &raw mut selection) {
                sys::NO_VALUE => return Ok(false),
                r => check(r, "select everything")?,
            }
            check(
                sys::ghostty_terminal_set(
                    raw,
                    sys::TERMINAL_OPT_SELECTION,
                    (&raw const selection).cast(),
                ),
                "install the selection",
            )?;
        }
        Ok(true)
    }

    /// Clears the selection.
    pub fn select_clear(&mut self) {
        let _gate = self.shared.lock();
        // SAFETY: NULL clears the selection, as documented.
        unsafe {
            sys::ghostty_terminal_set(self.shared.raw(), sys::TERMINAL_OPT_SELECTION, null())
        };
    }

    /// The selected text, the way Ghostty copies it: plain, soft wraps
    /// joined, trailing space trimmed. `None` with nothing selected.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let _gate = self.shared.lock();
        let options = sys::TerminalSelectionFormatOptions {
            size: size_of::<sys::TerminalSelectionFormatOptions>(),
            emit: sys::FORMATTER_FORMAT_PLAIN,
            unwrap: true,
            trim: true,
            selection: null(),
        };
        let (mut ptr, mut len) = (null_mut::<u8>(), 0usize);
        // SAFETY: a live terminal under the gate; the result is freed by
        // `take_allocated`.
        let r = unsafe {
            sys::ghostty_terminal_selection_format_alloc(
                self.shared.raw(),
                null(),
                options,
                &raw mut ptr,
                &raw mut len,
            )
        };
        if r != sys::SUCCESS {
            return None;
        }
        // SAFETY: an `_alloc` result with the default allocator.
        let bytes = unsafe { take_allocated(ptr, len) };
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// Applies an armed gesture event and installs what it selected.
///
/// # Safety
///
/// `terminal` alive and gated; `gesture` and `event` live.
unsafe fn apply_gesture(
    terminal: sys::Handle,
    gesture: sys::Handle,
    event: sys::Handle,
    clear_on_none: bool,
) -> Result<bool, Refused> {
    let mut selection = sys::Selection::sized();
    // SAFETY: the caller's promise.
    let r = unsafe {
        sys::ghostty_selection_gesture_event(gesture, terminal, event, &raw mut selection)
    };
    match r {
        sys::SUCCESS => {
            // SAFETY: the snapshot just returned, installed before any mutation.
            check(
                unsafe {
                    sys::ghostty_terminal_set(
                        terminal,
                        sys::TERMINAL_OPT_SELECTION,
                        (&raw const selection).cast(),
                    )
                },
                "install the selection",
            )?;
            Ok(true)
        }
        sys::NO_VALUE => {
            if clear_on_none {
                // A press that selects nothing still clears the old selection.
                // SAFETY: NULL clears it.
                unsafe { sys::ghostty_terminal_set(terminal, sys::TERMINAL_OPT_SELECTION, null()) };
            }
            Ok(false)
        }
        code => Err(Refused {
            what: "apply a gesture",
            code,
        }),
    }
}

// ===========================================================================
// Decoding a snapshot into a mirror.
// ===========================================================================

/// What one [`Decoder::next`] restored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restored {
    /// A page of history, prepended: this many rows.
    Page {
        /// Rows it added (0 when it could no longer be applied).
        rows: usize,
    },
    /// The pixels of a kitty image arrived.
    Image {
        /// The image.
        id: u32,
        /// Bytes attached (0 when they were dropped).
        bytes: usize,
    },
}

/// The source a decoder reads, behind the library's reader callback.
struct Source {
    read: Box<dyn Read + Send>,
}

unsafe extern "C" fn read_source(
    userdata: *mut c_void,
    buffer: *mut u8,
    capacity: usize,
    out_read: *mut usize,
) -> bool {
    guarded(false, || {
        // SAFETY: `userdata` is the decoder's `Source`, and the library calls
        // only from inside a decoder call made through `&mut Decoder`;
        // `buffer` is writable for `capacity` bytes.
        let source = unsafe { &mut *userdata.cast::<Source>() };
        // ZEROED before it becomes a slice: the library's buffer may be
        // uninitialised, and a `&mut [u8]` over uninitialised bytes handed to
        // an arbitrary `Read` (which may read before it writes) is undefined
        // behaviour.
        // SAFETY: writable for `capacity` bytes, as the reader contract says.
        unsafe { buffer.write_bytes(0, capacity) };
        let buf = unsafe { std::slice::from_raw_parts_mut(buffer, capacity) };
        loop {
            match source.read.read(buf) {
                Ok(n) => {
                    // Clamped: `Read` is a safe trait, and one that claims more
                    // than the buffer holds would have the library read past it
                    // ("no greater than capacity", io.h).
                    // SAFETY: the library's out pointer.
                    unsafe { out_read.write(n.min(capacity)) };
                    return true;
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => return false,
            }
        }
    })
}

/// Decodes a snapshot into a [`Mirror`]: the renderable part first
/// ([`Decoder::ready`]), then history page by page ([`Decoder::next`]), so a
/// window draws at once and prepends the past as it arrives.
pub struct Decoder {
    decoder: NonNull<c_void>,
    /// Boxed so the pointer the library holds stays put.
    source: NonNull<Source>,
    /// The mirror's terminal, from `ready` until FINISH: the decoder writes
    /// history into it, so it holds it alive and takes its gate.
    target: Option<Arc<Shared>>,
    finished: bool,
}

// SAFETY: the decoder and its source are reached only through `&mut self`;
// the terminal it fills is reached only under its gate.
unsafe impl Send for Decoder {}

impl Decoder {
    /// A decoder over a source that yields the snapshot's bytes. A read that
    /// waits for more bytes waits inside `ready`/`next`: give it a source
    /// whose bytes are there, or call it where waiting is fine.
    ///
    /// # Errors
    ///
    /// The library refused to create it.
    pub fn new(read: impl Read + Send + 'static) -> Result<Self, Refused> {
        let source = NonNull::from(Box::leak(Box::new(Source {
            read: Box::new(read),
        })));
        let reader = sys::Reader {
            read: Some(read_source),
            userdata: source.as_ptr().cast(),
        };
        let mut raw: sys::Handle = null_mut();
        // SAFETY: the reader's userdata lives as long as the decoder (freed
        // after it in `Drop`).
        let r = unsafe { sys::ghostty_snapshot_decoder_new(null(), &raw mut raw, reader) };
        let Some(decoder) = NonNull::new(raw).filter(|_| r == sys::SUCCESS) else {
            // SAFETY: the box made above, never handed out.
            drop(unsafe { Box::from_raw(source.as_ptr()) });
            return Err(Refused {
                what: "create a snapshot decoder",
                code: r,
            });
        };
        let d = Self {
            decoder,
            source,
            target: None,
            finished: false,
        };
        // A continuation as large as the holder may have kept, or the
        // snapshot of a terminal caught inside a kitty image would be refused.
        let limit = CONTINUATION_LIMIT_BYTES;
        // History compressed as it arrives: a mirror holds the same history as
        // its holder, and doubling it uncompressed is the memory this pays.
        let compress = true;
        // SAFETY: each value has its option's documented type.
        unsafe {
            check(
                sys::ghostty_snapshot_decoder_set(
                    d.decoder.as_ptr(),
                    sys::SNAPSHOT_DECODER_OPT_MAX_CONTINUATION_BYTES,
                    (&raw const limit).cast(),
                ),
                "take the continuation limit",
            )?;
            check(
                sys::ghostty_snapshot_decoder_set(
                    d.decoder.as_ptr(),
                    sys::SNAPSHOT_DECODER_OPT_COMPRESS_HISTORY,
                    (&raw const compress).cast(),
                ),
                "compress history",
            )?;
        }
        Ok(d)
    }

    /// A decoder over a whole snapshot in memory.
    ///
    /// # Errors
    ///
    /// The library refused to create it.
    pub fn of_bytes(snapshot: Vec<u8>) -> Result<Self, Refused> {
        Self::new(std::io::Cursor::new(snapshot))
    }

    /// Decodes the renderable part, through READY: a mirror that can be drawn
    /// and fed live bytes now. Once only.
    ///
    /// # Errors
    ///
    /// The snapshot is malformed or truncated, or this was called twice.
    pub fn ready(&mut self) -> Result<Mirror, Refused> {
        let mut raw: sys::Handle = null_mut();
        // SAFETY: a live decoder; the source is read during the call.
        check(
            unsafe { sys::ghostty_snapshot_decoder_ready(self.decoder.as_ptr(), &raw mut raw) },
            "decode a snapshot",
        )?;
        let terminal = NonNull::new(raw).ok_or(Refused {
            what: "decode a snapshot",
            code: sys::INVALID_VALUE,
        })?;
        let sink = NonNull::from(Box::leak(Box::new(MirrorSink {
            held: false,
            capture: None,
        })));
        let shared = Arc::new(Shared {
            terminal,
            sink,
            gate: Mutex::new(()),
        });
        // Held BEFORE anything can fail: the decoder borrows this terminal
        // until FINISH or its own end, and must never outlive it.
        self.target = Some(Arc::clone(&shared));
        Mirror::from_decoded(shared)
    }

    /// Restores the next history page or image's pixels into the mirror
    /// `ready` returned; `None` once the snapshot is complete. The
    /// [`Iterator`] form, `next`, says the same with the error inside.
    ///
    /// # Errors
    ///
    /// The snapshot is malformed, or `ready` was not called.
    pub fn restore_next(&mut self) -> Result<Option<Restored>, Refused> {
        if self.finished {
            return Ok(None);
        }
        let Some(target) = &self.target else {
            return Err(Refused {
                what: "restore history before the screen",
                code: sys::INVALID_VALUE,
            });
        };
        let gate = target.lock();
        let decoder = self.decoder.as_ptr();
        // SAFETY: a live decoder; it writes into the terminal it returned,
        // which `target` keeps alive and the gate keeps ours.
        let r = unsafe { sys::ghostty_snapshot_decoder_next(decoder) };
        if r == sys::NO_VALUE {
            drop(gate);
            self.finished = true;
            self.target = None;
            return Ok(None);
        }
        check(r, "restore history")?;
        // SAFETY: the progress fields' documented types.
        let restored = unsafe {
            let mut kind = sys::SNAPSHOT_DECODER_PROGRESS_KIND_PAGE;
            sys::ghostty_snapshot_decoder_get(
                decoder,
                sys::SNAPSHOT_DECODER_DATA_PROGRESS_KIND,
                (&raw mut kind).cast(),
            );
            if kind == sys::SNAPSHOT_DECODER_PROGRESS_KIND_IMAGE {
                let (mut id, mut bytes) = (0u32, 0usize);
                sys::ghostty_snapshot_decoder_get(
                    decoder,
                    sys::SNAPSHOT_DECODER_DATA_PROGRESS_IMAGE_ID,
                    (&raw mut id).cast(),
                );
                sys::ghostty_snapshot_decoder_get(
                    decoder,
                    sys::SNAPSHOT_DECODER_DATA_PROGRESS_IMAGE_BYTES,
                    (&raw mut bytes).cast(),
                );
                Restored::Image { id, bytes }
            } else {
                let mut rows = 0usize;
                sys::ghostty_snapshot_decoder_get(
                    decoder,
                    sys::SNAPSHOT_DECODER_DATA_PROGRESS_ROWS,
                    (&raw mut rows).cast(),
                );
                Restored::Page { rows }
            }
        };
        Ok(Some(restored))
    }

    /// Decodes a whole snapshot: READY and every page after it.
    ///
    /// # Errors
    ///
    /// The snapshot is malformed or truncated.
    pub fn decode(snapshot: Vec<u8>) -> Result<Mirror, Refused> {
        let mut d = Self::of_bytes(snapshot)?;
        let mirror = d.ready()?;
        while d.restore_next()?.is_some() {}
        Ok(mirror)
    }
}

/// History page by history page, until FINISH: `for step in &mut decoder`.
impl Iterator for Decoder {
    type Item = Result<Restored, Refused>;

    fn next(&mut self) -> Option<Self::Item> {
        self.restore_next().transpose()
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let gate = self.target.as_ref().map(|t| t.lock());
        // SAFETY: freed once, under the target's gate if it has one (the
        // decoder may release state it holds in that terminal), and before
        // the source its reader reads.
        unsafe { sys::ghostty_snapshot_decoder_free(self.decoder.as_ptr()) };
        drop(gate);
        // SAFETY: the box made in `new`; the decoder that read it is gone.
        drop(unsafe { Box::from_raw(self.source.as_ptr()) });
    }
}

// ===========================================================================
// Search.
// ===========================================================================

/// How far a search has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchStatus {
    /// [`Search::tick`] can make progress without the terminal.
    Running,
    /// [`Search::feed`] must read the terminal before it can go on.
    NeedsFeed,
    /// Caught up with the terminal as of the last feed.
    Complete,
}

/// A match on screen, from its first cell to its last, both inclusive, in
/// viewport coordinates. A match can span rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Highlight {
    /// `(column, row)` of the first cell.
    pub start: (u16, u16),
    /// `(column, row)` of the last cell.
    pub end: (u16, u16),
}

/// A search over a mirror's screen and scrollback, both screens.
///
/// It borrows the terminal through the mirror's shared handle: [`Search::tick`]
/// never touches it and may run anywhere; everything else takes the gate.
pub struct Search {
    search: NonNull<c_void>,
    shared: Arc<Shared>,
}

// SAFETY: one owner at a time (`&mut self` for every call that mutates);
// terminal access goes through the gate.
unsafe impl Send for Search {}

impl Search {
    /// A search over `mirror`, idle until a needle is set.
    ///
    /// # Errors
    ///
    /// The library refused to create it.
    pub fn new(mirror: &Mirror) -> Result<Self, Refused> {
        let _gate = mirror.shared.lock();
        let mut raw: sys::Handle = null_mut();
        // SAFETY: a live terminal under the gate.
        let r = unsafe { sys::ghostty_search_new(null(), &raw mut raw, mirror.shared.raw()) };
        let search = NonNull::new(raw)
            .filter(|_| r == sys::SUCCESS)
            .ok_or(Refused {
                what: "create a search",
                code: r,
            })?;
        Ok(Self {
            search,
            shared: Arc::clone(&mirror.shared),
        })
    }

    /// Sets what to look for, which restarts the search (the same needle again
    /// keeps the results). Empty returns it to idle. ASCII letters match
    /// regardless of case.
    ///
    /// # Errors
    ///
    /// The library refused it.
    pub fn set_needle(&mut self, needle: &str) -> Result<(), Refused> {
        let _gate = self.shared.lock();
        let value = borrowed(needle.as_bytes());
        // SAFETY: the needle is copied by the library; gated.
        check(
            unsafe {
                sys::ghostty_search_set(
                    self.search.as_ptr(),
                    sys::SEARCH_OPT_NEEDLE,
                    (&raw const value).cast(),
                )
            },
            "set the needle",
        )
    }

    /// Reads the terminal to catch up — the only way a search learns the
    /// terminal changed. Bounded work.
    ///
    /// # Errors
    ///
    /// The terminal is gone.
    pub fn feed(&mut self) -> Result<(), Refused> {
        let _gate = self.shared.lock();
        // SAFETY: gated.
        check(
            unsafe { sys::ghostty_search_feed(self.search.as_ptr()) },
            "feed the search",
        )
    }

    /// A bounded step of work on what was fed. Never touches the terminal.
    ///
    /// # Errors
    ///
    /// The terminal is gone.
    pub fn tick(&mut self) -> Result<SearchStatus, Refused> {
        let mut status = sys::SEARCH_STATUS_COMPLETE;
        // SAFETY: search-owned memory only, as documented.
        check(
            unsafe { sys::ghostty_search_tick(self.search.as_ptr(), &raw mut status) },
            "advance the search",
        )?;
        Ok(match status {
            sys::SEARCH_STATUS_RUNNING => SearchStatus::Running,
            sys::SEARCH_STATUS_FEED_REQUIRED => SearchStatus::NeedsFeed,
            _ => SearchStatus::Complete,
        })
    }

    /// Feeds and ticks until caught up. Blocking: for tests and one-shot use.
    ///
    /// # Errors
    ///
    /// The terminal is gone.
    pub fn run(&mut self) -> Result<(), Refused> {
        loop {
            self.feed()?;
            loop {
                match self.tick()? {
                    SearchStatus::Running => {}
                    SearchStatus::NeedsFeed => break,
                    SearchStatus::Complete => return Ok(()),
                }
            }
        }
    }

    /// Matches found so far on the active screen.
    #[must_use]
    pub fn total(&self) -> usize {
        let mut n = 0usize;
        // SAFETY: search-owned memory; `size_t`.
        unsafe {
            sys::ghostty_search_get(
                self.search.as_ptr(),
                sys::SEARCH_DATA_TOTAL_MATCHES,
                (&raw mut n).cast(),
            )
        };
        n
    }

    /// The selected match's index, 0 the newest — a find bar shows `index + 1`
    /// of [`Search::total`].
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        let mut n = 0usize;
        // SAFETY: search-owned memory; `size_t`.
        (unsafe {
            sys::ghostty_search_get(
                self.search.as_ptr(),
                sys::SEARCH_DATA_SELECTED_INDEX,
                (&raw mut n).cast(),
            )
        } == sys::SUCCESS)
            .then_some(n)
    }

    /// Selects the next match toward OLDER content (up from the prompt),
    /// wrapping, and scrolls to it if it is off screen. `false` when there
    /// are no matches.
    ///
    /// # Errors
    ///
    /// The terminal is gone.
    pub fn select_older(&mut self) -> Result<bool, Refused> {
        self.select(sys::SEARCH_OPT_SELECT_NEXT)
    }

    /// Selects the next match toward NEWER content.
    ///
    /// # Errors
    ///
    /// The terminal is gone.
    pub fn select_newer(&mut self) -> Result<bool, Refused> {
        self.select(sys::SEARCH_OPT_SELECT_PREV)
    }

    fn select(&mut self, option: c_int) -> Result<bool, Refused> {
        let _gate = self.shared.lock();
        // SAFETY: gated; the value must be NULL.
        match unsafe { sys::ghostty_search_set(self.search.as_ptr(), option, null()) } {
            sys::NO_VALUE => Ok(false),
            r => check(r, "select a match").map(|()| true),
        }
    }

    /// The matches on the viewport, for drawing — caught up with the terminal
    /// first, under the same hold of the gate.
    ///
    /// The feed is not a courtesy. A match is a grid ref, valid only until
    /// the terminal next changes (search.h, "Matches are selections"), and a
    /// mirror fed or resized since the last [`Search::feed`] would have these
    /// turned into positions on pages that were rebuilt or freed. Feeding and
    /// converting without letting go of the gate is what keeps them current.
    #[must_use]
    pub fn on_screen(&mut self) -> Vec<Highlight> {
        self.drawn().0
    }

    /// The matches on the viewport and, among them, the one walked to — the
    /// one a find bar's "k of n" names, drawn apart from the rest — both from
    /// ONE feed under one hold of the gate, so the frame that draws them
    /// never pairs a list with a selected match of another moment. The
    /// selected one is `None` before any walk, and when it is off screen.
    ///
    /// The selected match is read again after the feed, never kept: the
    /// library keeps it accurate across the terminal's changes, and "re-read
    /// `GHOSTTY_SEARCH_DATA_SELECTED_MATCH` after each feed" is search.h's
    /// own way to follow it.
    #[must_use]
    pub fn drawn(&mut self) -> (Vec<Highlight>, Option<Highlight>) {
        let _gate = self.shared.lock();
        let search = self.search.as_ptr();
        // SAFETY: gated, and the only call on this search in progress
        // (`&mut self`).
        if unsafe { sys::ghostty_search_feed(search) } != sys::SUCCESS {
            return (Vec::new(), None);
        }
        let raw = self.shared.raw();
        let highlight = |m: &sys::Selection| {
            // SAFETY: the refs come from the feed above, and the gate has
            // kept the terminal unchanged since.
            let at = |r: &sys::GridRef| unsafe { viewport_point(raw, r) };
            Some(Highlight {
                start: at(&m.start)?,
                end: at(&m.end)?,
            })
        };
        let mut selected = sys::Selection::sized();
        // SAFETY: search-owned memory, a sized selection as documented; read
        // under the same gate as the feed.
        let selected = (unsafe {
            sys::ghostty_search_get(
                search,
                sys::SEARCH_DATA_SELECTED_MATCH,
                (&raw mut selected).cast(),
            )
        } == sys::SUCCESS)
            .then(|| highlight(&selected))
            .flatten();
        (Self::viewport_matches(search, highlight), selected)
    }

    /// The matches on the viewport, under the caller's gate and after its
    /// feed, each made a [`Highlight`] by `highlight`.
    fn viewport_matches(
        search: sys::Handle,
        highlight: impl Fn(&sys::Selection) -> Option<Highlight>,
    ) -> Vec<Highlight> {
        let mut buffer = sys::SelectionBuffer {
            ptr: null_mut(),
            cap: 0,
            len: 0,
        };
        // SAFETY: a capacity query, then a buffer of the capacity asked for.
        unsafe {
            sys::ghostty_search_get(
                search,
                sys::SEARCH_DATA_VIEWPORT_MATCHES,
                (&raw mut buffer).cast(),
            );
        }
        let mut matches = vec![sys::Selection::sized(); buffer.len];
        if matches.is_empty() {
            return Vec::new();
        }
        buffer = sys::SelectionBuffer {
            ptr: matches.as_mut_ptr(),
            cap: matches.len(),
            len: 0,
        };
        // SAFETY: as above.
        if unsafe {
            sys::ghostty_search_get(
                search,
                sys::SEARCH_DATA_VIEWPORT_MATCHES,
                (&raw mut buffer).cast(),
            )
        } != sys::SUCCESS
        {
            return Vec::new();
        }
        matches.truncate(buffer.len);
        matches.iter().filter_map(highlight).collect()
    }
}

impl Drop for Search {
    fn drop(&mut self) {
        let _gate = self.shared.lock();
        // SAFETY: freed once; it releases tracked state in the terminal, so
        // under the gate.
        unsafe { sys::ghostty_search_free(self.search.as_ptr()) };
    }
}

/// A grid ref's viewport position, or `None` when it is off screen.
///
/// # Safety
///
/// `terminal` alive and gated; `reference` taken since its last mutation.
unsafe fn viewport_point(terminal: sys::Handle, reference: &sys::GridRef) -> Option<(u16, u16)> {
    let mut out = sys::PointCoordinate { x: 0, y: 0 };
    // SAFETY: the caller's promise.
    (unsafe {
        sys::ghostty_terminal_point_from_grid_ref(
            terminal,
            reference,
            sys::POINT_TAG_VIEWPORT,
            &raw mut out,
        )
    } == sys::SUCCESS)
        .then(|| (out.x, u16::try_from(out.y).unwrap_or(u16::MAX)))
}

// ===========================================================================
// Reading a terminal for drawing.
// ===========================================================================

/// What a render state is updated from: a [`Vt`] or a [`Mirror`].
pub trait Screen: sealed::Sealed {
    /// Runs `f` with the terminal, holding whatever keeps it ours meanwhile.
    #[doc(hidden)]
    fn with_terminal<R>(&self, f: impl FnOnce(TerminalRef) -> R) -> R;
    /// Whether a program holds the screen (synchronized output).
    fn held(&self) -> bool;
}

/// A terminal handle lent to a [`RenderState`] for one call.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct TerminalRef(sys::Handle);

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Vt {}
    impl Sealed for super::Mirror {}
}

impl Screen for Vt {
    fn with_terminal<R>(&self, f: impl FnOnce(TerminalRef) -> R) -> R {
        f(TerminalRef(self.raw()))
    }

    fn held(&self) -> bool {
        Self::held(self)
    }
}

impl Screen for Mirror {
    fn with_terminal<R>(&self, f: impl FnOnce(TerminalRef) -> R) -> R {
        let _gate = self.shared.lock();
        f(TerminalRef(self.shared.raw()))
    }

    fn held(&self) -> bool {
        Self::held(self)
    }
}

/// The library's render state and the iterators it fills, made once and
/// reused, with the scratch every frame reads into.
struct RenderInner {
    state: sys::Handle,
    rows: sys::Handle,
    cells: sys::Handle,
    images: sys::Handle,
    /// The current row's cells and text, refilled per row.
    row_cells: Vec<Cell>,
    row_text: String,
    /// Where one cell's UTF-8 lands before it joins `row_text`.
    grapheme: Vec<u8>,
}

/// The render state behind its own lock, so the mirror's hold callback can
/// capture a frame into it while a window is not reading it.
struct RenderCell {
    inner: Mutex<RenderInner>,
}

impl RenderCell {
    fn lock(&self) -> MutexGuard<'_, RenderInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

// SAFETY: the render state and its iterators are reached only under the
// `Mutex`; the library ties none of them to a thread.
unsafe impl Send for RenderInner {}

impl Drop for RenderInner {
    fn drop(&mut self) {
        // SAFETY: each freed once; the free functions accept NULL.
        unsafe {
            sys::ghostty_render_state_image_iterator_free(self.images);
            sys::ghostty_render_state_row_cells_free(self.cells);
            sys::ghostty_render_state_row_iterator_free(self.rows);
            sys::ghostty_render_state_free(self.state);
        }
    }
}

/// What a frame needs from a terminal, copied out so drawing never holds the
/// terminal.
///
/// The update has two phases, as the library offers it: [`RenderState::begin`]
/// reads the terminal and is the only part that needs it (so the lock that
/// guards the terminal is held for that call only), and [`RenderState::end`]
/// finishes in the render state's own memory, after the lock is released.
pub struct RenderState {
    cell: Arc<RenderCell>,
}

impl RenderState {
    /// A render state, empty until the first update.
    ///
    /// # Errors
    ///
    /// The library could not allocate it.
    pub fn new() -> Result<Self, Refused> {
        let mut inner = RenderInner {
            state: null_mut(),
            rows: null_mut(),
            cells: null_mut(),
            images: null_mut(),
            row_cells: Vec::new(),
            row_text: String::new(),
            grapheme: vec![0; 64],
        };
        // SAFETY: NULL allocator; out pointers are ours. On failure `Drop`
        // frees what was made.
        unsafe {
            check(
                sys::ghostty_render_state_new(null(), &raw mut inner.state),
                "create a render state",
            )?;
            check(
                sys::ghostty_render_state_row_iterator_new(null(), &raw mut inner.rows),
                "create a row iterator",
            )?;
            check(
                sys::ghostty_render_state_row_cells_new(null(), &raw mut inner.cells),
                "create row cells",
            )?;
            check(
                sys::ghostty_render_state_image_iterator_new(null(), &raw mut inner.images),
                "create an image iterator",
            )?;
        }
        Ok(Self {
            cell: Arc::new(RenderCell {
                inner: Mutex::new(inner),
            }),
        })
    }

    /// The first phase: reads `screen` (under its lock, for this call only).
    ///
    /// While a program holds the screen it does NOTHING: the frame on show is
    /// the one captured when the hold began ([`Mirror::capture_holds`]), or
    /// the last one before it.
    ///
    /// # Errors
    ///
    /// The library could not update it.
    pub fn begin(&self, screen: &impl Screen) -> Result<(), Refused> {
        if screen.held() {
            return Ok(());
        }
        screen.with_terminal(|terminal| {
            let inner = self.cell.lock();
            // SAFETY: the terminal is the screen's, locked for this call; the
            // render state is ours, locked.
            check(
                unsafe { sys::ghostty_render_state_begin_update(inner.state, terminal.0) },
                "update the render state",
            )
        })
    }

    /// The second phase: finishes the update in the render state's own memory.
    /// A no-op without a `begin`.
    ///
    /// # Errors
    ///
    /// The library could not finish it.
    pub fn end(&self) -> Result<(), Refused> {
        let inner = self.cell.lock();
        // SAFETY: the render state is ours, locked; it touches no terminal.
        check(
            unsafe { sys::ghostty_render_state_end_update(inner.state) },
            "finish the render state",
        )
    }

    /// Both phases at once, for a caller with no lock to release in between.
    ///
    /// # Errors
    ///
    /// As [`RenderState::begin`] and [`RenderState::end`].
    pub fn update(&self, screen: &impl Screen) -> Result<(), Refused> {
        self.begin(screen)?;
        self.end()
    }

    /// The captured frame, for reading. Holds the render state's lock while
    /// it lives, so a hold beginning meanwhile waits for the frame to be read.
    ///
    /// Do not call the mirror while a frame is alive: feeding takes the
    /// mirror's lock and then this one, so taking them the other way round
    /// on another thread is a deadlock. Read the frame, drop it, then ask.
    #[must_use]
    pub fn frame(&self) -> Frame<'_> {
        Frame {
            inner: self.cell.lock(),
        }
    }
}

/// How much of a frame changed since it was last cleaned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dirty {
    /// Nothing: drawing can be skipped.
    Clean,
    /// Some rows; their `dirty` says which.
    Rows,
    /// Everything.
    Full,
}

/// The shape a cursor is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    /// A vertical bar (DECSCUSR 5, 6).
    Bar,
    /// A block (DECSCUSR 1, 2).
    Block,
    /// An underline (DECSCUSR 3, 4).
    Underline,
    /// A hollow block (an unfocused window).
    Hollow,
}

/// The cursor, as a frame draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct FrameCursor {
    /// `(column, row)` in the viewport, or `None` when it is not on screen.
    pub at: Option<(u16, u16)>,
    /// On the right half of a wide character.
    pub wide_tail: bool,
    /// The program shows it.
    pub visible: bool,
    /// It blinks.
    pub blinking: bool,
    /// It sits at a password prompt.
    pub password: bool,
    /// Its shape.
    pub shape: CursorShape,
}

/// A frame's colours: the defaults and the palette as they are now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameColors {
    /// The background.
    pub background: Rgb,
    /// The foreground.
    pub foreground: Rgb,
    /// The cursor, when a colour was set for it.
    pub cursor: Option<Rgb>,
    /// The 256-colour palette.
    pub palette: Box<[Rgb; 256]>,
}

/// How a cell is underlined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Underline {
    /// Not at all.
    #[default]
    None,
    /// One line.
    Single,
    /// Two lines.
    Double,
    /// A wave.
    Curly,
    /// Dots.
    Dotted,
    /// Dashes.
    Dashed,
}

/// Every attribute Ghostty keeps on a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct CellStyle {
    /// SGR 1.
    pub bold: bool,
    /// SGR 3.
    pub italic: bool,
    /// SGR 2.
    pub faint: bool,
    /// SGR 5.
    pub blink: bool,
    /// SGR 7: the renderer swaps foreground and background.
    pub inverse: bool,
    /// SGR 8.
    pub invisible: bool,
    /// SGR 9.
    pub strikethrough: bool,
    /// SGR 53.
    pub overline: bool,
    /// SGR 4 and its kinds.
    pub underline: Underline,
}

/// How a cell takes columns, as the renderer needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellWidth {
    /// One column.
    #[default]
    Narrow,
    /// The first of two.
    Wide,
    /// The second half of a wide character: draw nothing.
    SpacerTail,
    /// The end of a line a wide character did not fit on: draw nothing.
    SpacerHead,
}

/// One cell of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cell {
    /// Where its text is in the row's text ([`RowView::text`]).
    text: (u32, u32),
    /// Its attributes.
    pub style: CellStyle,
    /// The foreground, resolved through the palette; `None` for the default.
    pub fg: Option<Rgb>,
    /// The background, resolved; `None` for the default.
    pub bg: Option<Rgb>,
    /// The underline's own colour (SGR 58); `None` to follow the foreground.
    pub underline_colour: Option<Rgb>,
    /// How it takes columns.
    pub width: CellWidth,
    /// Inside the selection.
    pub selected: bool,
}

/// One row of a frame, lent for the duration of a visit.
pub struct RowView<'a> {
    /// The row's place in the viewport, from 0.
    pub y: i32,
    /// It changed since the frame was last cleaned.
    pub dirty: bool,
    /// The selected columns, first and last inclusive.
    pub selection: Option<(u16, u16)>,
    /// Its cells, one per column.
    pub cells: &'a [Cell],
    text: &'a str,
}

impl RowView<'_> {
    /// A cell's grapheme cluster; empty for an empty cell.
    #[must_use]
    pub fn text(&self, cell: &Cell) -> &str {
        let (start, len) = (cell.text.0 as usize, cell.text.1 as usize);
        self.text.get(start..start + len).unwrap_or_default()
    }
}

/// Which layer an image is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageLayer {
    /// Below the cells' backgrounds.
    BelowBackground,
    /// Above the backgrounds, below the text.
    BelowText,
    /// Above the text.
    AboveText,
}

/// One rectangle of one kitty image, placed on the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// The image ([`Mirror::image`]).
    pub image: u32,
    /// The image's pixel generation, for a texture cache's key.
    pub generation: u64,
    /// The layer.
    pub layer: ImageLayer,
    /// Order within the layer, lowest first.
    pub z: i32,
    /// The top-left cell, possibly off the left or the top.
    pub column: i32,
    /// The top-left cell's row.
    pub row: i32,
    /// Cells covered.
    pub columns: u32,
    /// Rows covered.
    pub rows: u32,
    /// From the top-left cell's corner, in pixels.
    pub offset_px: (u32, u32),
    /// The size to draw at, in pixels.
    pub size_px: (u32, u32),
    /// The part of the image to draw: x, y, width, height in image pixels.
    pub source: (u32, u32, u32, u32),
}

/// A frame, read under the render state's lock.
pub struct Frame<'a> {
    inner: MutexGuard<'a, RenderInner>,
}

impl Frame<'_> {
    fn state(&self) -> sys::Handle {
        self.inner.state
    }

    /// `(columns, rows)` of the viewport.
    #[must_use]
    pub fn size(&self) -> (u16, u16) {
        let (mut c, mut r) = (0u16, 0u16);
        // SAFETY: `uint16_t` outputs; the render state is ours, locked.
        unsafe {
            sys::ghostty_render_state_get(
                self.state(),
                sys::RENDER_STATE_DATA_COLS,
                (&raw mut c).cast(),
            );
            sys::ghostty_render_state_get(
                self.state(),
                sys::RENDER_STATE_DATA_ROWS,
                (&raw mut r).cast(),
            );
        }
        (c, r)
    }

    /// How much changed.
    #[must_use]
    pub fn dirty(&self) -> Dirty {
        let mut d = sys::RENDER_STATE_DIRTY_FULL;
        // SAFETY: a `GhosttyRenderStateDirty` output.
        unsafe {
            sys::ghostty_render_state_get(
                self.state(),
                sys::RENDER_STATE_DATA_DIRTY,
                (&raw mut d).cast(),
            )
        };
        match d {
            sys::RENDER_STATE_DIRTY_FALSE => Dirty::Clean,
            sys::RENDER_STATE_DIRTY_PARTIAL => Dirty::Rows,
            _ => Dirty::Full,
        }
    }

    /// Marks the frame drawn: both the frame's and every row's dirtiness.
    pub fn clean(&mut self) {
        // SAFETY: ours, locked.
        unsafe { sys::ghostty_render_state_clean(self.state()) };
    }

    /// The cursor.
    #[must_use]
    pub fn cursor(&self) -> FrameCursor {
        let mut c = sys::RenderStateCursor {
            size: size_of::<sys::RenderStateCursor>(),
            viewport_has_value: false,
            viewport_x: 0,
            viewport_y: 0,
            wide_tail: false,
            visible: false,
            blinking: false,
            password_input: false,
            visual_style: sys::RENDER_STATE_CURSOR_VISUAL_STYLE_BLOCK,
        };
        // SAFETY: a sized `GhosttyRenderStateCursor`.
        unsafe {
            sys::ghostty_render_state_get(
                self.state(),
                sys::RENDER_STATE_DATA_CURSOR,
                (&raw mut c).cast(),
            )
        };
        FrameCursor {
            // "When viewport_has_value is false, viewport_x, viewport_y, and
            // wide_tail contain undefined data and must not be read."
            at: c.viewport_has_value.then_some((c.viewport_x, c.viewport_y)),
            wide_tail: c.viewport_has_value && c.wide_tail,
            visible: c.visible,
            blinking: c.blinking,
            password: c.password_input,
            shape: match c.visual_style {
                sys::RENDER_STATE_CURSOR_VISUAL_STYLE_BAR => CursorShape::Bar,
                sys::RENDER_STATE_CURSOR_VISUAL_STYLE_UNDERLINE => CursorShape::Underline,
                sys::RENDER_STATE_CURSOR_VISUAL_STYLE_BLOCK_HOLLOW => CursorShape::Hollow,
                _ => CursorShape::Block,
            },
        }
    }

    /// The colours.
    #[must_use]
    pub fn colors(&self) -> FrameColors {
        let colors = raw_colors(self.state());
        FrameColors {
            background: colors.background,
            foreground: colors.foreground,
            cursor: colors.cursor_has_value.then_some(colors.cursor),
            palette: Box::new(colors.palette),
        }
    }

    /// Visits every row, top to bottom, with its cells read in one library
    /// call each into reused buffers.
    pub fn rows(&mut self, mut visit: impl FnMut(&RowView<'_>)) {
        let palette = raw_colors(self.inner.state).palette;
        let inner = &mut *self.inner;
        let mut rows = inner.rows;
        // SAFETY: fills our pre-allocated iterator from our render state.
        if unsafe {
            sys::ghostty_render_state_get(
                inner.state,
                sys::RENDER_STATE_DATA_ROW_ITERATOR,
                (&raw mut rows).cast(),
            )
        } != sys::SUCCESS
        {
            return;
        }
        // SAFETY: the iterator was just filled; everything read below lives in
        // the render state, which the lock keeps from being updated meanwhile.
        while unsafe { sys::ghostty_render_state_row_iterator_next(rows) } {
            let (mut dirty, mut y) = (false, 0i32);
            let mut selection = sys::RenderStateRowSelection {
                size: size_of::<sys::RenderStateRowSelection>(),
                start_x: 0,
                end_x: 0,
            };
            let mut cells = inner.cells;
            // SAFETY: each output has its documented type.
            let selected = unsafe {
                sys::ghostty_render_state_row_get(
                    rows,
                    sys::RENDER_STATE_ROW_DATA_DIRTY,
                    (&raw mut dirty).cast(),
                );
                sys::ghostty_render_state_row_get(
                    rows,
                    sys::RENDER_STATE_ROW_DATA_VIEWPORT_Y,
                    (&raw mut y).cast(),
                );
                sys::ghostty_render_state_row_get(
                    rows,
                    sys::RENDER_STATE_ROW_DATA_CELLS,
                    (&raw mut cells).cast(),
                );
                sys::ghostty_render_state_row_get(
                    rows,
                    sys::RENDER_STATE_ROW_DATA_SELECTION,
                    (&raw mut selection).cast(),
                ) == sys::SUCCESS
            };
            let selection = selected.then_some((selection.start_x, selection.end_x));
            inner.row_cells.clear();
            inner.row_text.clear();
            let mut x = 0u16;
            // SAFETY: the cells were just filled for this row.
            while unsafe { sys::ghostty_render_state_row_cells_next(cells) } {
                // SAFETY: as above.
                let mut cell = unsafe { read_cell(cells, &mut inner.grapheme, &palette) };
                let start = u32::try_from(inner.row_text.len()).unwrap_or(u32::MAX);
                let text = String::from_utf8_lossy(&inner.grapheme[..cell.text.1 as usize]);
                inner.row_text.push_str(&text);
                cell.text = (start, u32::try_from(text.len()).unwrap_or(0));
                cell.selected = selection.is_some_and(|(a, b)| (a..=b).contains(&x));
                inner.row_cells.push(cell);
                x = x.saturating_add(1);
            }
            visit(&RowView {
                y,
                dirty,
                selection,
                cells: &inner.row_cells,
                text: &inner.row_text,
            });
        }
    }

    /// Changes whenever the image placements do: reread [`Frame::images`]
    /// only then.
    #[must_use]
    pub fn images_generation(&self) -> u64 {
        let mut g = 0u64;
        // SAFETY: a `uint64_t` output.
        unsafe {
            sys::ghostty_render_state_get(
                self.state(),
                sys::RENDER_STATE_DATA_IMAGE_PLACEMENTS_GENERATION,
                (&raw mut g).cast(),
            );
        }
        g
    }

    /// The kitty image placements on the frame. Iteration order is not
    /// drawing order: sort by layer, then by `z`.
    #[must_use]
    pub fn images(&self) -> Vec<Placement> {
        let mut images = self.inner.images;
        let mut out = Vec::new();
        // SAFETY: fills our pre-allocated iterator.
        if unsafe {
            sys::ghostty_render_state_get(
                self.state(),
                sys::RENDER_STATE_DATA_IMAGE_ITERATOR,
                (&raw mut images).cast(),
            )
        } != sys::SUCCESS
        {
            return out;
        }
        // SAFETY: the iterator was just filled; each output has its type.
        while unsafe { sys::ghostty_render_state_image_iterator_next(images) } {
            let mut protocol = sys::IMAGE_PROTOCOL_KITTY;
            let (mut id, mut generation, mut layer, mut z) =
                (0u32, 0u64, sys::RENDER_STATE_IMAGE_LAYER_ABOVE_TEXT, 0i32);
            let (mut column, mut row) = (0i32, 0i32);
            let (mut columns, mut rows) = (0u32, 0u32);
            let (mut offset_x, mut offset_y, mut width, mut height) = (0u32, 0u32, 0u32, 0u32);
            let (mut source_x, mut source_y, mut source_width, mut source_height) =
                (0u32, 0u32, 0u32, 0u32);
            let keys = [
                sys::RENDER_STATE_IMAGE_DATA_PROTOCOL,
                sys::RENDER_STATE_IMAGE_DATA_IMAGE_ID,
                sys::RENDER_STATE_IMAGE_DATA_IMAGE_GENERATION,
                sys::RENDER_STATE_IMAGE_DATA_LAYER,
                sys::RENDER_STATE_IMAGE_DATA_Z,
                sys::RENDER_STATE_IMAGE_DATA_VIEWPORT_COL,
                sys::RENDER_STATE_IMAGE_DATA_VIEWPORT_ROW,
                sys::RENDER_STATE_IMAGE_DATA_GRID_COLS,
                sys::RENDER_STATE_IMAGE_DATA_GRID_ROWS,
                sys::RENDER_STATE_IMAGE_DATA_CELL_OFFSET_X,
                sys::RENDER_STATE_IMAGE_DATA_CELL_OFFSET_Y,
                sys::RENDER_STATE_IMAGE_DATA_PIXEL_WIDTH,
                sys::RENDER_STATE_IMAGE_DATA_PIXEL_HEIGHT,
                sys::RENDER_STATE_IMAGE_DATA_SOURCE_X,
                sys::RENDER_STATE_IMAGE_DATA_SOURCE_Y,
                sys::RENDER_STATE_IMAGE_DATA_SOURCE_WIDTH,
                sys::RENDER_STATE_IMAGE_DATA_SOURCE_HEIGHT,
            ];
            let mut values: [*mut c_void; 17] = [
                (&raw mut protocol).cast(),
                (&raw mut id).cast(),
                (&raw mut generation).cast(),
                (&raw mut layer).cast(),
                (&raw mut z).cast(),
                (&raw mut column).cast(),
                (&raw mut row).cast(),
                (&raw mut columns).cast(),
                (&raw mut rows).cast(),
                (&raw mut offset_x).cast(),
                (&raw mut offset_y).cast(),
                (&raw mut width).cast(),
                (&raw mut height).cast(),
                (&raw mut source_x).cast(),
                (&raw mut source_y).cast(),
                (&raw mut source_width).cast(),
                (&raw mut source_height).cast(),
            ];
            // SAFETY: seventeen keys, seventeen outputs of the documented types.
            let r = unsafe {
                sys::ghostty_render_state_image_get_multi(
                    images,
                    keys.len(),
                    keys.as_ptr(),
                    values.as_mut_ptr(),
                    null_mut(),
                )
            };
            // "Skip any placement whose protocol you don't support."
            if r != sys::SUCCESS || protocol != sys::IMAGE_PROTOCOL_KITTY {
                continue;
            }
            out.push(Placement {
                image: id,
                generation,
                layer: match layer {
                    sys::RENDER_STATE_IMAGE_LAYER_BELOW_BACKGROUND => ImageLayer::BelowBackground,
                    sys::RENDER_STATE_IMAGE_LAYER_BELOW_TEXT => ImageLayer::BelowText,
                    _ => ImageLayer::AboveText,
                },
                z,
                column,
                row,
                columns,
                rows,
                offset_px: (offset_x, offset_y),
                size_px: (width, height),
                source: (source_x, source_y, source_width, source_height),
            });
        }
        out
    }
}

/// The render state's colours, as the library lays them out.
fn raw_colors(state: sys::Handle) -> sys::RenderStateColors {
    let mut colors = sys::RenderStateColors {
        size: size_of::<sys::RenderStateColors>(),
        background: Rgb::default(),
        foreground: Rgb::default(),
        cursor: Rgb::default(),
        cursor_has_value: false,
        palette: [Rgb::default(); 256],
    };
    // SAFETY: a sized `GhosttyRenderStateColors`; the caller holds the lock.
    unsafe {
        sys::ghostty_render_state_get(
            state,
            sys::RENDER_STATE_DATA_COLORS,
            (&raw mut colors).cast(),
        )
    };
    colors
}

/// A style colour resolved through the palette.
fn resolved(c: sys::StyleColor, palette: &[Rgb; 256]) -> Option<Rgb> {
    match c.tag {
        // SAFETY: the tag says which arm is filled.
        sys::STYLE_COLOR_PALETTE => Some(palette[usize::from(unsafe { c.value.palette })]),
        // SAFETY: as above.
        sys::STYLE_COLOR_RGB => Some(unsafe { c.value.rgb }),
        _ => None,
    }
}

/// One cell, read with ONE call for what the row's cells can give (the raw
/// value, the style, the grapheme and the background) and one pure call for
/// its width, which only the raw value carries. The grapheme's bytes are left
/// in `grapheme`, their length in the returned cell's `text.1`.
///
/// # Safety
///
/// `cells` must be positioned on a cell of a row filled under the lock.
unsafe fn read_cell(cells: sys::Handle, grapheme: &mut Vec<u8>, palette: &[Rgb; 256]) -> Cell {
    let mut raw = 0u64;
    let mut style = sys::Style::sized();
    let mut bg = Rgb::default();
    let mut buffer = sys::Buffer {
        ptr: grapheme.as_mut_ptr(),
        cap: grapheme.len(),
        len: 0,
    };
    // The background LAST: "no background" is an error the batch stops at,
    // and it must not stop the fields before it.
    let keys = [
        sys::RENDER_STATE_ROW_CELLS_DATA_RAW,
        sys::RENDER_STATE_ROW_CELLS_DATA_STYLE,
        sys::RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_UTF8,
        sys::RENDER_STATE_ROW_CELLS_DATA_BG_COLOR,
    ];
    let mut values: [*mut c_void; 4] = [
        (&raw mut raw).cast(),
        (&raw mut style).cast(),
        (&raw mut buffer).cast(),
        (&raw mut bg).cast(),
    ];
    let mut written = 0usize;
    // SAFETY: the caller's promise; four keys, four outputs of their types.
    let mut r = unsafe {
        sys::ghostty_render_state_row_cells_get_multi(
            cells,
            keys.len(),
            keys.as_ptr(),
            values.as_mut_ptr(),
            &raw mut written,
        )
    };
    if r == sys::OUT_OF_SPACE && written == 2 {
        // A grapheme longer than the scratch: grow it, read the rest alone.
        grapheme.resize(buffer.len, 0);
        buffer = sys::Buffer {
            ptr: grapheme.as_mut_ptr(),
            cap: grapheme.len(),
            len: 0,
        };
        // SAFETY: as above.
        unsafe {
            sys::ghostty_render_state_row_cells_get(
                cells,
                sys::RENDER_STATE_ROW_CELLS_DATA_GRAPHEMES_UTF8,
                (&raw mut buffer).cast(),
            );
            r = sys::ghostty_render_state_row_cells_get(
                cells,
                sys::RENDER_STATE_ROW_CELLS_DATA_BG_COLOR,
                (&raw mut bg).cast(),
            );
        }
        written = if r == sys::SUCCESS { 4 } else { 3 };
    }
    let has_bg = r == sys::SUCCESS && written == 4;
    // SAFETY: `WIDE` reads a value, no state.
    let wide = unsafe { cell_field(raw, sys::CELL_DATA_WIDE, sys::CELL_WIDE_NARROW) };
    let text_len = if written >= 3 {
        buffer.len.min(grapheme.len())
    } else {
        0
    };
    Cell {
        text: (0, u32::try_from(text_len).unwrap_or(0)),
        style: cell_style(&style),
        fg: resolved(style.fg_color, palette),
        bg: has_bg.then_some(bg),
        underline_colour: resolved(style.underline_color, palette),
        width: cell_width(wide),
        selected: false,
    }
}

/// Whether `mirror`'s terminal and `vt`'s write out the same VT — every
/// extra the formatter knows (palette, modes, tab stops, cursor, charsets…)
/// included — and have the same cursor and title. `None` when they agree;
/// otherwise both forms, for the message.
///
/// A question for a comparison, never for a running terminal: the
/// differential asks it on every cut of the corpus, and this file's tests on
/// every cut of their streams. The answer is both screens and titles whole —
/// what a program wrote — so it belongs in a verdict, never in a log.
#[must_use]
pub fn disagreement(vt: &Vt, mirror: &Mirror) -> Option<String> {
    // A refusal is a disagreement, never two empty outputs that agree: a
    // formatter refusing on both sides would otherwise pass every cut.
    let (a, b) = match (vt.format(Format::Vt), mirror.format(Format::Vt)) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => {
            return Some(format!(
                "the formatter refused: holder {:?}, mirror {:?}",
                a.err(),
                b.err()
            ));
        }
    };
    let (ca, cb) = (vt.cursor(), mirror.cursor());
    let (ta, tb) = (vt.title(), mirror.title());
    (a != b || ca != cb || ta != tb).then(|| {
        format!(
            "holder: {:?} {ca:?} {ta:?}\nmirror: {:?} {cb:?} {tb:?}",
            String::from_utf8_lossy(&a),
            String::from_utf8_lossy(&b)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLUMNS: u16 = 20;
    const ROWS: u16 = 6;

    fn vt() -> Vt {
        let mut vt = Vt::new(COLUMNS, ROWS, &Config::for_version("9.9")).expect("a terminal");
        vt.resize(COLUMNS, ROWS, 8, 16).expect("a size");
        vt.set_colors(&Colors::of_a_test()).expect("colours");
        let _ = vt.take_effects();
        vt
    }

    fn fed(bytes: &[u8]) -> Vt {
        let mut vt = vt();
        vt.feed(bytes);
        vt
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    /// Streams with every kind of place a cut can fall: inside a CSI, an OSC
    /// ended by BEL and by ST, a DCS, a kitty APC carrying an image, UTF-8 of
    /// two, three and four bytes, a wide character, styles, a scroll region,
    /// the alternate screen, tab stops, a charset and a hyperlink.
    const STREAMS: &[&[u8]] = &[
        b"plain text\r\nsecond line\r\n\x1b[1;31mred bold\x1b[0m \x1b[38;2;10;20;30mrgb\x1b[m",
        b"\x1b]0;a title\x07\x1b]2;another\x1b\\\x1b]8;;https://example.org\x1b\\link\x1b]8;;\x1b\\",
        "caf\u{e9} \u{65e5}\u{672c} \u{1f600} e\u{301}\r\n\x1b[2;4r\x1b[3;1Hin region\n\n\n"
            .as_bytes(),
        b"\x1b[?1049h\x1b[Halt screen\x1b[?1049l\x1b[3g\x1bH\x1b(0lqk\x1b(B\x1b[?2004h",
        b"\x1b_Gf=24,s=1,v=1,a=T,i=7;AAAA\x1b\\\x1bP+q544e\x1b\\after the image",
        b"\x1b[5;5H\x1b7abc\x1b[1;1Hxyz\x1b8Z\x1b[4h\x1b[?25l\x1b[3 q",
    ];

    #[test]
    fn every_handle_crosses_a_thread() {
        // FAILS IF a type that a holder or a viewer moves to its own thread
        // stops being `Send` — the engine thread owns the `Vt`, a window's
        // reader feeds the `Mirror`, a worker ticks the `Search`, and the
        // render state is shared with the mirror's hold callback.
        const fn sendable<T: Send>() {}
        const fn shareable<T: Send + Sync>() {}
        sendable::<Vt>();
        sendable::<Mirror>();
        sendable::<Decoder>();
        sendable::<Search>();
        shareable::<RenderState>();
    }

    #[test]
    fn it_writes_two_letters_and_reads_them_back() {
        // THE TEST THAT DECIDES EVERYTHING ELSE. If the binding is wrong — the
        // wrong tag, the wrong enum value, a struct of the wrong shape — it
        // shows here, on two characters, instead of showing up as two engines
        // mysteriously disagreeing over a whole screen.
        let vt = fed(b"hi");
        let (h, i) = (vt.written(0, 0), vt.written(1, 0));
        assert_eq!(h.map(|c| c.text).as_deref(), Some("h"));
        assert_eq!(i.map(|c| c.text).as_deref(), Some("i"));
        assert_eq!((vt.cursor().x, vt.cursor().y), (2, 0));
    }

    #[test]
    fn a_terminal_asked_for_at_zero_is_one_cell_everywhere() {
        // FAILS IF the zero is clamped only for the library: `size()` and the
        // grid would then say 0x0 of a terminal that holds a cell, and a
        // character written to it would be on no grid at all.
        let mut vt = Vt::new(0, 0, &Config::for_version("9.9")).unwrap();
        vt.feed(b"x");
        assert_eq!(vt.size(), (1, 1));
        assert_eq!(vt.written(0, 0).map(|c| c.text).as_deref(), Some("x"));
    }

    #[test]
    fn the_mirror_matches_the_holder_at_every_cut() {
        // FAILS IF a snapshot taken in the MIDDLE of a sequence loses it: a
        // window that attached at that byte would draw a different screen from
        // the holder's forever after. Continuation tracking has to be on before
        // the first write for this to hold, and every cut is tried — inside
        // CSI, OSC, DCS, APC and UTF-8 — because the defect is per position.
        for (n, stream) in STREAMS.iter().enumerate() {
            for cut in 0..=stream.len() {
                let mut holder = fed(&stream[..cut]);
                let snapshot = holder.snapshot().expect("a snapshot");
                let mut mirror = Decoder::decode(snapshot).expect("a mirror");
                holder.feed(&stream[cut..]);
                mirror.feed(&stream[cut..]);
                if let Some(d) = disagreement(&holder, &mirror) {
                    panic!("stream {n}, cut at byte {cut}:\n{d}");
                }
            }
        }
    }

    #[test]
    fn a_resize_lands_at_the_same_byte() {
        // FAILS IF the mirror can resize at a different point of the stream
        // from the holder and still agree — which would mean this comparison
        // proves nothing about where the holder's `Resize` frame is placed.
        // A position past the new width: clamped if the resize came first,
        // reflowed onto the next row if it came after.
        let (before, after) = (&b"0123456789abcdefghijklm"[..], &b"\x1b[1;15Hx"[..]);
        let mut holder = fed(before);
        let mut mirror = Decoder::decode(holder.snapshot().unwrap()).unwrap();
        holder.resize(10, ROWS, 8, 16).unwrap();
        mirror.resize(10, ROWS, 8, 16).unwrap();
        holder.feed(after);
        mirror.feed(after);
        assert_eq!(disagreement(&holder, &mirror), None);

        // The control: the same bytes, the resize one frame late.
        let mut holder = fed(before);
        let mut late = Decoder::decode(holder.snapshot().unwrap()).unwrap();
        holder.resize(10, ROWS, 8, 16).unwrap();
        holder.feed(after);
        late.feed(after);
        late.resize(10, ROWS, 8, 16).unwrap();
        assert!(
            disagreement(&holder, &late).is_some(),
            "a resize applied at the wrong byte went unnoticed"
        );
    }

    #[test]
    fn history_arrives_after_ready() {
        // FAILS IF the mirror is only usable after the whole history is
        // decoded, or if the history pages never arrive: the window draws at
        // READY and prepends the past page by page.
        let mut holder = vt();
        // Enough to fill more than the page that holds the screen: history
        // in that page travels before READY, and only the pages past it
        // arrive one by one.
        for i in 0..40_000 {
            holder.feed(format!("line {i}\r\n").as_bytes());
        }
        let mut decoder = Decoder::of_bytes(holder.snapshot().unwrap()).unwrap();
        let mut mirror = decoder.ready().unwrap();
        mirror.feed(b"live");
        let early = mirror.scrollbar().total;
        let mut pages = 0;
        for restored in &mut decoder {
            let restored = restored.unwrap();
            if matches!(restored, Restored::Page { rows } if rows > 0) {
                pages += 1;
            }
        }
        assert!(pages > 0, "no history page was restored");
        assert!(
            mirror.scrollbar().total > early,
            "history did not grow the scrollback"
        );
        assert!(String::from_utf8_lossy(&mirror.format(Format::Plain).unwrap()).contains("live"));
    }

    #[test]
    fn cursor_and_status_queries_are_answered_to_the_pty() {
        // FAILS IF the writer callback is not installed or not drained: a
        // program asking where the cursor is waits for an answer forever.
        let mut vt = fed(b"\x1b[2;3H\x1b[6n\x1b[5n");
        let fx = vt.take_effects();
        assert!(contains(&fx.to_pty, b"\x1b[2;3R"), "{:?}", fx.to_pty);
        assert!(contains(&fx.to_pty, b"\x1b[0n"), "{:?}", fx.to_pty);
        assert!(vt.take_effects().to_pty.is_empty(), "taken, not repeated");
    }

    #[test]
    fn program_status_reports_become_effects_and_the_probe_is_answered() {
        // FAILS IF OSC 7501 is not wired: Claude Code asks `7501;?` first and
        // stays silent where nobody answers, and the agent marks never light.
        let mut vt = fed(b"\x1b]7501;?\x1b\\");
        let fx = vt.take_effects();
        assert!(
            contains(&fx.to_pty, b"\x1b]7501;?:states="),
            "{:?}",
            String::from_utf8_lossy(&fx.to_pty)
        );
        vt.feed(b"\x1b]7501;state=working:app=claude-code\x1b\\");
        vt.feed(b"\x1b]7501;state=blocked:kind=permission:app=claude-code\x07");
        vt.feed(b"\x1b]7501;state=done:app=claude-code\x1b\\");
        let states: Vec<(ProgramState, BlockedOn, String)> = vt
            .take_effects()
            .events
            .into_iter()
            .filter_map(|e| match e {
                Effect::ProgramStatus(s) => Some((s.state, s.blocked_on, s.app)),
                _ => None,
            })
            .collect();
        let app = || "claude-code".to_owned();
        assert_eq!(
            states,
            [
                (ProgramState::Working, BlockedOn::Unsaid, app()),
                (ProgramState::Blocked, BlockedOn::Permission, app()),
                (ProgramState::Done, BlockedOn::Unsaid, app()),
            ]
        );
    }

    #[test]
    fn a_mirror_has_no_way_to_write_to_the_pty() {
        // FAILS IF a mirror ever gets a PTY writer. The LIBRARY is asked:
        // `ghostty_terminal_paste` refuses with INVALID_VALUE exactly when no
        // writer is installed — and the holder's answer proves the probe can
        // tell the two apart.
        let holder = fed(b"text");
        let mut mirror = Decoder::decode(holder.snapshot().unwrap()).unwrap();
        mirror.feed(b"\x1b[6n\x1b[c\x1b]7501;?\x1b\\\x1b]11;?\x07");
        let probe = |terminal: sys::Handle| {
            let mime = borrowed(b"text/plain");
            let mut data: &[u8] = b"x";
            let paste = sys::Paste {
                size: size_of::<sys::Paste>(),
                location: sys::CLIPBOARD_LOCATION_STANDARD,
                source: sys::PASTE_SOURCE_TEXT,
                mimes: &raw const mime,
                mimes_len: 1,
                reader: sys::MimeReader {
                    read: Some(read_paste),
                    userdata: (&raw mut data).cast(),
                },
                allow_unsafe: false,
            };
            let mut written = false;
            // SAFETY: a live terminal; everything the paste points at lives
            // for the call.
            unsafe { sys::ghostty_terminal_paste(terminal, &raw const paste, &raw mut written) }
        };
        let mirror_answer = {
            let _gate = mirror.shared.lock();
            probe(mirror.shared.raw())
        };
        assert_eq!(
            mirror_answer,
            sys::INVALID_VALUE,
            "the mirror can write to the PTY"
        );
        assert_eq!(probe(holder.raw()), sys::SUCCESS, "the probe cannot tell");
    }

    #[test]
    fn an_unsafe_paste_is_refused_until_allowed() {
        // FAILS IF a line break pasted outside bracketed paste reaches the
        // shell without the person confirming — it runs the command.
        let mut vt = vt();
        let text = "ls\nrm -rf ~\n";
        assert!(!is_paste_safe(text));
        assert!(is_paste_safe("echo hi"));
        assert_eq!(
            vt.paste(text, PasteSource::Clipboard, false),
            Err(PasteRefused::Unsafe)
        );
        assert!(vt.take_effects().to_pty.is_empty(), "a refused paste wrote");
        assert_eq!(vt.paste(text, PasteSource::Clipboard, true), Ok(true));
        assert!(contains(&vt.take_effects().to_pty, b"rm -rf ~"));
        // Inside bracketed paste the same text is safe, and arrives bracketed.
        vt.feed(b"\x1b[?2004h");
        assert_eq!(vt.paste(text, PasteSource::Clipboard, false), Ok(true));
        let out = vt.take_effects().to_pty;
        assert!(
            out.starts_with(b"\x1b[200~") && out.ends_with(b"\x1b[201~"),
            "{out:?}"
        );
    }

    #[test]
    fn xtversion_names_the_distro_and_the_library() {
        // FAILS IF XTVERSION answers the library's default name: a program
        // that adapts to the terminal would think it runs in a bare libghostty.
        let mut vt = fed(b"\x1b[>q");
        let reply = String::from_utf8_lossy(&vt.take_effects().to_pty).into_owned();
        let expected = format!("ronoctua 9.9 (libghostty-vt {RELEASE})");
        assert!(reply.contains(&expected), "{reply:?}");
        // A release, digits and dots — not the ABI's `0.1.0-dev`.
        assert!(
            !RELEASE.is_empty() && RELEASE.chars().all(|c| c.is_ascii_digit() || c == '.'),
            "{RELEASE:?}"
        );
        assert!(!library_version().is_empty());
    }

    #[test]
    fn da1_claims_sixel_only_when_told() {
        // FAILS IF sixel is advertised by a terminal with no filter to turn it
        // into images (programs would send pictures nobody draws), or not
        // advertised by one that has it.
        let mut vt = fed(b"\x1b[c");
        assert_eq!(vt.take_effects().to_pty, b"\x1b[?62;4;22;52c");
        let config = Config {
            sixel: false,
            ..Config::for_version("9.9")
        };
        let mut plain = Vt::new(COLUMNS, ROWS, &config).unwrap();
        plain.feed(b"\x1b[c\x1b[>c");
        assert_eq!(plain.take_effects().to_pty, b"\x1b[?62;22;52c\x1b[>1;10;0c");
    }

    #[test]
    fn the_persons_cursor_is_the_cursor_of_now_and_of_a_reset() {
        // FAILS IF the default cursor options are bound wrong (22, 23): a
        // terminal born with the person's bar draws the library's block, a
        // program's reset (DECSCUSR 0) goes back to the block, or a mirror
        // decoded from its snapshot goes back to another cursor than its
        // holder.
        let shape_of = |screen: &dyn Fn(&RenderState)| {
            let state = RenderState::new().unwrap();
            screen(&state);
            let c = state.frame().cursor();
            (c.shape, c.blinking)
        };
        let bar = CursorDefault {
            shape: CursorShape::Bar,
            blinks: false,
        };
        let config = Config {
            cursor: Some(bar),
            ..Config::for_version("9.9")
        };
        let mut vt = Vt::new(COLUMNS, ROWS, &config).unwrap();
        assert_eq!(
            shape_of(&|s| s.update(&vt).unwrap()),
            (CursorShape::Bar, false),
            "born with another cursor"
        );
        // A PROGRAM'S UNDERLINE, then its reset.
        vt.feed(b"\x1b[4 q");
        assert_eq!(
            shape_of(&|s| s.update(&vt).unwrap()).0,
            CursorShape::Underline
        );
        // THE SNAPSHOT CARRIES THE DEFAULT: a mirror decoded from it is told
        // nothing else, and goes back where its holder does.
        let mut mirror = Decoder::decode(vt.snapshot().unwrap()).unwrap();
        assert_eq!(
            shape_of(&|s| s.update(&mirror).unwrap()).0,
            CursorShape::Underline
        );
        vt.feed(b"\x1b[0 q");
        mirror.feed(b"\x1b[0 q");
        assert_eq!(
            shape_of(&|s| s.update(&vt).unwrap()),
            (CursorShape::Bar, false)
        );
        assert_eq!(
            shape_of(&|s| s.update(&mirror).unwrap()),
            (CursorShape::Bar, false),
            "the mirror went back to another cursor than its holder"
        );
        // AND THE LIBRARY'S, with no default given: a block.
        let plain = Vt::new(COLUMNS, ROWS, &Config::for_version("9.9")).unwrap();
        assert_eq!(
            shape_of(&|s| s.update(&plain).unwrap()).0,
            CursorShape::Block
        );
    }

    #[test]
    fn grapheme_clustering_is_on_and_survives_a_reset() {
        // FAILS IF mode 2027 is only set as the current value: `ESC c` would
        // turn it off, and emoji with modifiers would take the wrong width.
        let mut vt = vt();
        assert_eq!(vt.mode(Mode::GRAPHEME_CLUSTER), Some(true));
        vt.feed(b"\x1bc");
        assert_eq!(vt.mode(Mode::GRAPHEME_CLUSTER), Some(true));
    }

    #[test]
    fn the_terminfo_name_is_answered_to_xtgettcap() {
        // FAILS IF XTGETTCAP `TN` answers nothing or another name than the
        // `TERM` the PTY exports: a program asking either way must hear one.
        let mut vt = fed(b"\x1bP+q544e\x1b\\");
        let reply = String::from_utf8_lossy(&vt.take_effects().to_pty).to_ascii_lowercase();
        let hex: String = TERMINFO_NAME.bytes().map(|b| format!("{b:02x}")).collect();
        assert!(reply.contains(&hex), "{reply:?} lacks {hex}");
    }

    #[test]
    fn what_a_program_says_besides_the_screen_becomes_effects() {
        // FAILS IF the bell, the title, the pwd, a notification or a progress
        // report is dropped on the way out — each is a mark or an event the
        // footer shows.
        let mut vt = fed(b"\x07\x1b]2;the title\x07\x1b]7;file:///home/x\x1b\\\
              \x1b]9;done building\x07\x1b]9;4;1;40\x07\x1b]777;notify;T;B\x07");
        let events = vt.take_effects().events;
        assert_eq!(
            events,
            [
                Effect::Bell,
                Effect::Title("the title".to_owned()),
                Effect::Pwd("file:///home/x".to_owned()),
                Effect::Notification {
                    title: String::new(),
                    body: "done building".to_owned()
                },
                Effect::Progress {
                    state: ProgressState::Set,
                    percent: Some(40)
                },
                Effect::Notification {
                    title: "T".to_owned(),
                    body: "B".to_owned()
                },
            ]
        );
    }

    #[test]
    fn a_reset_and_a_resize_request_become_effects() {
        // FAILS IF RIS goes unreported — the holder would keep showing a title
        // and a folder the program wiped — or `CSI 8 ; rows ; columns t` loses
        // its numbers on the way out of the library's tagged union.
        let mut vt = fed(b"\x1b[8;30;100t\x1bc");
        let events: Vec<Effect> = vt
            .take_effects()
            .events
            .into_iter()
            .filter(|e| matches!(e, Effect::Reset | Effect::ResizeRequest { .. }))
            .collect();
        assert_eq!(
            events,
            [
                Effect::ResizeRequest {
                    rows: 30,
                    columns: 100
                },
                Effect::Reset
            ]
        );
    }

    #[test]
    fn compression_ends_and_keeps_the_history() {
        // FAILS IF the compression result is read wrong — a step that never
        // says "done" spins the holder's idle loop for good — or if a pass
        // changes what the history says.
        let mut vt = vt();
        for i in 0..20_000 {
            vt.feed(format!("line {i}\r\n").as_bytes());
        }
        let before = vt.format(Format::Plain).unwrap();
        let mut steps = 0u32;
        while vt.compress_step() {
            steps += 1;
            assert!(steps < 1_000_000, "compression never said it was done");
        }
        assert_eq!(vt.format(Format::Plain).unwrap(), before);
    }

    #[test]
    fn shell_integration_reports_the_exit_code() {
        // FAILS IF OSC 133's command end loses its exit code: "a long command
        // finished" with its status is built on it.
        let mut vt = fed(b"\x1b]133;A\x07$ \x1b]133;B\x07\x1b]133;C\x07out\r\n\x1b]133;D;3\x07");
        let steps: Vec<(PromptStep, Option<i32>)> = vt
            .take_effects()
            .events
            .into_iter()
            .filter_map(|e| match e {
                Effect::Prompt(p) => Some((p.step, p.exit_code)),
                _ => None,
            })
            .collect();
        assert_eq!(
            steps,
            [
                (PromptStep::PromptStart, None),
                (PromptStep::InputStart, None),
                (PromptStep::OutputStart, None),
                (PromptStep::CommandEnd, Some(3)),
            ]
        );
    }

    #[test]
    fn clipboard_writes_are_effects_only_while_accepted() {
        // FAILS IF an OSC 52 write reaches the clipboard with no window to
        // receive it, or is lost while one is.
        let mut vt = fed(b"\x1b]52;c;aGVsbG8=\x07");
        assert!(
            vt.take_effects().events.is_empty(),
            "a write with nobody attached"
        );
        vt.accept_clipboard_writes(true);
        vt.feed(b"\x1b]52;c;aGVsbG8=\x07");
        assert_eq!(
            vt.take_effects().events,
            [Effect::ClipboardWrite {
                clipboard: Clipboard::Standard,
                contents: vec![("text/plain".to_owned(), b"hello".to_vec())]
            }]
        );
    }

    #[test]
    fn a_clipboard_read_is_answered_by_the_reader_alone() {
        // FAILS IF an OSC 52 read is answered with nobody consenting, or the
        // reader's answer does not reach the program.
        let mut vt = fed(b"\x1b]52;c;?\x07");
        assert!(
            !contains(&vt.take_effects().to_pty, b"]52;"),
            "answered with no reader"
        );
        vt.set_clipboard_reader(Some(Box::new(|ask: &ClipboardAsk| {
            (ask.clipboard == Clipboard::Standard).then(|| "hi".to_owned())
        })))
        .unwrap();
        vt.feed(b"\x1b]52;c;?\x07");
        assert!(
            contains(&vt.take_effects().to_pty, b"aGk="),
            "the answer did not arrive"
        );
    }

    #[test]
    fn colours_are_answered_from_what_the_window_set() {
        // FAILS IF OSC 10/11/4 queries answer something other than the
        // window's colours — a program choosing light or dark text by them
        // would choose wrong.
        let mut vt = fed(b"\x1b]11;?\x07\x1b]4;1;?\x07");
        let window = Colors::of_a_test();
        // The spelling Ghostty answers in: 16 bits a channel, the high byte
        // repeated, and the terminator the query came with.
        let spelt = |slot: &str, c: Rgb| {
            format!(
                "\x1b]{slot};rgb:{0:02x}{0:02x}/{1:02x}{1:02x}/{2:02x}{2:02x}\x07",
                c.r, c.g, c.b
            )
        };
        assert_eq!(
            String::from_utf8_lossy(&vt.take_effects().to_pty),
            spelt("11", window.background) + &spelt("4;1", window.palette[1])
        );
    }

    #[test]
    fn a_scheme_change_is_reported_only_to_a_program_that_asked() {
        // FAILS IF mode 2031 is ignored (the program never learns the theme
        // changed) or reported unasked (stray bytes in a shell's input).
        let mut vt = vt();
        let mut light = Colors::of_a_test();
        light.scheme = Scheme::Light;
        vt.set_colors(&light).unwrap();
        assert!(vt.take_effects().to_pty.is_empty());
        vt.feed(b"\x1b[?2031h");
        vt.set_colors(&Colors::of_a_test()).unwrap();
        assert_eq!(vt.take_effects().to_pty, b"\x1b[?997;1n");
        vt.feed(b"\x1b[?996n");
        assert_eq!(vt.take_effects().to_pty, b"\x1b[?997;1n");
    }

    fn key(key: Key, mods: Mods, text: &str) -> KeyInput {
        KeyInput {
            key,
            action: KeyAction::Press,
            mods,
            consumed: Mods::NONE,
            text: text.to_owned(),
            unshifted: text.chars().next(),
            composing: false,
        }
    }

    #[test]
    fn keys_are_encoded_against_the_programs_modes() {
        // FAILS IF the encoder does not read the terminal's modes: vim in
        // application cursor mode would see the wrong arrows, and a kitty
        // keyboard program would never get its protocol.
        let mut vt = vt();
        assert_eq!(vt.encode_key(&key(Key::ArrowUp, Mods::NONE, "")), b"\x1b[A");
        vt.feed(b"\x1b[?1h");
        assert_eq!(vt.encode_key(&key(Key::ArrowUp, Mods::NONE, "")), b"\x1bOA");
        assert_eq!(vt.encode_key(&key(Key::C, Mods::CTRL, "c")), b"\x03");
        assert_eq!(vt.encode_key(&key(Key::A, Mods::NONE, "a")), b"a");
        vt.feed(b"\x1b[>1u");
        assert_eq!(vt.kitty_keyboard_flags(), 1);
        assert_eq!(
            vt.encode_key(&key(Key::Escape, Mods::NONE, "")),
            b"\x1b[27u"
        );
    }

    #[test]
    fn the_mouse_is_encoded_only_for_a_program_that_tracks_it() {
        // FAILS IF clicks reach a program that did not ask (garbage in a
        // shell), or fail to reach one that did, in its format.
        let geometry = Geometry {
            width_px: 160,
            height_px: 96,
            cell_width_px: 8,
            cell_height_px: 16,
            padding_top_px: 0,
            padding_bottom_px: 0,
            padding_left_px: 0,
            padding_right_px: 0,
        };
        let press = MouseInput {
            action: MouseAction::Press,
            button: Some(Button::Left),
            mods: Mods::NONE,
            x_px: 20.0,
            y_px: 20.0,
            any_button_held: true,
        };
        let mut vt = vt();
        assert!(!vt.mouse_tracking());
        assert!(vt.encode_mouse(&press, &geometry).is_empty());
        vt.feed(b"\x1b[?1000h\x1b[?1006h");
        assert!(vt.mouse_tracking());
        assert_eq!(vt.encode_mouse(&press, &geometry), b"\x1b[<0;3;2M");
    }

    #[test]
    fn focus_is_reported_only_in_mode_1004() {
        // FAILS IF focus reports reach a program that did not ask for them —
        // `^[[I` typed into a shell on every window switch.
        let mut vt = vt();
        assert!(vt.encode_focus(true).is_empty());
        vt.feed(b"\x1b[?1004h");
        assert_eq!(vt.encode_focus(true), b"\x1b[I");
        assert_eq!(vt.encode_focus(false), b"\x1b[O");
    }

    #[test]
    fn a_hold_is_an_effect_and_ends_when_the_mode_is_reset() {
        // FAILS IF the holder's one-second deadline (which resets the mode
        // itself, and the library does not call back for that) leaves the
        // hold standing — the screen would freeze for good.
        let mut vt = fed(b"\x1b[?2026h");
        assert!(vt.held());
        assert_eq!(vt.take_effects().events, [Effect::RenderHold(true)]);
        vt.set_mode(Mode::SYNC_OUTPUT, false).unwrap();
        assert!(!vt.held());
        assert_eq!(vt.mode(Mode::SYNC_OUTPUT), Some(false));
    }

    #[test]
    fn pixels_reach_the_size_reports() {
        // FAILS IF the cell's pixels are dropped on the way in: `CSI 14 t`
        // (and kitty images) would size by a cell of nothing.
        let mut vt = fed(b"\x1b[14t\x1b[18t");
        let out = vt.take_effects().to_pty;
        assert!(
            contains(&out, b"\x1b[4;96;160t"),
            "{:?}",
            String::from_utf8_lossy(&out)
        );
        assert!(
            contains(&out, b"\x1b[8;6;20t"),
            "{:?}",
            String::from_utf8_lossy(&out)
        );
    }

    fn frame_cells(vt: &Vt) -> Vec<(String, Cell)> {
        let state = RenderState::new().unwrap();
        state.update(vt).unwrap();
        let mut frame = state.frame();
        let mut out = Vec::new();
        frame.rows(|row| {
            if row.y == 0 {
                out = row
                    .cells
                    .iter()
                    .map(|c| (row.text(c).to_owned(), *c))
                    .collect();
            }
        });
        out
    }

    #[test]
    fn the_render_state_reads_styles_colours_and_widths() {
        // FAILS IF a style bit, a resolved colour or a width is lost between
        // the library and a cell — the renderer draws what this hands it.
        let vt = fed("\x1b[1;3;4:3;53;38;5;196;48;2;1;2;3mX\x1b[0m\u{65e5}\x1b[5mb".as_bytes());
        let cells = frame_cells(&vt);
        let (text, x) = &cells[0];
        assert_eq!(text, "X");
        assert!(x.style.bold && x.style.italic && x.style.overline);
        assert_eq!(x.style.underline, Underline::Curly);
        assert_eq!(x.fg, Some(Colors::of_a_test().palette[196]));
        assert_eq!(x.bg, Some(Rgb { r: 1, g: 2, b: 3 }));
        assert_eq!(cells[1].0, "\u{65e5}");
        assert_eq!(cells[1].1.width, CellWidth::Wide);
        assert_eq!(cells[2].1.width, CellWidth::SpacerTail);
        assert_eq!(cells[2].0, "");
        assert!(cells[3].1.style.blink);
        assert_eq!(cells[3].1.fg, None, "the default foreground is no colour");
    }

    #[test]
    fn a_written_cell_keeps_the_programs_own_words() {
        // FAILS IF `written` resolves a colour the program named by its
        // palette entry (two engines handed different palettes would then
        // disagree about a cell they read alike), loses a style bit or a
        // width, or misses the background a cell erased with no text carries
        // in its content rather than in a style.
        let vt = fed("\x1b[1;4:3;31;48;2;1;2;3mX\x1b[0m\u{65e5}\x1b[44m\x1b[K".as_bytes());
        let x = vt.written(0, 0).expect("a cell");
        assert_eq!(x.text, "X");
        assert!(x.style.bold && !x.style.italic);
        assert_eq!(x.style.underline, Underline::Curly);
        assert_eq!(x.fg, AskedColour::Palette(1));
        assert_eq!(x.bg, AskedColour::Rgb(Rgb { r: 1, g: 2, b: 3 }));
        assert_eq!(x.width, CellWidth::Narrow);

        let wide = vt.written(1, 0).expect("a cell");
        assert_eq!(
            (wide.text.as_str(), wide.width),
            ("\u{65e5}", CellWidth::Wide)
        );
        assert_eq!(wide.fg, AskedColour::Default, "SGR 0 asks for no colour");
        let spacer = vt.written(2, 0).expect("a cell");
        assert_eq!(
            (spacer.text.as_str(), spacer.width),
            ("", CellWidth::SpacerTail)
        );

        let erased = vt.written(COLUMNS - 1, 0).expect("a cell");
        assert_eq!(erased.text, "");
        assert_eq!(erased.bg, AskedColour::Palette(4));
        assert_eq!(vt.written(COLUMNS, 0), None, "outside the active area");
    }

    #[test]
    fn the_tests_colours_are_not_the_engines_own_in_any_slot() {
        // FAILS IF a slot of the tests' colours equals the engine's default
        // for it: a test asking that slot would then pass against a terminal
        // that ignored `set_colors`.
        let vt = Vt::new(1, 1, &Config::for_version("9.9")).unwrap();
        let state = RenderState::new().unwrap();
        state.update(&vt).unwrap();
        let base = state.frame().colors();
        let window = Colors::of_a_test();
        assert_ne!(window.foreground, base.foreground);
        assert_ne!(window.background, base.background);
        assert_ne!(window.cursor, Some(base.cursor.unwrap_or(base.foreground)));
        for (i, (a, b)) in window.palette.iter().zip(base.palette.iter()).enumerate() {
            assert_ne!(a, b, "palette entry {i}");
        }
    }

    #[test]
    fn the_frame_carries_the_windows_colours_and_a_programs_changes() {
        // FAILS IF the render state's colours are read with the wrong layout —
        // a frame painted on someone else's background — or miss what a
        // program changed with OSC 4.
        let vt = fed(b"\x1b]4;1;rgb:12/34/56\x07");
        let state = RenderState::new().unwrap();
        state.update(&vt).unwrap();
        let colours = state.frame().colors();
        let window = Colors::of_a_test();
        assert_eq!(colours.background, window.background);
        assert_eq!(colours.foreground, window.foreground);
        assert_eq!(colours.cursor, window.cursor);
        assert_eq!(
            colours.palette[1],
            Rgb {
                r: 0x12,
                g: 0x34,
                b: 0x56
            }
        );
        assert_eq!(colours.palette[2], window.palette[2]);
    }

    #[test]
    fn the_frame_says_where_the_cursor_is_and_how_it_looks() {
        // FAILS IF DECSCUSR or the cursor's position do not reach the frame.
        let vt = fed(b"ab\x1b[5 q");
        let state = RenderState::new().unwrap();
        state.update(&vt).unwrap();
        let cursor = state.frame().cursor();
        assert_eq!(cursor.at, Some((2, 0)));
        assert_eq!(cursor.shape, CursorShape::Bar);
        assert!(cursor.blinking && cursor.visible);
    }

    #[test]
    fn a_held_frame_is_captured_at_the_byte_the_hold_began() {
        // FAILS IF the frame shown during synchronized output is whatever was
        // drawn last instead of what the program left before holding: a
        // program that clears and redraws in one write would flash.
        let holder = fed(b"");
        let mut mirror = Decoder::decode(holder.snapshot().unwrap()).unwrap();
        let state = RenderState::new().unwrap();
        mirror.capture_holds(&state);
        mirror.feed(b"before\x1b[?2026h\x1b[2J\x1b[Hafter");
        assert!(mirror.held());
        state.update(&mirror).unwrap();
        let first_row = |state: &RenderState| {
            let mut text = String::new();
            state.frame().rows(|row| {
                if row.y == 0 {
                    text = row.cells.iter().map(|c| row.text(c)).collect();
                }
            });
            text
        };
        assert!(
            first_row(&state).starts_with("before"),
            "{:?}",
            first_row(&state)
        );
        mirror.set_mode(Mode::SYNC_OUTPUT, false).unwrap();
        state.update(&mirror).unwrap();
        assert!(
            first_row(&state).starts_with("after"),
            "{:?}",
            first_row(&state)
        );
    }

    fn mirror_of(bytes: &[u8]) -> Mirror {
        let holder = fed(bytes);
        Decoder::decode(holder.snapshot().unwrap()).unwrap()
    }

    #[test]
    fn a_double_click_selects_a_word_and_copies_it() {
        // FAILS IF the gesture's click count is lost between presses, or the
        // selection is not installed where the formatter reads it.
        let mut mirror = mirror_of(b"hello world");
        let at = Pointer {
            column: 1,
            row: 0,
            x_px: 12.0,
            y_px: 8.0,
        };
        let repeat = Repeat {
            distance_px: 4.0,
            interval_ns: 500_000_000,
        };
        mirror.select_press(at, 1_000, repeat, false).unwrap();
        mirror.select_release(Some((1, 0))).unwrap();
        assert!(mirror.select_press(at, 2_000, repeat, false).unwrap());
        mirror.select_release(Some((1, 0))).unwrap();
        assert_eq!(mirror.selected_text().as_deref(), Some("hello"));
        mirror.select_extend(10, 0).unwrap();
        assert_eq!(mirror.selected_text().as_deref(), Some("hello world"));
        mirror.select_clear();
        assert_eq!(mirror.selected_text(), None);
        assert!(mirror.select_all().unwrap());
        assert_eq!(mirror.selected_text().as_deref(), Some("hello world"));
    }

    #[test]
    fn a_drag_selects_and_a_rectangle_is_a_block() {
        // FAILS IF the drag's geometry or the rectangle flag do not reach the
        // gesture: dragging would select nothing, or a line where a block was
        // asked for (Ctrl+Alt in the window).
        let geometry = Geometry {
            width_px: 160,
            height_px: 96,
            cell_width_px: 8,
            cell_height_px: 16,
            padding_top_px: 0,
            padding_bottom_px: 0,
            padding_left_px: 0,
            padding_right_px: 0,
        };
        let repeat = Repeat {
            distance_px: 4.0,
            interval_ns: 1,
        };
        // A quarter into the first cell and three quarters into the last: a
        // selection takes a cell only from the side of its middle the drag
        // covers, as Ghostty does.
        let at = |column: u16, row: u16, quarters: f64| Pointer {
            column,
            row,
            x_px: f64::from(column) * 8.0 + quarters * 2.0,
            y_px: f64::from(row) * 16.0 + 8.0,
        };
        let mut mirror = mirror_of(b"abcdef\r\nghijkl\r\nmnopqr");
        mirror
            .select_press(at(1, 0, 1.0), 10, repeat, false)
            .unwrap();
        assert!(mirror.select_drag(at(2, 1, 3.0), false, &geometry).unwrap());
        mirror.select_release(Some((2, 1))).unwrap();
        assert_eq!(mirror.selected_text().as_deref(), Some("bcdef\nghi"));

        mirror
            .select_press(at(1, 0, 1.0), 1_000_000_000, repeat, false)
            .unwrap();
        assert!(mirror.select_drag(at(2, 2, 3.0), true, &geometry).unwrap());
        mirror.select_release(Some((2, 2))).unwrap();
        assert_eq!(mirror.selected_text().as_deref(), Some("bc\nhi\nno"));
    }

    #[test]
    fn a_click_with_command_output_selects_what_the_command_printed() {
        // FAILS IF the behaviour table does not reach the gesture: Ctrl+click
        // on a command's output would select one cell instead of the output
        // shell integration (OSC 133) marked.
        let mut mirror = mirror_of(
            b"\x1b]133;A\x07$ \x1b]133;B\x07ls\r\n\x1b]133;C\x07one\r\ntwo\r\n\
              \x1b]133;D;0\x07\x1b]133;A\x07$ ",
        );
        let at = Pointer {
            column: 1,
            row: 1,
            x_px: 12.0,
            y_px: 24.0,
        };
        let repeat = Repeat {
            distance_px: 4.0,
            interval_ns: 1,
        };
        assert!(mirror.select_press(at, 10, repeat, true).unwrap());
        assert_eq!(mirror.selected_text().as_deref(), Some("one\ntwo"));
    }

    #[test]
    fn kitty_images_cross_the_snapshot_and_reach_the_frame() {
        // FAILS IF a kitty image a program showed BEFORE a window attached is
        // lost on the way to the mirror — the snapshot is what carries it (the
        // reason the design's re-transmitting "prelude" was dropped) — or if
        // the image's pixels and its placement cannot be read back for drawing.
        let mut decoder = Decoder::of_bytes(
            fed(b"\x1b_Gf=24,s=1,v=1,a=T,i=7;AAAA\x1b\\")
                .snapshot()
                .unwrap(),
        )
        .unwrap();
        let mirror = decoder.ready().unwrap();
        let mut pixels_arrived = false;
        for restored in &mut decoder {
            if matches!(restored.unwrap(), Restored::Image { id: 7, bytes } if bytes > 0) {
                pixels_arrived = true;
            }
        }
        assert!(pixels_arrived, "the image's pixels never arrived");
        let image = mirror.image(7).expect("the image crossed the snapshot");
        assert_eq!(
            (image.width, image.height, image.format),
            (1, 1, PixelFormat::Rgb)
        );
        assert_eq!(image.pixels.as_deref(), Some(&[0u8, 0, 0][..]));
        let state = RenderState::new().unwrap();
        state.update(&mirror).unwrap();
        let placements = state.frame().images();
        assert!(
            placements.iter().any(|p| p.image == 7
                && (p.columns, p.rows) == (1, 1)
                && (p.column, p.row) == (0, 0)),
            "{placements:?}"
        );
    }

    #[test]
    fn a_link_is_found_under_the_pointer() {
        // FAILS IF OSC 8 links do not survive the snapshot or the lookup: the
        // Ctrl+click that opens them would open nothing.
        let mirror = mirror_of(b"\x1b]8;;https://example.org\x1b\\link\x1b]8;;\x1b\\ plain");
        assert_eq!(mirror.link_at(2, 0).as_deref(), Some("https://example.org"));
        assert_eq!(mirror.link_at(6, 0), None);
    }

    #[test]
    fn a_search_finds_every_match_and_where_it_is() {
        // FAILS IF the search API is bound wrong: a find bar with no results,
        // or results drawn in the wrong place.
        let mirror = mirror_of(b"abc foo def\r\nFOO again");
        let mut search = Search::new(&mirror).unwrap();
        search.set_needle("foo").unwrap();
        search.run().unwrap();
        assert_eq!(search.total(), 2);
        let mut on_screen = search.on_screen();
        on_screen.sort_by_key(|h| (h.start.1, h.start.0));
        assert_eq!(
            on_screen,
            [
                Highlight {
                    start: (4, 0),
                    end: (6, 0)
                },
                Highlight {
                    start: (0, 1),
                    end: (2, 1)
                },
            ]
        );
        assert!(search.select_older().unwrap());
        assert!(search.selected().is_some());
    }

    #[test]
    fn the_match_walked_to_is_told_apart_from_the_others() {
        // FAILS IF the selected match is bound wrong (option 4, a sized
        // selection): the find bar's current match would look like every
        // other one, or be drawn where it is not — and before any walk there
        // is none.
        let mirror = mirror_of(b"abc foo def\r\nFOO again");
        let mut search = Search::new(&mirror).unwrap();
        search.set_needle("foo").unwrap();
        search.run().unwrap();
        let (all, selected) = search.drawn();
        assert_eq!(all.len(), 2);
        assert_eq!(selected, None, "a match is selected before any walk");
        // THE NEWEST FIRST: the first walk toward older output selects the
        // match nearest the prompt, the second the one above it.
        assert!(search.select_older().unwrap());
        let newest = Highlight {
            start: (0, 1),
            end: (2, 1),
        };
        assert_eq!(search.drawn().1, Some(newest));
        assert!(search.select_older().unwrap());
        assert_eq!(
            search.drawn().1,
            Some(Highlight {
                start: (4, 0),
                end: (6, 0)
            })
        );
        assert!(
            search.drawn().0.contains(&newest),
            "the rest are still drawn"
        );
    }

    #[test]
    fn highlights_follow_a_terminal_that_changed_since_the_last_feed() {
        // FAILS IF the highlights are positions of grid refs taken at the last
        // feed: they are valid only until the terminal next changes, and a
        // window that resized (or was fed) in between would read refs to pages
        // that were rebuilt — highlights in the wrong place, or none.
        let mut mirror = mirror_of(b"abc foo def\r\nFOO again");
        let mut search = Search::new(&mirror).unwrap();
        search.set_needle("foo").unwrap();
        search.run().unwrap();
        // A reflow rebuilds every page: at six columns the two lines take
        // four rows, the first two of which go up into the scrollback, and
        // "FOO again" now starts the screen.
        mirror.resize(6, ROWS, 8, 16).unwrap();
        assert_eq!(
            search.on_screen(),
            [Highlight {
                start: (0, 0),
                end: (2, 0)
            }]
        );
    }

    #[test]
    fn the_viewport_scrolls_back_and_returns() {
        // FAILS IF scrolling the mirror's viewport is bound wrong: the
        // scrollbar and the wheel would move nothing.
        let mut holder = vt();
        for i in 0..50 {
            holder.feed(format!("{i}\r\n").as_bytes());
        }
        let mut mirror = Decoder::decode(holder.snapshot().unwrap()).unwrap();
        assert!(mirror.at_bottom());
        mirror.scroll(Scroll::Lines(-10));
        assert!(!mirror.at_bottom());
        let bar = mirror.scrollbar();
        assert!(bar.total > bar.len && bar.offset + bar.len < bar.total);
        mirror.scroll(Scroll::Bottom);
        assert!(mirror.at_bottom());
    }

    // ---- the layouts and numbers, against the linked library ----

    /// A JSON value, enough of it to read the library's type manifest.
    #[derive(Debug)]
    enum Json {
        /// `null`, a boolean or a list: read past, never asked.
        Other,
        Number(f64),
        Text(String),
        Object(Vec<(String, Json)>),
    }

    impl Json {
        fn get(&self, key: &str) -> Option<&Self> {
            match self {
                Self::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }

        fn number(&self) -> Option<i64> {
            match self {
                #[allow(clippy::cast_possible_truncation)]
                Self::Number(n) => Some(*n as i64),
                _ => None,
            }
        }
    }

    fn parse(text: &[u8]) -> Json {
        fn ws(s: &[u8], i: &mut usize) {
            while *i < s.len() && s[*i].is_ascii_whitespace() {
                *i += 1;
            }
        }
        fn value(s: &[u8], i: &mut usize) -> Json {
            ws(s, i);
            match s[*i] {
                b'{' => {
                    *i += 1;
                    let mut fields = Vec::new();
                    loop {
                        ws(s, i);
                        if s[*i] == b'}' {
                            *i += 1;
                            return Json::Object(fields);
                        }
                        let Json::Text(k) = value(s, i) else {
                            panic!("a key")
                        };
                        ws(s, i);
                        assert_eq!(s[*i], b':');
                        *i += 1;
                        fields.push((k, value(s, i)));
                        ws(s, i);
                        if s[*i] == b',' {
                            *i += 1;
                        }
                    }
                }
                b'[' => {
                    *i += 1;
                    loop {
                        ws(s, i);
                        if s[*i] == b']' {
                            *i += 1;
                            return Json::Other;
                        }
                        value(s, i);
                        ws(s, i);
                        if s[*i] == b',' {
                            *i += 1;
                        }
                    }
                }
                b'"' => {
                    *i += 1;
                    let mut out = String::new();
                    while s[*i] != b'"' {
                        if s[*i] == b'\\' {
                            *i += 1;
                        }
                        out.push(char::from(s[*i]));
                        *i += 1;
                    }
                    *i += 1;
                    Json::Text(out)
                }
                b't' | b'n' => {
                    *i += 4;
                    Json::Other
                }
                b'f' => {
                    *i += 5;
                    Json::Other
                }
                _ => {
                    let start = *i;
                    while *i < s.len()
                        && matches!(s[*i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
                    {
                        *i += 1;
                    }
                    Json::Number(std::str::from_utf8(&s[start..*i]).unwrap().parse().unwrap())
                }
            }
        }
        value(text, &mut 0)
    }

    /// What `sys` declares: constants, struct sizes and field offsets.
    struct Declared {
        consts: Vec<(String, i64)>,
        sizes: Vec<(String, i64)>,
        offsets: Vec<(String, String, i64)>,
    }

    /// The `sys` module's declarations, read from this very file the way the
    /// C probe in `tests/checks.nix` reads them.
    fn declared() -> Declared {
        let source = include_str!("ghostty.rs");
        let start = source.find("\nmod sys {\n").expect("the sys module");
        let body = &source[start..];
        let body = &body[..body[1..].find("\n}\n").expect("its end") + 1];
        // Whitespace collapsed: `rustfmt` may break a long assertion over
        // two lines, and the declaration is the same.
        let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
        let identifier =
            |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        let number = |s: &str| {
            s.split([';', ')'])
                .next()
                .and_then(|n| n.trim().parse::<i64>().ok())
        };
        let (mut consts, mut sizes, mut offsets) = (Vec::new(), Vec::new(), Vec::new());
        for piece in flat.split("pub const ").skip(1) {
            if let Some((name, rest)) = piece.split_once(": ")
                && let Some((_, value)) = rest.split_once(" = ")
                && let Some(v) = number(value)
                && identifier(name)
            {
                consts.push((name.to_owned(), v));
            }
        }
        for piece in flat.split("size_of::<").skip(1) {
            if let Some((name, rest)) = piece.split_once(">() == ")
                && identifier(name)
                && let Some(v) = number(rest)
            {
                sizes.push((name.to_owned(), v));
            }
        }
        for piece in flat.split("offset_of!(").skip(1) {
            if let Some((names, rest)) = piece.split_once(") == ")
                && let Some((ty, field)) = names.split_once(", ")
                && identifier(ty)
                && identifier(field)
                && let Some(v) = number(rest)
            {
                offsets.push((ty.to_owned(), field.to_owned(), v));
            }
        }
        Declared {
            consts,
            sizes,
            offsets,
        }
    }

    #[test]
    fn every_key_of_the_library_has_one_w3c_name() {
        // FAILS IF a key the library knows has no name a window can send it
        // by, two keys share a name, or a name does not come back as its key.
        // SAFETY: a static, NUL-terminated string the library owns.
        let manifest = unsafe { std::ffi::CStr::from_ptr(sys::ghostty_type_json()) };
        let manifest = parse(manifest.to_bytes());
        let Some(Json::Object(values)) = manifest
            .get("types")
            .and_then(|t| t.get("GhosttyKey"))
            .and_then(|k| k.get("values"))
        else {
            panic!("the manifest has the keys")
        };
        let mut numbers: Vec<i32> = Key::NAMED.iter().map(|(k, _)| *k as i32).collect();
        numbers.sort_unstable();
        numbers.dedup();
        assert_eq!(numbers.len(), Key::NAMED.len(), "a key twice");
        // `MAX_VALUE` is the enum's width, not a key.
        let keys = values
            .iter()
            .filter(|(n, _)| !n.ends_with("MAX_VALUE"))
            .count();
        assert_eq!(numbers.len(), keys, "the library's keys, every one");
        let mut names: Vec<&str> = Key::NAMED.iter().map(|(_, n)| *n).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Key::NAMED.len(), "a name twice");
        for (key, name) in Key::NAMED {
            assert_eq!(Key::from_w3c(name), Some(*key));
            assert_eq!(key.w3c(), *name);
        }
    }

    #[test]
    fn the_layouts_and_numbers_are_the_linked_librarys() {
        // FAILS IF a size, an offset or an enum value declared in `sys`
        // differs from what the LINKED library says of itself — the case the
        // C probe in `tests/checks.nix` cannot see: headers from one build and
        // a library from another, each consistent alone.
        // SAFETY: a static, NUL-terminated string the library owns.
        let manifest = unsafe { std::ffi::CStr::from_ptr(sys::ghostty_type_json()) };
        let manifest = parse(manifest.to_bytes());
        let types = manifest.get("types").expect("types");
        let Json::Object(all) = types else {
            panic!("types is an object")
        };
        let Declared {
            consts,
            sizes,
            offsets,
        } = declared();
        let mut wrong = Vec::new();
        let mut enums_checked = 0;
        for (name, value) in &consts {
            let full = format!("GHOSTTY_{name}");
            let found = all.iter().find_map(|(_, t)| {
                let prefix = match t.get("prefix")? {
                    Json::Text(p) => p,
                    _ => return None,
                };
                t.get("values")?
                    .get(full.strip_prefix(prefix.as_str())?)?
                    .number()
            });
            match found {
                Some(v) if v == *value => enums_checked += 1,
                Some(v) => wrong.push(format!("{name}: the library says {v}, sys says {value}")),
                // A macro (`GHOSTTY_MODE_*`, `GHOSTTY_DA_*`, `GHOSTTY_MODS_*`):
                // the manifest lists types, and only the C probe measures these.
                None => {}
            }
        }
        for (name, size) in &sizes {
            match types
                .get(&format!("Ghostty{name}"))
                .and_then(|t| t.get("size")?.number())
            {
                Some(s) if s == *size => {}
                other => wrong.push(format!(
                    "size of {name}: the library says {other:?}, sys says {size}"
                )),
            }
        }
        for (name, field, offset) in &offsets {
            let found = types
                .get(&format!("Ghostty{name}"))
                .and_then(|t| t.get("fields")?.get(field)?.get("offset")?.number());
            if found != Some(*offset) {
                wrong.push(format!(
                    "offset of {name}.{field}: the library says {found:?}, sys says {offset}"
                ));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
        // Floors: a reader that found nothing would pass with nothing wrong.
        assert!(
            enums_checked >= 350,
            "only {enums_checked} enum values checked"
        );
        assert!(sizes.len() >= 45, "only {} sizes", sizes.len());
        assert!(offsets.len() >= 95, "only {} offsets", offsets.len());
    }
}
