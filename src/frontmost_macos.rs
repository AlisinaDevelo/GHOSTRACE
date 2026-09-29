//! NSWorkspace frontmost-application adapter (opt-in `frontmost` feature).
//!
//! The adapter asks `NSWorkspace` for its frontmost `NSRunningApplication`
//! and reads only the facts in [`FrontmostRawObservation`]: the bundle
//! identifier, activation policy, whether the executable is bundled and
//! translocated, the developer-set `CFBundleName` and
//! `CFBundleShortVersionString` from the bundle's unlocalized Info.plist, the
//! process start time, and code-signing facts. It never asks for a window
//! title, document, URL, or Accessibility data, and needs no permission.
//!
//! `NSWorkspace` updates `frontmostApplication` from notifications delivered
//! on the run loop, so in a command-line process it stays stale unless the
//! main thread's run loop runs. [`FrontmostProbe::poll`] runs the calling
//! thread's run loop for the polling interval before each read, so it must be
//! called on the main thread; elsewhere it keeps reporting the application
//! that was frontmost when the process started.

use std::{
    ffi::{c_char, c_void, CStr},
    str::FromStr,
    time::Duration,
};

use chrono::Utc;
use core_foundation::{
    base::{CFType, TCFType},
    dictionary::{CFDictionary, CFDictionaryRef},
    number::CFNumber,
    string::{CFString, CFStringRef},
};
use security_framework::os::macos::code_signing::{
    Flags, GuestAttributes, SecCode, SecRequirement,
};

use crate::frontmost::{
    FrontmostActivationPolicy, FrontmostRawObservation, FrontmostSigningInput, FrontmostTransition,
};
use crate::fsevents::ffi::{kCFRunLoopDefaultMode, CFRunLoopRunInMode};

type Id = *mut c_void;
type Sel = *mut c_void;

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_autoreleasePoolPush() -> *mut c_void;
    fn objc_autoreleasePoolPop(pool: *mut c_void);
}

#[link(name = "Security", kind = "framework")]
extern "C" {
    static kSecCodeInfoTeamIdentifier: CFStringRef;
    static kSecCodeInfoFlags: CFStringRef;
    static kSecCodeInfoPlatformIdentifier: CFStringRef;
    fn SecCodeCopySigningInformation(
        code: *const c_void,
        flags: u32,
        information: *mut CFDictionaryRef,
    ) -> i32;
}

const K_CF_RUN_LOOP_RUN_FINISHED: i32 = 1;
const K_SEC_CS_SIGNING_INFORMATION: u32 = 1 << 1;
const K_SEC_CODE_SIGNATURE_ADHOC: i64 = 0x0002;

/// Samples the frontmost application and reports each change of launch
/// instance as an activation.
#[derive(Debug, Default)]
pub struct FrontmostProbe {
    last: Option<(i32, i64)>,
}

impl FrontmostProbe {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run this thread's run loop for `interval`, then return an activation
    /// if a different launch instance is now frontmost. The observation time
    /// is the read time, at most `interval` after the switch.
    pub fn poll(&mut self, interval: Duration) -> Option<FrontmostRawObservation> {
        pump_run_loop(interval);
        let observation = current_frontmost()?;
        let instance = (observation.process_id, observation.process_started_micros);
        if self.last == Some(instance) {
            return None;
        }
        self.last = Some(instance);
        Some(observation)
    }
}

/// Run the calling thread's run loop so AppKit can deliver the workspace
/// notifications that keep `frontmostApplication` current.
pub fn pump_run_loop(interval: Duration) {
    let started = std::time::Instant::now();
    // SAFETY: kCFRunLoopDefaultMode is a constant CFString; running the
    // current thread's run loop has no other preconditions.
    let result = unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, interval.as_secs_f64(), 0) };
    // A thread whose run loop has no sources returns at once (off the main
    // thread, AppKit delivers nothing here). Keep the polling interval
    // anyway so a caller never spins.
    if result == K_CF_RUN_LOOP_RUN_FINISHED {
        std::thread::sleep(interval.saturating_sub(started.elapsed()));
    }
}

/// Whether the calling thread is the process's main thread, the only one on
/// which [`FrontmostProbe::poll`] sees focus changes.
pub fn is_main_thread() -> bool {
    // SAFETY: pthread_main_np has no preconditions.
    unsafe { libc::pthread_main_np() == 1 }
}

