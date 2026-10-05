//! libpam, by hand — the one module with `unsafe` in this crate.
//!
//! # Why by hand and not bindgen
//!
//! Five functions, three structs and a handful of numbers are used, and
//! bindgen would bring clang into the build for them. What keeps the
//! hand-written half honest is the arrangement `ronoctua-camera::v4l2` and the
//! player's libmpv use: every SIZE and every field's OFFSET is asserted at
//! compile time here, and every size, offset and constant is measured against
//! the pinned `<security/pam_appl.h>` by `pam-layout-matches-c`. A wrong size
//! would be libpam reading a response array with the wrong stride, and a wrong
//! offset a pointer read where a number is — the secret handed to the wrong
//! prompt, or a pointer read out of the middle of another — with nothing said
//! anywhere.
//!
//! # The conversation
//!
//! PAM asks for the password through a callback ([`converse`]): an array of
//! messages in, an array of responses out. The responses are allocated with
//! the C library's `calloc` and `malloc` because LIBPAM FREES THEM, with its
//! own `free` — a Rust allocation handed over would be freed by the wrong
//! allocator. The secret is copied into its `malloc`'d response at the moment
//! PAM asks, and nowhere else; when the conversation cannot answer a message,
//! every copy it already made is zeroed before it is freed ([`drop_replies`]).
//!
//! This is Linux-PAM's convention for the message array: `msg[i]` is a
//! POINTER to the i-th message (Solaris reads `(*msg)[i]` instead). With one
//! message the two agree; the stack this crate talks to asks one at a time.

#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_int, c_void};
use std::mem::{offset_of, size_of};

use zeroize::Zeroize as _;

/// `pam_handle_t`: opaque.
#[repr(C)]
pub(crate) struct PamHandle {
    _private: [u8; 0],
}

/// `struct pam_message`.
#[repr(C)]
pub(crate) struct PamMessage {
    /// One of the `PAM_PROMPT_*`, `PAM_ERROR_MSG`, `PAM_TEXT_INFO`.
    pub(crate) msg_style: c_int,
    /// The prompt or the words, NUL-terminated.
    pub(crate) msg: *const c_char,
}

/// `struct pam_response`.
#[repr(C)]
pub(crate) struct PamResponse {
    /// The answer, `malloc`'d; libpam frees it.
    pub(crate) resp: *mut c_char,
    /// Unused by Linux-PAM; zero.
    pub(crate) resp_retcode: c_int,
}

/// The conversation function's type.
pub(crate) type Conversation = unsafe extern "C" fn(
    num_msg: c_int,
    msg: *const *const PamMessage,
    resp: *mut *mut PamResponse,
    appdata_ptr: *mut c_void,
) -> c_int;

/// `struct pam_conv`.
#[repr(C)]
pub(crate) struct PamConv {
    /// The callback.
    pub(crate) conv: Option<Conversation>,
    /// What the callback is handed back.
    pub(crate) appdata_ptr: *mut c_void,
}

// THE SIZES, asserted here and measured against the headers by
// `pam-layout-matches-c` (a number in a comment would be prose, not a promise).
const _: () = assert!(size_of::<PamMessage>() == 16);
const _: () = assert!(size_of::<PamResponse>() == 16);
const _: () = assert!(size_of::<PamConv>() == 16);

// AND WHERE EACH FIELD SITS, measured the same way. The size alone cannot
// tell: each of the three is 16 bytes in EITHER order of its two fields, so a
// swapped `resp` and `resp_retcode` keeps every size and hands libpam the
// secret's pointer at the wrong offset.
const _: () = assert!(offset_of!(PamMessage, msg_style) == 0);
const _: () = assert!(offset_of!(PamMessage, msg) == 8);
const _: () = assert!(offset_of!(PamResponse, resp) == 0);
const _: () = assert!(offset_of!(PamResponse, resp_retcode) == 8);
const _: () = assert!(offset_of!(PamConv, conv) == 0);
const _: () = assert!(offset_of!(PamConv, appdata_ptr) == 8);

