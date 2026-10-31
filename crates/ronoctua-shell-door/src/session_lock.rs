//! THE SESSION LOCKED — the one file that names the compositor's lock
//! protocol, and the shape a door takes while it holds one.
//!
//! # Why this protocol, and no other
//!
//! A lock drawn over the screen with layer-shell is a picture, not a lock: the
//! windows under it still take a click the overlay lets through, a keyboard
//! shortcut of the compositor still reaches them, and a footer that dies
//! leaves them all reachable. `ext-session-lock-v1` is the compositor's own
//! word: from the `locked` event on it hides every surface but the lock's,
//! gives the input to the lock's surfaces alone, and keeps the session locked
//! if the process holding the lock dies — a new locker can take it over, and
//! only `unlock_and_destroy` opens it. It is the owner's approved exception
//! to invariant 3 (2026-10-31), a STANDARD Wayland protocol, never a patch,
//! and `one-session-lock` keeps every other file from naming it.
//!
//! # One door, every screen
//!
//! A lock is ONE object, and every surface it shows must be made through it —
//! so the lock is one process, unlike the resizer's curtains. The door's own
//! surface becomes the lock's on the screen that had the focus when the lock
//! was asked (`wanted`, by connector), and draws there like any door: the
//! keyboard, its repeat and compose, every pointer, the scale. Every OTHER
//! screen gets a PLAIN surface of the lock, the chrome's colour and nothing
//! else, painted from shared memory: a screen the lock leaves bare is one the
//! compositor fills with a colour of its own, and invariant 14's exception is
//! that the lock's surfaces cover every screen while the session is locked.
//! A key typed while a plain screen has the keyboard is the door's all the
//! same — the door hears every keyboard — and a pointer on one answers
//! nothing but the arrow.
//!
//! # And the door's screen gets a plain picture first
//!
//! The compositor gives the keyboard to the first lock surface that MAPS. The
//! door's frame comes from the GPU, and a GPU that cannot be had would leave
//! that surface unmapped and the keyboard with nobody: a session locked for
//! good. So the first configure of the door's surface maps it with the same
//! plain picture, and the door's frames replace it. With no GPU at all the
//! person still types the password, on a screen of one colour.
//!
//! # A screen that goes
//!
//! The compositor ends the lock's surface on a screen that is unplugged. A
//! plain one is forgotten; the door's own moves to another screen with a
//! surface of its own, as a column moves (`Door::forget_the_surface`), so
//! the password is never asked on a screen that is not there.
//!
//! # Opening it again
//!
//! [`crate::door::Ui::unlock`] asks; the door sends `unlock_and_destroy` and
//! then waits for the compositor to have READ it before the process goes. A
//! client that hangs up with the request still in its socket is a lock
//! ABANDONED — the compositor reads the hang-up first and paints every screen
//! red — so the round trip is the difference between opening the session and
//! leaving it locked with nobody to open it.

use crate::door::{Content, Door, Shell, Wiring};
use anyhow::{Result, anyhow};
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    dispatch2::Dispatch2,
    error::GlobalError,
    globals::ProvidesBoundGlobal,
    session_lock::{
        SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface,
        SessionLockSurfaceConfigure,
    },
    shm::raw::RawPool,
};
use wayland_client::{
    Connection, QueueHandle,
    protocol::{
        wl_buffer::{self, WlBuffer},
        wl_output::WlOutput,
        wl_shm::{self, WlShm},
        wl_surface::WlSurface,
    },
};

/// THE WORD OF THE DOOR THAT HOLDS THE SESSION'S LOCK — `ronoctua-shell lock`
/// — here, where the footer that starts it reads it without naming the lock's
/// crate (`room::KEEPER_WORD` is the resizer's, for the same reason).
pub const LOCK_WORD: &str = "lock";

/// The user data of the shared memory and its buffers: neither tells this
/// door anything it acts on (the formats are the two every compositor must
/// take, and a plain picture is never drawn twice).
#[derive(Debug, Clone, Copy, Default)]
pub struct Quiet;

impl<S> Dispatch2<WlShm, S> for Quiet {
    fn event(&self, _: &mut S, _: &WlShm, _: wl_shm::Event, _: &Connection, _: &QueueHandle<S>) {}
}

impl<S> Dispatch2<WlBuffer, S> for Quiet {
    fn event(
        &self,
        _: &mut S,
        _: &WlBuffer,
        _: wl_buffer::Event,
        _: &Connection,
        _: &QueueHandle<S>,
    ) {
    }
}

/// The shared memory, as the pool asks for it.
struct Memory(WlShm);

impl ProvidesBoundGlobal<WlShm, 1> for Memory {
    fn bound_global(&self) -> std::result::Result<WlShm, GlobalError> {
        Ok(self.0.clone())
    }
}

/// One surface of the lock, on one screen.
struct Placed {
    /// The screen.
    output: WlOutput,
    /// Its role — dropping it ends the role and destroys the surface.
    role: SessionLockSurface,
    /// The plain picture shown on it, kept while it is.
    plain: Option<(RawPool, WlBuffer)>,
}

