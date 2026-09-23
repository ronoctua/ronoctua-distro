//! Input devices and seats — the pointer half of the contract.
//!
//! A SEAT is a set of input devices that act together: one cursor, one
//! keyboard focus. The compositor draws one cursor per seat, and every seat
//! moves its own focus — which is what makes several cursors possible at
//! all. Everything here is OURS: the adapter maps the compositor's own
//! spelling onto it, and no other crate sees that spelling.
//!
//! # A setting is present only where the device has it
//!
//! Every field of [`PointerSettings`] is an `Option`, and `None` means "this
//! device does not have this" — a mouse has no tapping, a touchpad has no
//! scroll button. The compositor reports only what libinput says the device
//! supports, so the same type answers both "what can it do" and "what is it
//! doing", and a screen that shows only the `Some`s shows exactly the
//! settings that exist.

use serde::{Deserialize, Serialize};

/// What kind of thing a device is, as far as a person cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DeviceKind {
    /// Something that moves a cursor and is not a touchpad: a mouse, a
    /// trackball, a trackpoint.
    Mouse,
    /// A touchpad.
    Touchpad,
    /// A keyboard. Keyboards stay on the first seat; they are listed so a
    /// screen can say where they are.
    Keyboard,
    /// Touch screens, tablets, switches — not pointers this crate configures.
    Other,
}

impl DeviceKind {
    /// Whether it moves a cursor.
    #[must_use]
    pub const fn points(self) -> bool {
        matches!(self, Self::Mouse | Self::Touchpad)
    }
}

/// One input device.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    /// The compositor's identifier — `1133:50498:Logitech_Wireless_Receiver_Mouse`.
    /// It is `vendor:product:name`, so it survives a replug and is shared by
    /// two units of the same model: that is the compositor's key, not ours.
    pub id: String,
    /// The name the device gives itself.
    pub name: String,
    /// What it is.
    pub kind: DeviceKind,
    /// What it supports, and what each setting is right now.
    pub settings: PointerSettings,
}

/// A seat: one cursor, one focus, and the devices that drive them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seat {
    /// Its name — `seat0` is the compositor's own.
    pub name: String,
    /// The identifiers of the devices attached to it.
    pub devices: Vec<String>,
    /// The window this seat has focused, when it has one.
    pub focus: Option<u64>,
}

/// Whether a device sends events at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SendEvents {
    /// It works.
    Enabled,
    /// It is switched off.
    Disabled,
    /// Off while a mouse is connected — a touchpad's setting.
    DisabledOnExternalMouse,
}

/// How pointer speed grows with hand speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccelProfile {
    /// The pointer moves in proportion to the hand.
    Flat,
    /// A fast gesture carries further.
    Adaptive,
}

/// Which button one, two and three fingers mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ButtonMap {
    /// Left, right, middle.
    Lrm,
    /// Left, middle, right.
    Lmr,
}

/// How a touchpad's physical click picks a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClickMethod {
    /// No software buttons.
    None,
    /// By where the finger presses: the bottom right corner is the right
    /// button.
    ButtonAreas,
    /// By how many fingers press.
    Clickfinger,
}

/// How a device scrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScrollMethod {
    /// It does not.
    None,
    /// Two fingers moving together.
    TwoFinger,
    /// One finger along the edge.
    Edge,
    /// Moving the device while a button is held.
    OnButtonDown,
}

/// Dragging with fingers alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FingerDrag {
    /// Off.
    Disabled,
    /// Three fingers drag.
    ThreeFinger,
    /// Four fingers drag.
    FourFinger,
}

/// Whether a tap-and-drag survives lifting the finger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DragLock {
    /// The drag ends when the finger lifts.
    Disabled,
    /// It survives a short lift.
    Enabled,
    /// It lasts until the next tap.
    Sticky,
}