/// The frontmost application as an activation observation, or `None` when
/// no application is frontmost (for example at the login window).
pub fn current_frontmost() -> Option<FrontmostRawObservation> {
    // SAFETY: every message is sent to an object of the class that defines
    // it, with the argument and return types that selector declares; the
    // autorelease pool bounds every returned object's lifetime, and strings
    // are copied out before the pool is popped.
    let facts = unsafe {
        let pool = objc_autoreleasePoolPush();
        let facts = workspace_facts();
        objc_autoreleasePoolPop(pool);
        facts
    }?;
    let process_started_micros = process_start_micros(facts.process_id).unwrap_or(0);
    Some(FrontmostRawObservation {
        transition: FrontmostTransition::Activated,
        observed_at: Utc::now(),
        bundle_identifier: facts.bundle_identifier,
        bundle_name: facts.bundle_name,
        bundle_version: facts.bundle_version,
        bundled: facts.bundled,
        activation_policy: facts.activation_policy,
        translocated: facts.translocated,
        signing: signing_input(facts.process_id),
        process_id: facts.process_id,
        process_started_micros,
    })
}

struct WorkspaceFacts {
    process_id: i32,
    bundle_identifier: Option<String>,
    bundle_name: Option<String>,
    bundle_version: Option<String>,
    bundled: bool,
    activation_policy: FrontmostActivationPolicy,
    translocated: bool,
}

unsafe fn workspace_facts() -> Option<WorkspaceFacts> {
    let workspace = send(class(c"NSWorkspace"), c"sharedWorkspace");
    let app = non_null(send(workspace, c"frontmostApplication"))?;
    let process_id = send_i32(app, c"processIdentifier");
    let bundle_identifier = string(send(app, c"bundleIdentifier"));
    let activation_policy = match send_isize(app, c"activationPolicy") {
        0 => FrontmostActivationPolicy::Regular,
        1 => FrontmostActivationPolicy::Accessory,
        2 => FrontmostActivationPolicy::Prohibited,
        _ => FrontmostActivationPolicy::Unknown,
    };
    let (mut bundled, mut translocated, mut bundle_name, mut bundle_version) =
        (false, false, None, None);
    if let Some(url) = non_null(send(app, c"bundleURL")) {
        // The path is read only to recognize App Translocation; it is not
        // kept.
        let path = string(send(url, c"path")).unwrap_or_default();
        translocated = path.contains("/AppTranslocation/");
        if let Some(bundle) = non_null(send_id(class(c"NSBundle"), c"bundleWithURL:", url)) {
            bundled =
                path.ends_with(".app") || non_null(send(bundle, c"bundleIdentifier")).is_some();
            // `infoDictionary` is the unlocalized Info.plist; the localized
            // or user-renamed display name is never read.
            if let Some(info) = non_null(send(bundle, c"infoDictionary")) {
                bundle_name = info_string(info, c"CFBundleName");
                bundle_version = info_string(info, c"CFBundleShortVersionString");
            }
        }
    }
    Some(WorkspaceFacts {
        process_id,
        bundle_identifier,
        bundle_name,
        bundle_version,
        bundled,
        activation_policy,
        translocated,
    })
}

unsafe fn info_string(info: Id, key: &CStr) -> Option<String> {
    let key = send_ptr(class(c"NSString"), c"stringWithUTF8String:", key.as_ptr());
    let value = non_null(send_id(info, c"objectForKey:", key))?;
    if !send_bool_id(value, c"isKindOfClass:", class(c"NSString")) {
        return None;
    }
    string(value)
}

unsafe fn class(name: &CStr) -> Id {
    objc_getClass(name.as_ptr())
}

fn non_null(object: Id) -> Option<Id> {
    (!object.is_null()).then_some(object)
}

unsafe fn selector(name: &CStr) -> Sel {
    sel_registerName(name.as_ptr())
}

unsafe fn send(receiver: Id, name: &CStr) -> Id {
    if receiver.is_null() {
        return std::ptr::null_mut();
    }
    let function: unsafe extern "C" fn(Id, Sel) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    function(receiver, selector(name))
}

unsafe fn send_id(receiver: Id, name: &CStr, argument: Id) -> Id {
    if receiver.is_null() {
        return std::ptr::null_mut();
    }
    let function: unsafe extern "C" fn(Id, Sel, Id) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    function(receiver, selector(name), argument)
}