/// THE LOCK A DOOR HOLDS: the shape it takes on the screen
/// ([`Shell::Locked`]) while the session is locked.
pub struct Locked {
    /// The global, kept for as long as the lock lives.
    _manager: SessionLockState,
    /// The lock.
    lock: SessionLock,
    /// The door's own surface — the lock's on the screen it is placed on.
    bare: WlSurface,
    /// Where the door's surface is, once its screen is known.
    door_on: Option<Placed>,
    /// Every other screen's plain surface.
    plain: Vec<Placed>,
    /// The screen the door's surface goes to, by connector: the one with the
    /// focus when the lock was asked. `None`, or a name no screen has, is the
    /// first screen.
    wanted: Option<String>,
    /// The shared memory the plain pictures are painted in; `None` when the
    /// compositor offers none (the screens then stay the compositor's colour).
    shm: Option<WlShm>,
    /// The compositor said `locked`.
    locked: bool,
    /// The compositor said `finished`: the lock is over, or never was.
    finished: bool,
}

impl Locked {
    /// The door's surface.
    #[must_use]
    pub const fn surface(&self) -> &WlSurface {
        &self.bare
    }

    /// Whether the compositor has said the session is locked.
    #[must_use]
    pub const fn is_locked(&self) -> bool {
        self.locked && !self.finished
    }
}

/// LOCKS THE SESSION, and gives the door its shape: what a door's
/// [`Content::open`] returns to hold the lock. `surface` is the door's own,
/// placed on `wanted` (a connector's name) once the screens are known.
///
/// # Errors
///
/// The compositor offers no session lock.
pub fn lock<C: Content>(
    wiring: &Wiring<'_, C>,
    surface: WlSurface,
    wanted: Option<String>,
) -> Result<Shell> {
    let manager = SessionLockState::new(wiring.globals, wiring.qh);
    let lock = manager
        .lock(wiring.qh)
        .map_err(|e| anyhow!("the compositor offers no session lock: {e}"))?;
    let shm = wiring
        .globals
        .bind::<WlShm, _, _>(wiring.qh, 1..=1, Quiet)
        .map_err(|e| tracing::warn!(error = %e, "no shared memory; the other screens stay the compositor's colour"))
        .ok();
    Ok(Shell::Locked(Box::new(Locked {
        _manager: manager,
        lock,
        bare: surface,
        door_on: None,
        plain: Vec::new(),
        wanted,
        shm,
        locked: false,
        finished: false,
    })))
}

/// A surface painted all over in `rgba`, `size` logical pixels at scale one —
/// exactly the size the compositor configured, which is what a lock's surface
/// must be committed at. `None`, said in the log, when the memory could not
/// be had: the screen is the compositor's colour, still locked.
fn painted_plain<D>(
    shm: &WlShm,
    surface: &WlSurface,
    size: (u32, u32),
    rgba: [u8; 4],
    qh: &QueueHandle<D>,
) -> Option<(RawPool, WlBuffer)>
where
    D: wayland_client::Dispatch<WlBuffer, Quiet> + 'static,
{
    let (w, h) = (size.0.max(1), size.1.max(1));
    let stride = w.checked_mul(4)?;
    let len = usize::try_from(stride.checked_mul(h)?).ok()?;
    let mut pool = RawPool::new(len, &Memory(shm.clone()))
        .map_err(|e| tracing::warn!(error = %e, "no memory for a locked screen's picture"))
        .ok()?;
    // XRGB8888 IS LITTLE-ENDIAN BGRX in memory, and opaque.
    let [r, g, b, _] = rgba;
    for pixel in pool.mmap().chunks_exact_mut(4) {
        pixel.copy_from_slice(&[b, g, r, 0xff]);
    }
    let (wi, hi, si) = (
        i32::try_from(w).ok()?,
        i32::try_from(h).ok()?,
        i32::try_from(stride).ok()?,
    );
    let buffer = pool.create_buffer(0, wi, hi, si, wl_shm::Format::Xrgb8888, Quiet, qh);
    surface.set_buffer_scale(1);
    surface.attach(Some(&buffer), 0, 0);
    surface.damage_buffer(0, 0, wi, hi);
    surface.commit();
    Some((pool, buffer))
}

impl<C: Content> Door<C> {
    /// The colour of a plain screen: the chrome of the page the door wears.
    fn plain_colour(&self) -> [u8; 4] {
        tokens::theme::set_ground(self.content.page());
        tokens::color::SURFACE_CHROME.to_rgba8()
    }