/// Every pointer setting a device can have. `None` is "not this device".
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PointerSettings {
    /// Whether it works at all.
    pub send_events: Option<SendEvents>,
    /// Speed, from -1 (slowest) to 1 (fastest).
    pub accel_speed: Option<f64>,
    /// How speed grows with hand speed.
    pub accel_profile: Option<AccelProfile>,
    /// Content follows the fingers, as on a phone.
    pub natural_scroll: Option<bool>,
    /// The buttons swapped for the left hand.
    pub left_handed: Option<bool>,
    /// A tap is a click.
    pub tap: Option<bool>,
    /// Which button a two- and three-finger tap is.
    pub tap_button_map: Option<ButtonMap>,
    /// Tap and then move drags.
    pub tap_drag: Option<bool>,
    /// Whether a tap-drag survives lifting the finger.
    pub tap_drag_lock: Option<DragLock>,
    /// Dragging with fingers alone.
    pub finger_drag: Option<FingerDrag>,
    /// How a physical click picks a button.
    pub click_method: Option<ClickMethod>,
    /// Which button two and three fingers clicking are.
    pub clickfinger_button_map: Option<ButtonMap>,
    /// Pressing left and right together is the middle button.
    pub middle_emulation: Option<bool>,
    /// How it scrolls.
    pub scroll_method: Option<ScrollMethod>,
    /// The button that scrolls, under [`ScrollMethod::OnButtonDown`] — a
    /// Linux event code.
    pub scroll_button: Option<u32>,
    /// The scroll button toggles instead of being held.
    pub scroll_button_lock: Option<bool>,
    /// Off while typing.
    pub disable_while_typing: Option<bool>,
    /// Off while the trackpoint is in use.
    pub disable_while_trackpointing: Option<bool>,
    /// How far one notch of the wheel scrolls, as a multiplier.
    pub scroll_factor: Option<f64>,
}

/// One setting, with its value — what can be asked of a device.
///
/// One variant per field of [`PointerSettings`], so asking for one change is
/// a value and not a struct with nineteen `None`s in it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "setting", content = "value")]
pub enum Setting {
    /// See [`PointerSettings::send_events`].
    SendEvents(SendEvents),
    /// See [`PointerSettings::accel_speed`].
    AccelSpeed(f64),
    /// See [`PointerSettings::accel_profile`].
    AccelProfile(AccelProfile),
    /// See [`PointerSettings::natural_scroll`].
    NaturalScroll(bool),
    /// See [`PointerSettings::left_handed`].
    LeftHanded(bool),
    /// See [`PointerSettings::tap`].
    Tap(bool),
    /// See [`PointerSettings::tap_button_map`].
    TapButtonMap(ButtonMap),
    /// See [`PointerSettings::tap_drag`].
    TapDrag(bool),
    /// See [`PointerSettings::tap_drag_lock`].
    TapDragLock(DragLock),
    /// See [`PointerSettings::finger_drag`].
    FingerDrag(FingerDrag),
    /// See [`PointerSettings::click_method`].
    ClickMethod(ClickMethod),
    /// See [`PointerSettings::clickfinger_button_map`].
    ClickfingerButtonMap(ButtonMap),
    /// See [`PointerSettings::middle_emulation`].
    MiddleEmulation(bool),
    /// See [`PointerSettings::scroll_method`].
    ScrollMethod(ScrollMethod),
    /// See [`PointerSettings::scroll_button`].
    ScrollButton(u32),
    /// See [`PointerSettings::scroll_button_lock`].
    ScrollButtonLock(bool),
    /// See [`PointerSettings::disable_while_typing`].
    DisableWhileTyping(bool),
    /// See [`PointerSettings::disable_while_trackpointing`].
    DisableWhileTrackpointing(bool),
    /// See [`PointerSettings::scroll_factor`].
    ScrollFactor(f64),
}