// THE NUMBERS, each measured by the same check.
pub(crate) const PAM_SUCCESS: c_int = 0;
pub(crate) const PAM_OPEN_ERR: c_int = 1;
pub(crate) const PAM_SYMBOL_ERR: c_int = 2;
pub(crate) const PAM_SERVICE_ERR: c_int = 3;
pub(crate) const PAM_SYSTEM_ERR: c_int = 4;
pub(crate) const PAM_BUF_ERR: c_int = 5;
pub(crate) const PAM_PERM_DENIED: c_int = 6;
pub(crate) const PAM_AUTH_ERR: c_int = 7;
pub(crate) const PAM_CRED_INSUFFICIENT: c_int = 8;
pub(crate) const PAM_AUTHINFO_UNAVAIL: c_int = 9;
pub(crate) const PAM_USER_UNKNOWN: c_int = 10;
pub(crate) const PAM_MAXTRIES: c_int = 11;
pub(crate) const PAM_NEW_AUTHTOK_REQD: c_int = 12;
pub(crate) const PAM_ACCT_EXPIRED: c_int = 13;
pub(crate) const PAM_CONV_ERR: c_int = 19;
pub(crate) const PAM_ABORT: c_int = 26;
pub(crate) const PAM_PROMPT_ECHO_OFF: c_int = 1;
pub(crate) const PAM_PROMPT_ECHO_ON: c_int = 2;
pub(crate) const PAM_ERROR_MSG: c_int = 3;
pub(crate) const PAM_TEXT_INFO: c_int = 4;
pub(crate) const PAM_SILENT: c_int = 0x8000;
pub(crate) const PAM_DISALLOW_NULL_AUTHTOK: c_int = 1;
pub(crate) const PAM_MAX_NUM_MSG: c_int = 32;

#[link(name = "pam")]
unsafe extern "C" {
    fn pam_start(
        service_name: *const c_char,
        user: *const c_char,
        pam_conversation: *const PamConv,
        pamh: *mut *mut PamHandle,
    ) -> c_int;
    fn pam_authenticate(pamh: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_acct_mgmt(pamh: *mut PamHandle, flags: c_int) -> c_int;
    fn pam_end(pamh: *mut PamHandle, pam_status: c_int) -> c_int;
    fn pam_strerror(pamh: *mut PamHandle, errnum: c_int) -> *const c_char;
}

// THE C LIBRARY'S ALLOCATOR, because libpam frees what the conversation
// returns with it.
unsafe extern "C" {
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn malloc(size: usize) -> *mut c_void;
    fn free(pointer: *mut c_void);
}

/// What the conversation answers with, and what PAM told it on the way.
pub(crate) struct Answering<'a> {
    /// The secret's bytes, without a NUL (the caller refused one inside).
    pub(crate) secret: &'a [u8],
    /// The `PAM_ERROR_MSG` and `PAM_TEXT_INFO` lines, for a log.
    pub(crate) told: Vec<String>,
}

/// A `malloc`'d, NUL-terminated copy of `bytes`; null when out of memory.
fn malloc_copy(bytes: &[u8]) -> *mut c_char {
    // SAFETY: `malloc` either fails with null or returns `len + 1` writable
    // bytes; `bytes` is a valid slice of `len` bytes that cannot overlap a
    // fresh allocation.
    unsafe {
        let copy = malloc(bytes.len() + 1).cast::<u8>();
        if copy.is_null() {
            return std::ptr::null_mut();
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), copy, bytes.len());
        *copy.add(bytes.len()) = 0;
        copy.cast::<c_char>()
    }
}

/// Zeroes and frees every answer of a `calloc`'d array of `count` responses,
/// then the array — what a conversation does with what it made when it
/// cannot hand it over, and what a test does after reading it.
///
/// # Safety
///
/// `replies` is null, or a `calloc`'d array of `count` responses whose `resp`
/// are null or `malloc`'d NUL-terminated strings, none of them used again.
pub(crate) unsafe fn drop_replies(replies: *mut PamResponse, count: usize) {
    if replies.is_null() {
        return;
    }
    for i in 0..count {
        // SAFETY: `i < count`, inside the array the caller vouched for.
        let reply = unsafe { &mut *replies.add(i) };
        if !reply.resp.is_null() {
            // SAFETY: a NUL-terminated `malloc`'d string, per the contract:
            // its `len` bytes are ours to overwrite and then to free.
            unsafe {
                let len = CStr::from_ptr(reply.resp).to_bytes().len();
                std::slice::from_raw_parts_mut(reply.resp.cast::<u8>(), len).zeroize();
                free(reply.resp.cast::<c_void>());
            }
            reply.resp = std::ptr::null_mut();
        }
    }
    // SAFETY: the array itself came from `calloc`.
    unsafe { free(replies.cast::<c_void>()) };
}

