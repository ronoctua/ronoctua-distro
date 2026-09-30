//! libmpv, by hand — the one module with `unsafe` in this crate.
//!
//! # Why by hand and not bindgen
//!
//! Twenty functions and four structs are used, and bindgen would bring
//! clang into the build for them. What keeps the hand-written half honest is
//! the same arrangement `ronoctua-camera::v4l2` and the torrent FFI use: the
//! SIZES are asserted at compile time here and measured against the pinned
//! headers by `mpv-layout-matches-c`, and so are the constants.
//!
//! # The threads
//!
//! The client API is thread-safe: a [`Mpv`] is shared between the event
//! thread and whoever asks. A [`Render`] is not — its functions are called
//! from the render thread only, and it holds the [`Mpv`] it was made from so
//! it is freed first, which the API requires.

#![allow(unsafe_code)]

use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    ptr::NonNull,
    sync::{Arc, Condvar, Mutex},
};

use anyhow::{Result, bail};

#[repr(C)]
struct Handle {
    _private: [u8; 0],
}

#[repr(C)]
struct RenderContext {
    _private: [u8; 0],
}

/// `mpv_event`.
#[repr(C)]
struct RawEvent {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

/// `mpv_event_property`.
#[repr(C)]
struct RawProperty {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}

/// `mpv_event_end_file`.
#[repr(C)]
struct RawEndFile {
    reason: c_int,
    error: c_int,
    playlist_entry_id: i64,
    playlist_insert_id: i64,
    playlist_insert_num_entries: c_int,
}

/// `mpv_render_param`.
#[repr(C)]
struct RawRenderParam {
    kind: c_int,
    data: *mut c_void,
}

/// `mpv_stream_cb_info`.
#[repr(C)]
struct RawStreamInfo {
    cookie: *mut c_void,
    read_fn: Option<unsafe extern "C" fn(*mut c_void, *mut c_char, u64) -> i64>,
    seek_fn: Option<unsafe extern "C" fn(*mut c_void, i64) -> i64>,
    size_fn: Option<unsafe extern "C" fn(*mut c_void) -> i64>,
    close_fn: Option<unsafe extern "C" fn(*mut c_void)>,
    // Since API 1.106, and missing here until `mpv-layout-matches-c` measured
    // 48 against the 40 this struct had. It worked by luck: the struct is
    // libmpv's, zeroed before the open callback fills it, so the field we did
    // not know about read as "no cancel".
    cancel_fn: Option<unsafe extern "C" fn(*mut c_void)>,
}

// THE SIZES, as the pinned headers have them — `mpv-layout-matches-c`
// compiles a probe against `mpv/client.h` and compares each number here.
const _: () = assert!(std::mem::size_of::<RawEvent>() == 24);
const _: () = assert!(std::mem::size_of::<RawProperty>() == 24);
const _: () = assert!(std::mem::size_of::<RawEndFile>() == 32);
const _: () = assert!(std::mem::size_of::<RawRenderParam>() == 16);
const _: () = assert!(std::mem::size_of::<RawStreamInfo>() == 48);

/// A change notification with no value.
pub const FORMAT_NONE: c_int = 0;
/// A string.
pub const FORMAT_STRING: c_int = 1;
/// A flag.
pub const FORMAT_FLAG: c_int = 3;
/// An integer.
pub const FORMAT_INT64: c_int = 4;
/// A float.
pub const FORMAT_DOUBLE: c_int = 5;

const EVENT_NONE: c_int = 0;
const EVENT_SHUTDOWN: c_int = 1;
const EVENT_START_FILE: c_int = 6;
const EVENT_END_FILE: c_int = 7;
const EVENT_FILE_LOADED: c_int = 8;
const EVENT_SEEK: c_int = 20;
const EVENT_PLAYBACK_RESTART: c_int = 21;
const EVENT_PROPERTY_CHANGE: c_int = 22;

const END_FILE_EOF: c_int = 0;
const END_FILE_ERROR: c_int = 4;

const RENDER_PARAM_API_TYPE: c_int = 1;
const RENDER_PARAM_BLOCK_FOR_TARGET_TIME: c_int = 12;
const RENDER_PARAM_SW_SIZE: c_int = 17;
const RENDER_PARAM_SW_FORMAT: c_int = 18;
const RENDER_PARAM_SW_STRIDE: c_int = 19;
const RENDER_PARAM_SW_POINTER: c_int = 20;
const RENDER_UPDATE_FRAME: u64 = 1;

// THE C LIBRARY'S `_exit`, the one function of it this crate calls by hand
// — declared here because this is the crate's one module with `unsafe`. See
// [`leave_now`].
unsafe extern "C" {
    fn _exit(status: c_int) -> !;
}

/// Ends the process NOW with `status`, running nothing on the way out: no
/// `atexit` handler and no C++ static destructor, which `std::process::exit`
/// runs while libmpv's threads — its event thread, the render threads, the
/// demuxers — are still alive. A destructor that pulled something from under
/// one of them would crash a player that was asked to leave WITH SUCCESS, and
/// its unit's `Restart=on-failure` would bring it back a second later showing
/// the queue it had just closed. Everything that has to outlive the process
/// — the queue remembered, the socket off the disk, every watcher's farewell
/// — is done before this is called.
pub fn leave_now(status: c_int) -> ! {
    // SAFETY: `_exit` takes any status and never returns; nothing of this
    // process runs after it.
    unsafe { _exit(status) }
}

#[link(name = "mpv")]
unsafe extern "C" {
    fn mpv_create() -> *mut Handle;
    fn mpv_initialize(ctx: *mut Handle) -> c_int;
    fn mpv_terminate_destroy(ctx: *mut Handle);
    fn mpv_set_option_string(ctx: *mut Handle, name: *const c_char, data: *const c_char) -> c_int;
    fn mpv_set_property_string(ctx: *mut Handle, name: *const c_char, data: *const c_char)
    -> c_int;
    fn mpv_get_property_string(ctx: *mut Handle, name: *const c_char) -> *mut c_char;
    fn mpv_get_property(
        ctx: *mut Handle,
        name: *const c_char,
        format: c_int,
        data: *mut c_void,
    ) -> c_int;
    fn mpv_free(data: *mut c_void);
    fn mpv_command(ctx: *mut Handle, args: *mut *const c_char) -> c_int;
    fn mpv_observe_property(ctx: *mut Handle, id: u64, name: *const c_char, format: c_int)
    -> c_int;
    fn mpv_wait_event(ctx: *mut Handle, timeout: f64) -> *mut RawEvent;
    fn mpv_error_string(error: c_int) -> *const c_char;
    fn mpv_stream_cb_add_ro(
        ctx: *mut Handle,
        protocol: *const c_char,
        user_data: *mut c_void,
        open_fn: unsafe extern "C" fn(*mut c_void, *mut c_char, *mut RawStreamInfo) -> c_int,
    ) -> c_int;
    fn mpv_render_context_create(
        res: *mut *mut RenderContext,
        mpv: *mut Handle,
        params: *mut RawRenderParam,
    ) -> c_int;
    fn mpv_render_context_update(ctx: *mut RenderContext) -> u64;
    fn mpv_render_context_render(ctx: *mut RenderContext, params: *mut RawRenderParam) -> c_int;
    fn mpv_render_context_set_update_callback(
        ctx: *mut RenderContext,
        callback: Option<unsafe extern "C" fn(*mut c_void)>,
        data: *mut c_void,
    );
    fn mpv_render_context_free(ctx: *mut RenderContext);
}

fn check(code: c_int, what: &str) -> Result<()> {
    if code >= 0 {
        return Ok(());
    }
    // SAFETY: `mpv_error_string` returns a static string for any code.
    let said = unsafe { CStr::from_ptr(mpv_error_string(code)) }.to_string_lossy();
    bail!("{what}: {said}")
}

fn c(text: &str) -> Result<CString> {
    Ok(CString::new(text)?)
}

/// A property's value, as observed.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// The property has no value now (nothing loaded, no such track).
    None,
    /// A string.
    Text(String),
    /// A flag.
    Flag(bool),
    /// An integer.
    Int(i64),
    /// A float.
    Float(f64),
}