    /// GIVES EVERY SCREEN ITS SURFACE OF THE LOCK: the door's on the screen it
    /// wants (or the first), a plain one on every other, none on a screen
    /// that went — and the door's moved when ITS screen went. Called after the
    /// screens are told and at every turn; does nothing while nothing moved.
    pub(crate) fn place_the_lock(&mut self) {
        if !matches!(&self.shell, Shell::Locked(l) if !l.finished) {
            return;
        }
        let screens: Vec<(WlOutput, Option<String>)> = self
            .outputs
            .outputs()
            .map(|o| {
                let name = self.outputs.info(&o).and_then(|i| i.name);
                (o, name)
            })
            .collect();
        let Shell::Locked(l) = &mut self.shell else {
            return;
        };
        if l.finished || screens.is_empty() {
            return;
        }
        let here = |o: &WlOutput| screens.iter().any(|(s, _)| s == o);
        l.plain.retain(|p| here(&p.output));
        // THE DOOR'S SCREEN WENT: the door moves to a surface of its own on
        // another — the old one's role was ended by the compositor.
        if l.door_on.as_ref().is_some_and(|d| !here(&d.output)) {
            let fresh = self._compositor.create_surface(&self.qh);
            tracing::info!("the locked screen with the password went; it moves to another");
            let left = self.forget_the_surface(fresh.clone());
            // THE OLD ROLE GOES AFTER THE GPU, which drew through it.
            if let Shell::Locked(l) = &mut self.shell {
                l.door_on = None;
                l.bare = fresh;
            }
            self.surface_taken(left);
        }
        let Shell::Locked(l) = &mut self.shell else {
            return;
        };
        if l.door_on.is_none() {
            let wanted = l.wanted.clone();
            let Some((output, name)) = screens
                .iter()
                .find(|(_, n)| n.is_some() && *n == wanted)
                .or_else(|| screens.first())
            else {
                return;
            };
            // A PLAIN SURFACE ON THAT SCREEN GIVES WAY: a screen has one.
            l.plain.retain(|p| &p.output != output);
            let role = l.lock.create_lock_surface(l.bare.clone(), output, &self.qh);
            tracing::info!(screen = ?name, "the password is asked on this screen");
            l.door_on = Some(Placed {
                output: output.clone(),
                role,
                plain: None,
            });
        }
        let taken: Vec<WlOutput> = l
            .door_on
            .iter()
            .chain(l.plain.iter())
            .map(|p| p.output.clone())
            .collect();
        for (output, _) in &screens {
            if taken.contains(output) {
                continue;
            }
            let surface = self._compositor.create_surface(&self.qh);
            let role = l.lock.create_lock_surface(surface, output, &self.qh);
            l.plain.push(Placed {
                output: output.clone(),
                role,
                plain: None,
            });
        }
    }

    /// OPENS THE SESSION AGAIN and ends the door — after the compositor has
    /// read the request (see the module's last section).
    pub(crate) fn open_the_session(&mut self) {
        if let Shell::Locked(l) = &mut self.shell
            && l.is_locked()
        {
            l.lock.unlock();
            l.finished = true;
            if let Err(e) = self.ui.conn.roundtrip() {
                tracing::error!(error = %e, "the compositor may not have read the unlock");
            } else {
                tracing::info!("the session is open again");
            }
        }
        self.ui.quit = true;
    }
}

impl<C: Content> SessionLockHandler for Door<C> {
    fn locked(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        if let Shell::Locked(l) = &mut self.shell {
            l.locked = true;
        }
        tracing::info!("the session is locked");
        self.content.screens_locked(&mut self.ui);
    }

    /// The lock is over — or never was: another lock holds the session, or
    /// the compositor refused it. The door ends either way; what it was is
    /// the content's to say.
    fn finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        let was = match &mut self.shell {
            Shell::Locked(l) => {
                let was = l.locked;
                l.finished = true;
                was
            }
            _ => false,
        };
        if was {
            tracing::info!("the compositor ended the lock");
        } else {
            tracing::warn!(
                "the compositor did not lock: another lock holds the session, or it refused"
            );
        }
        self.content.lock_ended(&mut self.ui, was);
        self.ui.quit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        _: u32,
    ) {
        let size = configure.new_size;
        let colour = self.plain_colour();
        let Shell::Locked(l) = &mut self.shell else {
            return;
        };
        let shm = l.shm.clone();
        let is_the_doors = l
            .door_on
            .as_ref()
            .is_some_and(|d| d.role.wl_surface() == surface.wl_surface());
        if is_the_doors {
            // MAPPED AT ONCE, with the plain picture, so the keyboard is the
            // lock's whatever the GPU does (the module's second section).
            if let (Some(d), Some(shm)) = (l.door_on.as_mut(), shm.as_ref())
                && d.plain.is_none()
            {
                d.plain = painted_plain(shm, surface.wl_surface(), size, colour, qh);
            }
            self.configured(
                Some(size.0).filter(|w| *w != 0),
                Some(size.1).filter(|h| *h != 0),
            );
            return;
        }
        if let (Some(p), Some(shm)) = (
            l.plain
                .iter_mut()
                .find(|p| p.role.wl_surface() == surface.wl_surface()),
            shm.as_ref(),
        ) {
            p.plain = painted_plain(shm, surface.wl_surface(), size, colour, qh);
        }
    }
}
