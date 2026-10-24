//! Loads the embedded ConPTY and opens a pseudoconsole through the crate's own types.

use core::ffi::{CStr, c_char, c_void};
use core::{mem, ptr};
use std::os::windows::ffi::OsStrExt;
use std::vec::Vec;
use std::{fs, process, thread};

use crate::{
    ConptyClosePseudoConsoleFn, ConptyCreatePseudoConsoleFn, ConptyResizePseudoConsoleFn, Coord,
    Handle, Hpcon, export,
};

const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
const TH32CS_SNAPPROCESS: u32 = 0x2;

#[repr(C)]
struct ProcessEntry32W {
    size: u32,
    usage: u32,
    process_id: u32,
    default_heap_id: usize,
    module_id: u32,
    threads: u32,
    parent_process_id: u32,
    pri_class_base: i32,
    flags: u32,
    exe_file: [u16; 260],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(name: *const u16, file: Handle, flags: u32) -> Handle;
    fn GetProcAddress(module: Handle, name: *const c_char) -> *const c_void;
    fn CreatePipe(
        read: *mut Handle,
        write: *mut Handle,
        attributes: *const c_void,
        size: u32,
    ) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn ReadFile(
        file: Handle,
        buf: *mut u8,
        len: u32,
        read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry32W) -> i32;
}

/// # Safety
/// `F` must be the function pointer type of the export `name`.
unsafe fn resolve<F: Copy>(module: Handle, name: &CStr) -> F {
    // SAFETY: `name` is NUL-terminated and `module` a loaded module.
    let address = unsafe { GetProcAddress(module, name.as_ptr()) };
    assert!(!address.is_null(), "conpty.dll does not export {name:?}");
    assert_eq!(mem::size_of::<F>(), mem::size_of::<*const c_void>());
    // SAFETY: the caller names the matching type; both are pointer-sized.
    unsafe { mem::transmute_copy(&address) }
}

/// The child processes of this process whose image is `OpenConsole.exe`.
fn openconsole_children() -> usize {
    // SAFETY: plain Toolhelp enumeration over a snapshot this function owns.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        assert_ne!(snapshot, INVALID_HANDLE_VALUE);
        let mut entry: ProcessEntry32W = mem::zeroed();
        entry.size = mem::size_of::<ProcessEntry32W>() as u32;
        let mut found = 0;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            let len = entry.exe_file.iter().position(|&c| c == 0).unwrap_or(260);
            let exe = std::string::String::from_utf16_lossy(&entry.exe_file[..len]);
            if entry.parent_process_id == process::id()
                && exe.eq_ignore_ascii_case("OpenConsole.exe")
            {
                found += 1;
            }
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        found
    }
}

#[test]
fn the_embedded_conpty_opens_a_pseudoconsole_on_the_bundled_openconsole() {
    let dir =
        std::env::temp_dir().join(std::format!("conpty-bin-vendored-smoke-{}", process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("conpty.dll"), crate::CONPTY_DLL).unwrap();
    fs::write(dir.join("OpenConsole.exe"), crate::OPENCONSOLE_EXE).unwrap();

    let path: Vec<u16> = dir
        .join("conpty.dll")
        .as_os_str()
        .encode_wide()
        .chain([0])
        .collect();
    // SAFETY: an absolute, NUL-terminated path to the DLL just written.
    let module = unsafe { LoadLibraryExW(path.as_ptr(), ptr::null_mut(), 0) };
    assert!(
        !module.is_null(),
        "LoadLibraryExW: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: each type is the crate's declaration of that export.
    let (create, resize, close) = unsafe {
        (
            resolve::<ConptyCreatePseudoConsoleFn>(module, export::ConptyCreatePseudoConsole),
            resolve::<ConptyResizePseudoConsoleFn>(module, export::ConptyResizePseudoConsole),
            resolve::<ConptyClosePseudoConsoleFn>(module, export::ConptyClosePseudoConsole),
        )
    };

    let (mut input_read, mut input_write) = (ptr::null_mut(), ptr::null_mut());
    let (mut output_read, mut output_write) = (ptr::null_mut(), ptr::null_mut());
    // SAFETY: out-pointers to locals.
    unsafe {
        assert_ne!(
            CreatePipe(&mut input_read, &mut input_write, ptr::null(), 0),
            0
        );
        assert_ne!(
            CreatePipe(&mut output_read, &mut output_write, ptr::null(), 0),
            0
        );
    }
    let mut pc: Hpcon = ptr::null_mut();
    // SAFETY: valid pipe ends and an out-pointer to a local.
    let hr = unsafe { create(Coord { x: 80, y: 25 }, input_read, output_write, 0, &mut pc) };
    assert_eq!(hr, 0, "ConptyCreatePseudoConsole: {hr:#x}");
    // SAFETY: the pseudoconsole duplicated the ends it keeps.
    unsafe {
        CloseHandle(input_read);
        CloseHandle(output_write);
    }

    // Drains until the host closes its end; an undrained pipe would block the host.
    let output_read = output_read as usize;
    let reader = thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            let mut read = 0;
            // SAFETY: a live pipe end and a local buffer of the given length.
            let ok = unsafe {
                ReadFile(
                    output_read as Handle,
                    buf.as_mut_ptr(),
                    buf.len() as u32,
                    &mut read,
                    ptr::null_mut(),
                )
            };
            if ok == 0 || read == 0 {
                break;
            }
        }
        // SAFETY: the read end this thread was given.
        unsafe { CloseHandle(output_read as Handle) };
    });

    assert_eq!(
        openconsole_children(),
        1,
        "conpty.dll fell back to another host"
    );

    // SAFETY: the live pseudoconsole and the input end this test owns.
    unsafe {
        assert_eq!(resize(pc, Coord { x: 100, y: 30 }), 0);
        close(pc);
        CloseHandle(input_write);
    }
    reader.join().unwrap();
}