/// One event, owned.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Nothing happened within the wait.
    None,
    /// mpv is going away.
    Shutdown,
    /// A file started loading.
    StartFile,
    /// A file ended: `eof` when it played to the end, `error` when it could
    /// not play, with the reason.
    EndFile {
        /// It reached its end.
        eof: bool,
        /// It failed, and why.
        error: Option<String>,
    },
    /// A file is loaded and playing.
    FileLoaded,
    /// A seek started.
    Seek,
    /// Playback resumed after a seek or a load.
    Restart,
    /// An observed property changed.
    Property {
        /// The id it was observed with.
        id: u64,
        /// Its new value.
        value: Value,
    },
    /// Anything else.
    Other(i32),
}

/// An mpv instance.
pub struct Mpv {
    handle: NonNull<Handle>,
}

// SAFETY: the client API is documented thread-safe: every function taking an
// `mpv_handle` may be called from any thread.
unsafe impl Send for Mpv {}
// SAFETY: as above.
unsafe impl Sync for Mpv {}

impl Mpv {
    /// A new instance with `options` set before it initialises.
    pub fn new(options: &[(&str, &str)]) -> Result<Self> {
        // SAFETY: no arguments; a null return is handled.
        let handle = NonNull::new(unsafe { mpv_create() })
            .ok_or_else(|| anyhow::anyhow!("mpv could not be created"))?;
        let mpv = Self { handle };
        for (name, value) in options {
            let (n, v) = (c(name)?, c(value)?);
            // SAFETY: the handle is live and both strings outlive the call.
            let code = unsafe { mpv_set_option_string(mpv.raw(), n.as_ptr(), v.as_ptr()) };
            check(code, &format!("option {name}"))?;
        }
        // SAFETY: the handle is live and not yet initialised.
        check(unsafe { mpv_initialize(mpv.raw()) }, "initialising mpv")?;
        Ok(mpv)
    }