/// The conversation: answers every `PAM_PROMPT_ECHO_OFF` with a `malloc`'d
/// copy of the secret, keeps the words of `PAM_ERROR_MSG` and
/// `PAM_TEXT_INFO`, and refuses everything else — a prompt that ECHOES asks
/// for something that is not a password (a user name, a code), and guessing
/// what would be answering a question nobody asked the person.
///
/// Never unwinds: nothing in it panics short of an allocation failure, which
/// aborts either way.
///
/// # Safety
///
/// Called by libpam with its own arguments: `msg` points at `num_msg`
/// message pointers, `resp` is writable, and `appdata_ptr` is the
/// [`Answering`] [`verify_with_pam`] handed to `pam_start`, alive for the
/// whole call.
pub(crate) unsafe extern "C" fn converse(
    num_msg: c_int,
    msg: *const *const PamMessage,
    resp: *mut *mut PamResponse,
    appdata_ptr: *mut c_void,
) -> c_int {
    if !(1..=PAM_MAX_NUM_MSG).contains(&num_msg)
        || msg.is_null()
        || resp.is_null()
        || appdata_ptr.is_null()
    {
        return PAM_CONV_ERR;
    }
    // SAFETY: the pointer `verify_with_pam` gave `pam_start`, alive for the
    // call and touched by nothing else while PAM runs on this thread.
    let answering = unsafe { &mut *appdata_ptr.cast::<Answering<'_>>() };
    let count = usize::try_from(num_msg).unwrap_or(0);
    // SAFETY: `calloc` returns null or `count` zeroed responses — every
    // `resp` null, which is what an unanswered message is.
    let replies = unsafe { calloc(count, size_of::<PamResponse>()) }.cast::<PamResponse>();
    if replies.is_null() {
        return PAM_BUF_ERR;
    }
    // A REFUSAL HANDS NOTHING OVER: every copy already made is zeroed and
    // freed, with the array, and `resp` is left as it was.
    let refuse = || {
        // SAFETY: the array and what was put in it so far are ours.
        unsafe { drop_replies(replies, count) };
        PAM_CONV_ERR
    };
    for i in 0..count {
        // SAFETY: `i < num_msg`, and libpam hands `num_msg` pointers.
        let message = unsafe { *msg.add(i) };
        if message.is_null() {
            return refuse();
        }
        // SAFETY: a message libpam built, valid for the call.
        let message = unsafe { &*message };
        match message.msg_style {
            PAM_PROMPT_ECHO_OFF => {
                let copy = malloc_copy(answering.secret);
                if copy.is_null() {
                    // SAFETY: as above.
                    unsafe { drop_replies(replies, count) };
                    return PAM_BUF_ERR;
                }
                // SAFETY: `i < count`, inside the fresh array.
                unsafe { (*replies.add(i)).resp = copy };
            }
            PAM_ERROR_MSG | PAM_TEXT_INFO => {
                if !message.msg.is_null() {
                    // SAFETY: a NUL-terminated string libpam built.
                    let words = unsafe { CStr::from_ptr(message.msg) };
                    answering
                        .told
                        .push(words.to_string_lossy().trim().to_owned());
                }
            }
            // AN ECHOING PROMPT asks for something that is not a password…
            PAM_PROMPT_ECHO_ON => return refuse(),
            // …and so does a radio or binary prompt, or a style this does not
            // know.
            _ => return refuse(),
        }
    }
    // SAFETY: `resp` is writable (checked non-null); libpam owns the array now.
    unsafe { *resp = replies };
    PAM_SUCCESS
}

/// A started PAM transaction, ended (`pam_end`) with its last status when
/// dropped — every way out of [`verify_with_pam`] passes through here.
struct Transaction {
    handle: *mut PamHandle,
    last: c_int,
}

impl Transaction {
    /// PAM's own words for `code` (`pam_strerror`), for a log.
    fn words(&self, code: c_int) -> String {
        strerror(self.handle, code)
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            // SAFETY: a handle `pam_start` returned, ended exactly once.
            unsafe { pam_end(self.handle, self.last) };
        }
    }
}

/// `pam_strerror`'s words for `code`; the handle may be null (Linux-PAM does
/// not read it).
fn strerror(handle: *mut PamHandle, code: c_int) -> String {
    // SAFETY: `pam_strerror` returns a static string (or null) for any code.
    let words = unsafe { pam_strerror(handle, code) };
    if words.is_null() {
        return format!("PAM error {code}");
    }
    // SAFETY: a NUL-terminated static string.
    unsafe { CStr::from_ptr(words) }
        .to_string_lossy()
        .into_owned()
}

/// Which call a status came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    /// `pam_start`.
    Start,
    /// `pam_authenticate`.
    Authenticate,
    /// `pam_acct_mgmt`.
    Account,
}

