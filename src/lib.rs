//! Windows Terminal's out-of-band ConPTY, unmodified from Microsoft's NuGet package
//! `Microsoft.Windows.Console.ConPTY`, with the API of that version.
//!
//! Write [`CONPTY_DLL`] and [`OPENCONSOLE_EXE`] into one directory, as `conpty.dll` and
//! `OpenConsole.exe`, load `conpty.dll` from there, resolve the functions by the names in
//! [`export`] and call them through the matching `*Fn` types. Ship [`MICROSOFT_LICENSE`] with them.
#![no_std]

use core::ffi::{CStr, c_void};

/// The `Microsoft.Windows.Console.ConPTY` package version the bytes come from.
pub const VERSION: &str = upstream_version(env!("CARGO_PKG_VERSION"));

/// Microsoft's MIT notice, which must accompany the files.
pub const MICROSOFT_LICENSE: &str = include_str!("../LICENSE-MICROSOFT");

#[cfg(target_arch = "x86_64")]
macro_rules! arch_dir {
    () => {
        "x64"
    };
}
#[cfg(target_arch = "aarch64")]
macro_rules! arch_dir {
    () => {
        "arm64"
    };
}
#[cfg(target_arch = "x86")]
macro_rules! arch_dir {
    () => {
        "x86"
    };
}

/// `conpty.dll` for the target architecture.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86"))]
pub const CONPTY_DLL: &[u8] = include_bytes!(concat!("../assets/", arch_dir!(), "/conpty.dll"));

/// `OpenConsole.exe` for the target architecture.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86"))]
pub const OPENCONSOLE_EXE: &[u8] =
    include_bytes!(concat!("../assets/", arch_dir!(), "/OpenConsole.exe"));

/// Win32 `HANDLE`.
pub type Handle = *mut c_void;
/// Win32 `HPCON`.
pub type Hpcon = *mut c_void;
/// Win32 `HWND`.
pub type Hwnd = *mut c_void;
/// Win32 `HRESULT`.
pub type Hresult = i32;
/// Win32 `BOOL`.
pub type Bool = i32;

/// Win32 `COORD`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Coord {
    pub x: i16,
    pub y: i16,
}

/// A `CreatePseudoConsole` flag of this version, from its `conpty.h`.
pub const PSEUDOCONSOLE_INHERIT_CURSOR: u32 = 0x1;
/// A `CreatePseudoConsole` flag of this version, from its `conpty.h`.
pub const PSEUDOCONSOLE_GLYPH_WIDTH__MASK: u32 = 0x18;
/// A `CreatePseudoConsole` flag of this version, from its `conpty.h`.
pub const PSEUDOCONSOLE_GLYPH_WIDTH_GRAPHEMES: u32 = 0x08;
/// A `CreatePseudoConsole` flag of this version, from its `conpty.h`.
pub const PSEUDOCONSOLE_GLYPH_WIDTH_WCSWIDTH: u32 = 0x10;
/// A `CreatePseudoConsole` flag of this version, from its `conpty.h`.
pub const PSEUDOCONSOLE_GLYPH_WIDTH_CONSOLE: u32 = 0x18;
/// A `CreatePseudoConsole` flag of this version, from its `conpty.h`.
pub const PSEUDOCONSOLE_AMBIGUOUS_IS_WIDE: u32 = 0x20;

#[cfg(test)]
macro_rules! ret_text {
    () => {
        "()"
    };
    ($ret:ty) => {
        stringify!($ret)
    };
}

macro_rules! functions {
    ($($fn_ty:ident = $name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?;)*) => {
        $(
            #[doc = concat!("The type of `", stringify!($name), "`.")]
            pub type $fn_ty = unsafe extern "system" fn($($arg: $ty),*) $(-> $ret)?;
        )*

        /// Export names of the functions `conpty.h` declares.
        #[allow(non_upper_case_globals)]
        pub mod export {
            use core::ffi::CStr;
            $(
                #[doc = concat!("`", stringify!($name), "`.")]
                pub const $name: &CStr = super::nul_terminated(concat!(stringify!($name), "\0"));
            )*
        }

        #[cfg(test)]
        const SIGNATURES: &[(&str, &[&str], &str)] = &[
            $((stringify!($name), &[$(stringify!($ty)),*], ret_text!($($ret)?)),)*
        ];
    };
}

functions! {
    ConptyCreatePseudoConsoleFn = ConptyCreatePseudoConsole(
        size: Coord, input: Handle, output: Handle, flags: u32, pc: *mut Hpcon
    ) -> Hresult;
    ConptyCreatePseudoConsoleAsUserFn = ConptyCreatePseudoConsoleAsUser(
        token: Handle, size: Coord, input: Handle, output: Handle, flags: u32, pc: *mut Hpcon
    ) -> Hresult;
    ConptyResizePseudoConsoleFn = ConptyResizePseudoConsole(pc: Hpcon, size: Coord) -> Hresult;
    ConptyClearPseudoConsoleFn = ConptyClearPseudoConsole(pc: Hpcon, keep_cursor_row: Bool) -> Hresult;
    ConptyShowHidePseudoConsoleFn = ConptyShowHidePseudoConsole(pc: Hpcon, show: Bool) -> Hresult;
    ConptyReparentPseudoConsoleFn = ConptyReparentPseudoConsole(pc: Hpcon, new_parent: Hwnd) -> Hresult;
    ConptyReleasePseudoConsoleFn = ConptyReleasePseudoConsole(pc: Hpcon) -> Hresult;
    ConptyClosePseudoConsoleFn = ConptyClosePseudoConsole(pc: Hpcon);
    ConptyPackPseudoConsoleFn = ConptyPackPseudoConsole(
        server_process: Handle, reference: Handle, signal: Handle, pc: *mut Hpcon
    ) -> Hresult;
}

const fn nul_terminated(name: &'static str) -> &'static CStr {
    match CStr::from_bytes_with_nul(name.as_bytes()) {
        Ok(name) => name,
        Err(_) => panic!("an export name with an interior NUL"),
    }
}

const fn upstream_version(crate_version: &str) -> &str {
    let Some((revision, upstream)) = crate_version.as_bytes().split_last() else {
        panic!("empty crate version");
    };
    assert!(
        revision.is_ascii_digit(),
        "the crate version must end in a revision digit"
    );
    match core::str::from_utf8(upstream) {
        Ok(upstream) => upstream,
        Err(_) => panic!("the crate version is not UTF-8"),
    }
}

#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests;