    fn raw(&self) -> *mut Handle {
        self.handle.as_ptr()
    }

    /// Runs a command, `["loadfile", path, "append"]`.
    pub fn command(&self, args: &[&str]) -> Result<()> {
        let owned: Vec<CString> = args.iter().map(|a| c(a)).collect::<Result<_>>()?;
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|a| a.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        // SAFETY: a null-terminated array of strings that outlive the call.
        check(
            unsafe { mpv_command(self.raw(), ptrs.as_mut_ptr()) },
            &format!("command {}", args.first().copied().unwrap_or_default()),
        )
    }

    /// Sets a property from its string form.
    pub fn set(&self, name: &str, value: &str) -> Result<()> {
        let (n, v) = (c(name)?, c(value)?);
        // SAFETY: live handle, strings outlive the call.
        check(
            unsafe { mpv_set_property_string(self.raw(), n.as_ptr(), v.as_ptr()) },
            &format!("property {name}"),
        )
    }

    /// A property as a string — for a node property, its JSON.
    #[must_use]
    pub fn text(&self, name: &str) -> Option<String> {
        let n = c(name).ok()?;
        // SAFETY: live handle; the result is freed with `mpv_free` below.
        let raw = unsafe { mpv_get_property_string(self.raw(), n.as_ptr()) };
        if raw.is_null() {
            return None;
        }
        // SAFETY: a non-null result is a NUL-terminated string mpv owns.
        let out = unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: returned by `mpv_get_property_string`.
        unsafe { mpv_free(raw.cast()) };
        Some(out)
    }

    /// A property as a float.
    #[must_use]
    pub fn float(&self, name: &str) -> Option<f64> {
        let n = c(name).ok()?;
        let mut out = 0.0f64;
        // SAFETY: live handle; `out` is a `double`, the format asked for.
        let code = unsafe {
            mpv_get_property(self.raw(), n.as_ptr(), FORMAT_DOUBLE, (&raw mut out).cast())
        };
        (code >= 0).then_some(out)
    }

    /// Asks for `name` to be reported under `id` whenever it changes.
    pub fn observe(&self, id: u64, name: &str, format: c_int) -> Result<()> {
        let n = c(name)?;
        // SAFETY: live handle, string outlives the call.
        check(
            unsafe { mpv_observe_property(self.raw(), id, n.as_ptr(), format) },
            &format!("observing {name}"),
        )
    }