/// What a PAM transaction ended in: its step, its status and PAM's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ended {
    /// Where it stopped.
    pub(crate) step: Step,
    /// The status that stopped it.
    pub(crate) code: c_int,
    /// `pam_strerror`, then what the conversation was told.
    pub(crate) words: String,
}

/// One whole transaction: start under `service` for `user`, authenticate,
/// check the account, end. `Ok` only when both said `PAM_SUCCESS`. Blocks
/// (PAM sleeps after a wrong password).
///
/// `PAM_SILENT`, and `PAM_DISALLOW_NULL_AUTHTOK`: an account with an empty
/// password is never let through by an empty answer.
pub(crate) fn verify_with_pam(service: &CStr, user: &CStr, secret: &[u8]) -> Result<(), Ended> {
    let mut answering = Answering {
        secret,
        told: Vec::new(),
    };
    let conversation = PamConv {
        conv: Some(converse),
        appdata_ptr: (&raw mut answering).cast::<c_void>(),
    };
    let mut handle: *mut PamHandle = std::ptr::null_mut();
    // SAFETY: NUL-terminated strings, a conversation that outlives the
    // transaction (it lives on this frame until after `pam_end`), and a
    // writable handle slot.
    let code = unsafe {
        pam_start(
            service.as_ptr(),
            user.as_ptr(),
            &raw const conversation,
            &raw mut handle,
        )
    };
    let mut transaction = Transaction { handle, last: code };
    if code != PAM_SUCCESS {
        return Err(Ended {
            step: Step::Start,
            code,
            words: strerror(handle, code),
        });
    }
    let flags = PAM_SILENT | PAM_DISALLOW_NULL_AUTHTOK;
    for (step, call) in [
        (
            Step::Authenticate,
            pam_authenticate as unsafe extern "C" fn(_, _) -> _,
        ),
        (Step::Account, pam_acct_mgmt),
    ] {
        // SAFETY: a live handle from `pam_start`.
        let code = unsafe { call(transaction.handle, flags) };
        transaction.last = code;
        if code != PAM_SUCCESS {
            let mut words = transaction.words(code);
            for line in answering.told.iter().filter(|l| !l.is_empty()) {
                words.push_str(" — ");
                words.push_str(line);
            }
            return Err(Ended { step, code, words });
        }
    }
    drop(transaction);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A message array as libpam hands one to a conversation.
    fn ask(messages: &[(c_int, &CStr)]) -> (Vec<PamMessage>, Vec<*const PamMessage>) {
        let owned: Vec<PamMessage> = messages
            .iter()
            .map(|(style, text)| PamMessage {
                msg_style: *style,
                msg: text.as_ptr(),
            })
            .collect();
        let pointers = owned.iter().map(std::ptr::from_ref).collect();
        (owned, pointers)
    }

    /// Runs the conversation over `messages`, returning its status, what it
    /// answered (read, then zeroed and freed) and what it was told.
    fn converse_over(
        messages: &[(c_int, &CStr)],
        secret: &[u8],
    ) -> (c_int, Vec<Option<Vec<u8>>>, Vec<String>) {
        let (_owned, pointers) = ask(messages);
        let mut answering = Answering {
            secret,
            told: Vec::new(),
        };
        let mut replies: *mut PamResponse = std::ptr::null_mut();
        let count = c_int::try_from(pointers.len()).unwrap();
        // SAFETY: the arguments libpam would pass, built by hand.
        let code = unsafe {
            converse(
                count,
                pointers.as_ptr(),
                &raw mut replies,
                (&raw mut answering).cast::<c_void>(),
            )
        };
        let mut answers = Vec::new();
        if code == PAM_SUCCESS {
            for i in 0..pointers.len() {
                // SAFETY: the array the conversation returned.
                let resp = unsafe { (*replies.add(i)).resp };
                answers.push((!resp.is_null()).then(|| {
                    // SAFETY: a NUL-terminated `malloc`'d copy.
                    unsafe { CStr::from_ptr(resp) }.to_bytes().to_vec()
                }));
            }
            // SAFETY: what libpam would free, freed here instead.
            unsafe { drop_replies(replies, pointers.len()) };
        } else {
            assert!(
                replies.is_null(),
                "a refused conversation hands nothing over"
            );
        }
        (code, answers, answering.told)
    }

    #[test]
    fn a_hidden_prompt_is_answered_with_a_copy_of_the_secret() {
        let (code, answers, told) =
            converse_over(&[(PAM_PROMPT_ECHO_OFF, c"Password: ")], b"s3cr3t");
        assert_eq!(code, PAM_SUCCESS);
        assert_eq!(answers, [Some(b"s3cr3t".to_vec())]);
        assert!(told.is_empty());
    }

    #[test]
    fn words_are_kept_and_left_unanswered() {
        let (code, answers, told) = converse_over(
            &[
                (PAM_TEXT_INFO, c"  Your password expires soon  "),
                (PAM_PROMPT_ECHO_OFF, c"Password: "),
                (PAM_ERROR_MSG, c"Authentication token is locked"),
            ],
            b"pw",
        );
        assert_eq!(code, PAM_SUCCESS);
        assert_eq!(answers, [None, Some(b"pw".to_vec()), None]);
        assert_eq!(
            told,
            [
                "Your password expires soon",
                "Authentication token is locked"
            ]
        );
    }

    #[test]
    fn a_prompt_that_echoes_is_refused_and_nothing_is_handed_over() {
        // An echoing prompt asks for something that is not a password: the
        // copy already made for the first prompt is zeroed and freed, and
        // `resp` is left alone.
        let (code, answers, _) = converse_over(
            &[
                (PAM_PROMPT_ECHO_OFF, c"Password: "),
                (PAM_PROMPT_ECHO_ON, c"login: "),
            ],
            b"pw",
        );
        assert_eq!(code, PAM_CONV_ERR);
        assert!(answers.is_empty());
        let (code, _, _) = converse_over(&[(99, c"binary")], b"pw");
        assert_eq!(code, PAM_CONV_ERR, "an unknown style is refused too");
    }

    #[test]
    fn a_malformed_call_is_refused() {
        let mut replies: *mut PamResponse = std::ptr::null_mut();
        let mut answering = Answering {
            secret: b"x",
            told: Vec::new(),
        };
        let app = (&raw mut answering).cast::<c_void>();
        let (_owned, pointers) = ask(&[(PAM_PROMPT_ECHO_OFF, c"Password: ")]);
        // SAFETY: each call is refused before anything is read.
        unsafe {
            assert_eq!(
                converse(0, pointers.as_ptr(), &raw mut replies, app),
                PAM_CONV_ERR
            );
            assert_eq!(
                converse(
                    PAM_MAX_NUM_MSG + 1,
                    pointers.as_ptr(),
                    &raw mut replies,
                    app
                ),
                PAM_CONV_ERR
            );
            assert_eq!(
                converse(1, std::ptr::null(), &raw mut replies, app),
                PAM_CONV_ERR
            );
            assert_eq!(
                converse(1, pointers.as_ptr(), std::ptr::null_mut(), app),
                PAM_CONV_ERR
            );
            assert_eq!(
                converse(1, pointers.as_ptr(), &raw mut replies, std::ptr::null_mut()),
                PAM_CONV_ERR
            );
            let null_message = [std::ptr::null::<PamMessage>()];
            assert_eq!(
                converse(1, null_message.as_ptr(), &raw mut replies, app),
                PAM_CONV_ERR
            );
        }
        assert!(replies.is_null());
    }

    #[test]
    fn an_answer_dropped_is_zeroed_first() {
        // The bytes are read back through a second pointer between the zeroing
        // and the free — the one moment a test can see them — by zeroing by
        // hand the way `drop_replies` does.
        let copy = malloc_copy(b"abc");
        assert!(!copy.is_null());
        // SAFETY: the fresh copy, three bytes and a NUL.
        unsafe {
            std::slice::from_raw_parts_mut(copy.cast::<u8>(), 3).zeroize();
            assert_eq!(
                std::slice::from_raw_parts(copy.cast::<u8>(), 4),
                [0, 0, 0, 0]
            );
            free(copy.cast::<c_void>());
        }
        // And `drop_replies` over an array with holes frees what is there.
        // SAFETY: a `calloc`'d array of two, one answered.
        unsafe {
            let replies = calloc(2, size_of::<PamResponse>()).cast::<PamResponse>();
            (*replies.add(1)).resp = malloc_copy(b"pw");
            drop_replies(replies, 2);
            drop_replies(std::ptr::null_mut(), 2);
        }
    }

    #[test]
    fn pams_words_are_its_own() {
        assert_eq!(strerror(std::ptr::null_mut(), PAM_SUCCESS), "Success");
        assert_eq!(
            strerror(std::ptr::null_mut(), PAM_AUTH_ERR),
            "Authentication failure"
        );
    }
}