impl PointerSettings {
    /// Every setting that is present, in the order a screen lists them.
    #[must_use]
    pub fn present(&self) -> Vec<Setting> {
        let mut out = Vec::new();
        let mut put = |s: Option<Setting>| out.extend(s);
        put(self.send_events.map(Setting::SendEvents));
        put(self.accel_speed.map(Setting::AccelSpeed));
        put(self.accel_profile.map(Setting::AccelProfile));
        put(self.natural_scroll.map(Setting::NaturalScroll));
        put(self.left_handed.map(Setting::LeftHanded));
        put(self.scroll_factor.map(Setting::ScrollFactor));
        put(self.tap.map(Setting::Tap));
        put(self.tap_button_map.map(Setting::TapButtonMap));
        put(self.tap_drag.map(Setting::TapDrag));
        put(self.tap_drag_lock.map(Setting::TapDragLock));
        put(self.finger_drag.map(Setting::FingerDrag));
        put(self.click_method.map(Setting::ClickMethod));
        put(self
            .clickfinger_button_map
            .map(Setting::ClickfingerButtonMap));
        put(self.middle_emulation.map(Setting::MiddleEmulation));
        put(self.scroll_method.map(Setting::ScrollMethod));
        put(self.scroll_button.map(Setting::ScrollButton));
        put(self.scroll_button_lock.map(Setting::ScrollButtonLock));
        put(self.disable_while_typing.map(Setting::DisableWhileTyping));
        put(self
            .disable_while_trackpointing
            .map(Setting::DisableWhileTrackpointing));
        out
    }

    /// Writes one setting in.
    pub fn set(&mut self, s: Setting) {
        match s {
            Setting::SendEvents(v) => self.send_events = Some(v),
            Setting::AccelSpeed(v) => self.accel_speed = Some(v.clamp(-1.0, 1.0)),
            Setting::AccelProfile(v) => self.accel_profile = Some(v),
            Setting::NaturalScroll(v) => self.natural_scroll = Some(v),
            Setting::LeftHanded(v) => self.left_handed = Some(v),
            Setting::Tap(v) => self.tap = Some(v),
            Setting::TapButtonMap(v) => self.tap_button_map = Some(v),
            Setting::TapDrag(v) => self.tap_drag = Some(v),
            Setting::TapDragLock(v) => self.tap_drag_lock = Some(v),
            Setting::FingerDrag(v) => self.finger_drag = Some(v),
            Setting::ClickMethod(v) => self.click_method = Some(v),
            Setting::ClickfingerButtonMap(v) => self.clickfinger_button_map = Some(v),
            Setting::MiddleEmulation(v) => self.middle_emulation = Some(v),
            Setting::ScrollMethod(v) => self.scroll_method = Some(v),
            Setting::ScrollButton(v) => self.scroll_button = Some(v),
            Setting::ScrollButtonLock(v) => self.scroll_button_lock = Some(v),
            Setting::DisableWhileTyping(v) => self.disable_while_typing = Some(v),
            Setting::DisableWhileTrackpointing(v) => {
                self.disable_while_trackpointing = Some(v);
            }
            Setting::ScrollFactor(v) => self.scroll_factor = Some(v.clamp(0.1, 10.0)),
        }
    }

    /// Whether this device HAS the setting `s` is about — the question a
    /// screen asks before drawing a line for it.
    #[must_use]
    pub fn has(&self, s: Setting) -> bool {
        self.present()
            .iter()
            .any(|p| std::mem::discriminant(p) == std::mem::discriminant(&s))
    }

    /// `over` written on top of `self`, field by field: what a device ends up
    /// with when a default for its kind and its own choice are both known.
    #[must_use]
    pub fn under(mut self, over: &Self) -> Self {
        for s in over.present() {
            self.set(s);
        }
        self
    }

    /// Only the fields `like` has — so a default written for every touchpad
    /// asks a touchpad without an edge for nothing about edges.
    #[must_use]
    pub fn only_where(&self, like: &Self) -> Self {
        let mut out = Self::default();
        for s in self.present() {
            if like.has(s) {
                out.set(s);
            }
        }
        out
    }
}