    /// Waits up to `timeout` seconds for the next event.
    #[must_use]
    pub fn wait(&self, timeout: f64) -> Event {
        // SAFETY: live handle; the event is valid until the next call on this
        // handle's event queue, which only this thread makes, and everything
        // is copied out before returning.
        let raw = unsafe { &*mpv_wait_event(self.raw(), timeout) };
        match raw.event_id {
            EVENT_NONE => Event::None,
            EVENT_SHUTDOWN => Event::Shutdown,
            EVENT_START_FILE => Event::StartFile,
            EVENT_FILE_LOADED => Event::FileLoaded,
            EVENT_SEEK => Event::Seek,
            EVENT_PLAYBACK_RESTART => Event::Restart,
            EVENT_END_FILE => {
                // SAFETY: an END_FILE event's data is an `mpv_event_end_file`.
                let end = unsafe { &*raw.data.cast::<RawEndFile>() };
                let error = (end.reason == END_FILE_ERROR).then(|| {
                    // SAFETY: static string for any code.
                    unsafe { CStr::from_ptr(mpv_error_string(end.error)) }
                        .to_string_lossy()
                        .into_owned()
                });
                Event::EndFile {
                    eof: end.reason == END_FILE_EOF,
                    error,
                }
            }
            EVENT_PROPERTY_CHANGE => {
                // SAFETY: a PROPERTY_CHANGE event's data is an
                // `mpv_event_property`, and `data` points at the format named.
                let p = unsafe { &*raw.data.cast::<RawProperty>() };
                let value = unsafe {
                    match p.format {
                        FORMAT_STRING if !p.data.is_null() => {
                            let s = *p.data.cast::<*const c_char>();
                            if s.is_null() {
                                Value::None
                            } else {
                                Value::Text(CStr::from_ptr(s).to_string_lossy().into_owned())
                            }
                        }
                        FORMAT_FLAG if !p.data.is_null() => {
                            Value::Flag(*p.data.cast::<c_int>() != 0)
                        }
                        FORMAT_INT64 if !p.data.is_null() => Value::Int(*p.data.cast::<i64>()),
                        FORMAT_DOUBLE if !p.data.is_null() => Value::Float(*p.data.cast::<f64>()),
                        _ => Value::None,
                    }
                };
                Event::Property {
                    id: raw.reply_userdata,
                    value,
                }
            }
            other => Event::Other(other),
        }
    }

    /// Registers `protocol://` as a read-only stream this process serves:
    /// `open` is called with the URI's text and answers a reader.
    ///
    /// THE OPENER LIVES AS LONG AS THE PROCESS — `'static`, one for every
    /// handle: mpv keeps the pointer for the life of the handle, and one
    /// leaked per handle was one leaked per voice.
    pub fn streams(&self, protocol: &str, open: &'static Opener) -> Result<()> {
        let p = c(protocol)?;
        // SAFETY: live handle; `open` is 'static and handed back to
        // `open_stream` as the `user_data` it was given.
        check(
            unsafe {
                mpv_stream_cb_add_ro(
                    self.raw(),
                    p.as_ptr(),
                    std::ptr::from_ref(open).cast_mut().cast(),
                    open_stream,
                )
            },
            "registering a stream protocol",
        )
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // SAFETY: live handle, destroyed exactly once.
        unsafe { mpv_terminate_destroy(self.raw()) };
    }
}

/// What a stream protocol answers when mpv opens one of its URIs.
pub type Opener = Box<dyn Fn(&str) -> Option<Box<dyn Stream>> + Send + Sync>;

/// What a protocol of ours hands mpv: bytes, and a way to move among them.
pub trait Stream: std::io::Read + Send {
    /// Moves to byte `to` and answers where the stream now is — short of `to`
    /// only when it ended first.
    ///
    /// # Errors
    ///
    /// The stream cannot get there at all.
    fn seek_to(&mut self, to: u64) -> std::io::Result<u64>;
}

unsafe extern "C" fn open_stream(
    user: *mut c_void,
    uri: *mut c_char,
    info: *mut RawStreamInfo,
) -> c_int {
    // SAFETY: `user` is the leaked `Opener` registered in `streams`, and
    // `uri` is a NUL-terminated string mpv owns for this call.
    let (open, uri) = unsafe {
        (
            &*user.cast::<Opener>(),
            CStr::from_ptr(uri).to_string_lossy(),
        )
    };
    let Some(reader) = open(&uri) else {
        return -13; // MPV_ERROR_LOADING_FAILED
    };
    let cookie = Box::into_raw(Box::new(reader));
    // SAFETY: `info` is the struct mpv asks us to fill.
    unsafe {
        (*info).cookie = cookie.cast();
        (*info).read_fn = Some(read_stream);
        (*info).seek_fn = Some(seek_stream);
        (*info).size_fn = None;
        (*info).close_fn = Some(close_stream);
        (*info).cancel_fn = None;
    }
    0
}