unsafe fn send_ptr(receiver: Id, name: &CStr, argument: *const c_char) -> Id {
    let function: unsafe extern "C" fn(Id, Sel, *const c_char) -> Id =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    function(receiver, selector(name), argument)
}

unsafe fn send_i32(receiver: Id, name: &CStr) -> i32 {
    let function: unsafe extern "C" fn(Id, Sel) -> i32 =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    function(receiver, selector(name))
}

unsafe fn send_isize(receiver: Id, name: &CStr) -> isize {
    let function: unsafe extern "C" fn(Id, Sel) -> isize =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    function(receiver, selector(name))
}

unsafe fn send_bool_id(receiver: Id, name: &CStr, argument: Id) -> bool {
    // BOOL is a one-byte `bool` on arm64 and a signed char on x86_64.
    let function: unsafe extern "C" fn(Id, Sel, Id) -> i8 =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    function(receiver, selector(name), argument) != 0
}

/// Copy an `NSString` into Rust.
unsafe fn string(object: Id) -> Option<String> {
    let object = non_null(object)?;
    let function: unsafe extern "C" fn(Id, Sel) -> *const c_char =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let bytes = function(object, selector(c"UTF8String"));
    (!bytes.is_null()).then(|| CStr::from_ptr(bytes).to_string_lossy().into_owned())
}

/// Process start time in microseconds since the epoch, or `None` when the
/// process is gone or belongs to another user.
fn process_start_micros(process_id: i32) -> Option<i64> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: `info` is a writable proc_bsdinfo of exactly `size` bytes.
    let written = unsafe {
        libc::proc_pidinfo(
            process_id,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    if written != size {
        return None;
    }
    let micros = (info.pbi_start_tvsec as i64)
        .checked_mul(1_000_000)?
        .checked_add(info.pbi_start_tvusec as i64)?;
    (micros > 0).then_some(micros)
}

/// Dynamic code-signing facts for the running process: whether its
/// signature is currently valid, and its ad-hoc, platform, and team
/// identity. `None` means the process has no code identity at all.
fn signing_input(process_id: i32) -> Option<FrontmostSigningInput> {
    let mut attributes = GuestAttributes::new();
    attributes.set_pid(process_id);
    let code = SecCode::copy_guest_with_attribues(None, &attributes, Flags::NONE).ok()?;
    let any = SecRequirement::from_str("always").ok()?;
    let valid = code.check_validity(Flags::NONE, &any).is_ok();
    let mut information: CFDictionaryRef = std::ptr::null();
    // SAFETY: `code` is a live SecCodeRef; on success `information` is a
    // dictionary we own under the create rule.
    let status = unsafe {
        SecCodeCopySigningInformation(
            code.as_concrete_TypeRef().cast(),
            K_SEC_CS_SIGNING_INFORMATION,
            &mut information,
        )
    };
    if status != 0 || information.is_null() {
        return Some(FrontmostSigningInput {
            valid: false,
            ad_hoc: false,
            platform_binary: false,
            team_identifier: None,
        });
    }
    // SAFETY: returned under the create rule, released when dropped.
    let information: CFDictionary<CFString, CFType> =
        unsafe { CFDictionary::wrap_under_create_rule(information) };
    // SAFETY: the keys are constant CFStrings exported by Security.
    let key = |raw: CFStringRef| unsafe { CFString::wrap_under_get_rule(raw) };
    let number = |raw: CFStringRef| {
        information
            .find(key(raw))
            .and_then(|value| value.downcast::<CFNumber>())
            .and_then(|n| n.to_i64())
    };
    let flags = number(unsafe { kSecCodeInfoFlags }).unwrap_or(0);
    let platform = number(unsafe { kSecCodeInfoPlatformIdentifier }).unwrap_or(0);
    let team_identifier = information
        .find(key(unsafe { kSecCodeInfoTeamIdentifier }))
        .and_then(|value| value.downcast::<CFString>())
        .map(|team| team.to_string());
    Some(FrontmostSigningInput {
        valid,
        ad_hoc: flags & K_SEC_CODE_SIGNATURE_ADHOC != 0,
        platform_binary: platform != 0,
        team_identifier,
    })
}