/// Something one may ask of a seat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeatAsk {
    /// Attach a device to it — which takes the device OFF every other seat,
    /// and CREATES the seat if it did not exist.
    Attach(String),
    /// Draw its cursor from this theme, at this size.
    Cursor {
        /// The Xcursor theme's name.
        theme: String,
        /// Its size in pixels.
        size: u16,
    },
    /// Hide the cursor after this many milliseconds still, or `0` for never.
    HideAfter(u32),
    /// Hide the cursor while a key is being typed.
    HideWhileTyping(bool),
}

/// Whether focus follows the pointer.
///
/// ONE setting for every seat: the compositor has no per-seat version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FocusFollows {
    /// A click focuses.
    No,
    /// Entering a window focuses it.
    #[default]
    Yes,
    /// Entering focuses, even within the same window's decorations.
    Always,
}

/// One keyboard setting of one device — what gives an independent keyboard
/// its own layout and repeat.
///
/// Apart from [`Setting`] because those are the POINTER's and all `Copy`;
/// these carry text. A device's block takes the WHOLE of its xkb settings,
/// never only the one that differs (see the adapter's `keyboard_command`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "setting", content = "value")]
pub enum KeyboardSetting {
    /// The xkb layout — `gb`.
    Layout(String),
    /// The xkb variant; empty is the layout's default.
    Variant(String),
    /// The xkb options, comma-separated.
    Options(String),
    /// Characters a second a held key repeats at.
    RepeatRate(u16),
    /// Milliseconds before a held key repeats.
    RepeatDelay(u16),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touchpad() -> PointerSettings {
        PointerSettings {
            send_events: Some(SendEvents::Enabled),
            accel_speed: Some(0.0),
            tap: Some(false),
            natural_scroll: Some(false),
            scroll_method: Some(ScrollMethod::TwoFinger),
            ..PointerSettings::default()
        }
    }

    #[test]
    fn a_setting_goes_in_and_comes_back_out() {
        let mut s = PointerSettings::default();
        s.set(Setting::Tap(true));
        s.set(Setting::AccelSpeed(4.0));
        assert_eq!(s.tap, Some(true));
        assert_eq!(
            s.accel_speed,
            Some(1.0),
            "the speed is clamped to the range"
        );
        assert_eq!(
            s.present(),
            vec![Setting::AccelSpeed(1.0), Setting::Tap(true)]
        );
    }

    #[test]
    fn a_default_asks_only_what_the_device_has() {
        // A default for every touchpad names an edge scroll a touchpad
        // without one cannot do — it must not be asked.
        let for_all = PointerSettings {
            tap: Some(true),
            scroll_button: Some(274),
            ..PointerSettings::default()
        };
        let here = for_all.only_where(&touchpad());
        assert_eq!(here.tap, Some(true));
        assert_eq!(here.scroll_button, None);
    }

    #[test]
    fn the_device_s_own_choice_wins_over_the_default() {
        let default = PointerSettings {
            tap: Some(true),
            natural_scroll: Some(true),
            ..PointerSettings::default()
        };
        let own = PointerSettings {
            tap: Some(false),
            ..PointerSettings::default()
        };
        let both = default.under(&own);
        assert_eq!(both.tap, Some(false));
        assert_eq!(both.natural_scroll, Some(true));
    }

    #[test]
    fn a_setting_round_trips_through_the_file() {
        for s in [
            Setting::ClickMethod(ClickMethod::Clickfinger),
            Setting::TapDragLock(DragLock::Sticky),
            Setting::ScrollFactor(1.5),
        ] {
            let text = serde_json::to_string(&s).unwrap();
            assert_eq!(serde_json::from_str::<Setting>(&text).unwrap(), s);
        }
    }
}