unsafe extern "C" fn read_stream(cookie: *mut c_void, buf: *mut c_char, n: u64) -> i64 {
    // SAFETY: the cookie is the boxed reader `open_stream` made; `buf` has `n`
    // writable bytes for this call.
    let (reader, out) = unsafe {
        (
            &mut *cookie.cast::<Box<dyn Stream>>(),
            std::slice::from_raw_parts_mut(buf.cast::<u8>(), usize::try_from(n).unwrap_or(0)),
        )
    };
    loop {
        match reader.read(out) {
            Ok(got) => return i64::try_from(got).unwrap_or(-1),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return -1,
        }
    }
}

unsafe extern "C" fn seek_stream(cookie: *mut c_void, offset: i64) -> i64 {
    // SAFETY: the cookie is the boxed stream `open_stream` made.
    let stream = unsafe { &mut *cookie.cast::<Box<dyn Stream>>() };
    let Ok(to) = u64::try_from(offset) else {
        return -1;
    };
    stream
        .seek_to(to)
        .ok()
        .and_then(|at| i64::try_from(at).ok())
        // ANY NEGATIVE is a failed seek to mpv.
        .unwrap_or(-1)
}

unsafe extern "C" fn close_stream(cookie: *mut c_void) {
    // SAFETY: the cookie is the boxed reader, freed exactly once here.
    drop(unsafe { Box::from_raw(cookie.cast::<Box<dyn Stream>>()) });
}

/// The software renderer — frames into memory, at whatever size is asked.
pub struct Render {
    context: NonNull<RenderContext>,
    /// Set by mpv's update callback, taken by [`Render::wait`]. Shared, so
    /// the engine can ring it too ([`Poke`]).
    bell: Arc<(Mutex<bool>, Condvar)>,
    _mpv: Arc<Mpv>,
}

/// Wakes a render thread that sleeps in [`Render::wait`]: a new viewer is
/// owed the frame on screen now, and a paused picture sends no new one.
#[derive(Clone)]
pub struct Poke(Arc<(Mutex<bool>, Condvar)>);

impl Poke {
    /// Rings the bell.
    pub fn ring(&self) {
        if let Ok(mut rung) = self.0.0.lock() {
            *rung = true;
        }
        self.0.1.notify_all();
    }
}

// SAFETY: the render API may be used from any ONE thread at a time; a
// `Render` is moved into the render thread and used only there.
unsafe impl Send for Render {}

unsafe extern "C" fn rang(data: *mut c_void) {
    // SAFETY: `data` is the `bell` box, which outlives the context.
    let bell = unsafe { &*data.cast::<(Mutex<bool>, Condvar)>() };
    if let Ok(mut rung) = bell.0.lock() {
        *rung = true;
    }
    bell.1.notify_all();
}

impl Render {
    /// A software render context over `mpv`.
    pub fn new(mpv: Arc<Mpv>) -> Result<Self> {
        let api = c("sw")?;
        let mut params = [
            RawRenderParam {
                kind: RENDER_PARAM_API_TYPE,
                data: api.as_ptr().cast_mut().cast(),
            },
            RawRenderParam {
                kind: 0,
                data: std::ptr::null_mut(),
            },
        ];
        let mut context = std::ptr::null_mut();
        // SAFETY: live handle, a zero-terminated parameter list.
        check(
            unsafe { mpv_render_context_create(&raw mut context, mpv.raw(), params.as_mut_ptr()) },
            "creating the renderer",
        )?;
        let context = NonNull::new(context).ok_or_else(|| anyhow::anyhow!("no render context"))?;
        let bell = Arc::new((Mutex::new(false), Condvar::new()));
        // SAFETY: the `Render` holds the bell for as long as the context lives
        // (the context is freed first, in Drop), so the pointer stays valid.
        unsafe {
            mpv_render_context_set_update_callback(
                context.as_ptr(),
                Some(rang),
                Arc::as_ptr(&bell).cast_mut().cast(),
            );
        }
        Ok(Self {
            context,
            bell,
            _mpv: mpv,
        })
    }

    /// A handle that wakes [`Render::wait`] from another thread.
    #[must_use]
    pub fn poke(&self) -> Poke {
        Poke(self.bell.clone())
    }

    /// Waits up to `timeout` for mpv to say something changed, and says
    /// whether a new FRAME is due.
    pub fn wait(&self, timeout: std::time::Duration) -> bool {
        let (lock, cv) = &*self.bell;
        if let Ok(guard) = lock.lock()
            && let Ok((mut rung, _)) = cv.wait_timeout_while(guard, timeout, |r| !*r)
        {
            *rung = false;
        }
        // SAFETY: live context, used from this thread only.
        let flags = unsafe { mpv_render_context_update(self.context.as_ptr()) };
        flags & RENDER_UPDATE_FRAME != 0
    }

    /// Renders the current frame into `into`: `width × height` RGBA, opaque
    /// and tightly packed. `width` must be a multiple of 16 so a row is 64 bytes aligned.
    pub fn render(&self, width: u32, height: u32, into: &mut Aligned) -> Result<()> {
        if !width.is_multiple_of(16) {
            bail!("a frame {width} wide is not a multiple of 16");
        }
        let mut size = [c_int::try_from(width)?, c_int::try_from(height)?];
        let mut stride = width as usize * 4;
        let mut block: c_int = 0;
        let format = c("rgb0")?;
        let buf = into.frame(width as usize * height as usize * 4);
        let mut params = [
            RawRenderParam {
                kind: RENDER_PARAM_SW_SIZE,
                data: size.as_mut_ptr().cast(),
            },
            RawRenderParam {
                kind: RENDER_PARAM_SW_FORMAT,
                data: format.as_ptr().cast_mut().cast(),
            },
            RawRenderParam {
                kind: RENDER_PARAM_SW_STRIDE,
                data: (&raw mut stride).cast(),
            },
            RawRenderParam {
                kind: RENDER_PARAM_SW_POINTER,
                data: buf.as_mut_ptr().cast(),
            },
            // NEVER WAIT FOR THE FRAME'S TIME: measured, the default blocks
            // the call until the frame is due, so it cost 31 ms "per frame"
            // at any size. The render thread paces itself on `wait`.
            RawRenderParam {
                kind: RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
                data: (&raw mut block).cast(),
            },
            RawRenderParam {
                kind: 0,
                data: std::ptr::null_mut(),
            },
        ];
        // SAFETY: live context; every parameter points at storage that
        // outlives the call, and `buf` holds `stride × height` bytes.
        check(
            unsafe { mpv_render_context_render(self.context.as_ptr(), params.as_mut_ptr()) },
            "rendering a frame",
        )?;
        // THE FOURTH BYTE IS PADDING, and the ring promises RGBA. `rgb0`
        // leaves it at zero, which a renderer takes as fully transparent:
        // measured, every frame arrived in the column and not one pixel of it
        // was seen. `rgba` is not a format the software renderer offers.
        for px in buf.as_chunks_mut::<4>().0 {
            px[3] = u8::MAX;
        }
        Ok(())
    }
}

impl Drop for Render {
    fn drop(&mut self) {
        // SAFETY: live context, freed once, before the `Mpv` it holds.
        unsafe { mpv_render_context_free(self.context.as_ptr()) };
    }
}

/// A frame buffer whose start is 64-byte aligned, which the software renderer
/// wants for its SIMD paths (and silently copies the whole frame without).
#[derive(Default)]
pub struct Aligned {
    raw: Vec<u8>,
}

impl Aligned {
    /// `len` bytes, aligned, reused between frames.
    pub fn frame(&mut self, len: usize) -> &mut [u8] {
        if self.raw.len() < len + 64 {
            self.raw.resize(len + 64, 0);
        }
        let off = self.raw.as_ptr().align_offset(64);
        &mut self.raw[off..off + len]
    }

    /// The frame last rendered, `len` bytes.
    #[must_use]
    pub fn get(&self, len: usize) -> &[u8] {
        let off = self.raw.as_ptr().align_offset(64);
        &self.raw[off..off + len.min(self.raw.len().saturating_sub(off))]
    }
}
