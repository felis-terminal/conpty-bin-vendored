use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::string::{String, ToString};
use std::vec::Vec;
use std::{format, fs, vec};

#[cfg(windows)]
mod smoke;

const ARCHES: [(&str, u16); 3] = [("x64", 0x8664), ("arm64", 0xaa64), ("x86", 0x014c)];

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn header() -> String {
    fs::read_to_string(assets().join("conpty.h")).unwrap()
}

#[test]
fn version_drops_the_revision_digit() {
    assert_eq!(super::upstream_version("1.25.2609300031"), "1.25.260930003");
    assert_eq!(super::VERSION.len() + 1, env!("CARGO_PKG_VERSION").len());
}

#[test]
fn assets_hold_exactly_the_seven_package_files() {
    let mut found = BTreeSet::new();
    let mut dirs = vec![assets()];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                let relative = path.strip_prefix(assets()).unwrap();
                found.insert(relative.to_str().unwrap().replace('\\', "/"));
            }
        }
    }
    let mut expected = BTreeSet::from(["conpty.h".to_string()]);
    for (arch, _) in ARCHES {
        expected.insert(format!("{arch}/conpty.dll"));
        expected.insert(format!("{arch}/OpenConsole.exe"));
    }
    assert_eq!(found, expected);
}

/// Parameter order and meaning are what a type comparison cannot see (two `HANDLE`s swapped):
/// an upstream change to any prototype fails here until a human has re-read the Rust types.
#[test]
fn the_header_prototypes_are_the_reviewed_ones() {
    let header: Vec<String> = header()
        .lines()
        .filter(|line| line.trim_start().starts_with("CONPTY_EXPORT "))
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let reviewed = [
        "CONPTY_EXPORT HRESULT WINAPI ConptyCreatePseudoConsole(COORD size, HANDLE hInput, HANDLE hOutput, DWORD dwFlags, HPCON* phPC);",
        "CONPTY_EXPORT HRESULT WINAPI ConptyCreatePseudoConsoleAsUser(HANDLE hToken, COORD size, HANDLE hInput, HANDLE hOutput, DWORD dwFlags, HPCON* phPC);",
        "CONPTY_EXPORT HRESULT WINAPI ConptyResizePseudoConsole(HPCON hPC, COORD size);",
        "CONPTY_EXPORT HRESULT WINAPI ConptyClearPseudoConsole(HPCON hPC, BOOL keepCursorRow);",
        "CONPTY_EXPORT HRESULT WINAPI ConptyShowHidePseudoConsole(HPCON hPC, BOOL show);",
        "CONPTY_EXPORT HRESULT WINAPI ConptyReparentPseudoConsole(HPCON hPC, HWND newParent);",
        "CONPTY_EXPORT HRESULT WINAPI ConptyReleasePseudoConsole(HPCON hPC);",
        "CONPTY_EXPORT VOID WINAPI ConptyClosePseudoConsole(HPCON hPC);",
        "CONPTY_EXPORT HRESULT WINAPI ConptyPackPseudoConsole(HANDLE hServerProcess, HANDLE hRef, HANDLE hSignal, HPCON* phPC);",
    ];
    assert_eq!(header, reviewed);
}

/// `RET WINAPI Name(TYPE name, ...)` for every `CONPTY_EXPORT` line of the header.
fn header_prototypes() -> Vec<(String, Vec<String>, String)> {
    header()
        .lines()
        .filter_map(|line| line.trim().strip_prefix("CONPTY_EXPORT "))
        .map(|rest| {
            let (head, params) = rest.split_once('(').unwrap();
            let mut head = head.split_whitespace();
            let ret = head.next().unwrap().to_string();
            assert_eq!(head.next(), Some("WINAPI"), "{rest}");
            let name = head.next().unwrap().to_string();
            let params = params.trim_end_matches(';').trim_end_matches(')');
            let types = params
                .split(',')
                .map(|param| {
                    let mut words: Vec<&str> = param.split_whitespace().collect();
                    words.pop();
                    words.join(" ")
                })
                .collect();
            (name, types, ret)
        })
        .collect()
}

fn rust_type(c_type: &str) -> &'static str {
    match c_type {
        "HRESULT" => "Hresult",
        "VOID" => "()",
        "COORD" => "Coord",
        "HANDLE" => "Handle",
        "HPCON" => "Hpcon",
        "HPCON*" => "*mut Hpcon",
        "DWORD" => "u32",
        "BOOL" => "Bool",
        "HWND" => "Hwnd",
        other => panic!("no Rust type for the C type {other}; extend the table and the crate"),
    }
}

#[test]
fn every_function_type_matches_its_header_prototype() {
    let header: Vec<(String, Vec<String>, String)> = header_prototypes()
        .into_iter()
        .map(|(name, types, ret)| {
            let types = types.iter().map(|t| rust_type(t).to_string()).collect();
            (name, types, rust_type(&ret).to_string())
        })
        .collect();
    let crate_side: Vec<(String, Vec<String>, String)> = super::SIGNATURES
        .iter()
        .map(|(name, types, ret)| {
            let types = types.iter().map(|t| t.to_string()).collect();
            (name.to_string(), types, ret.to_string())
        })
        .collect();
    assert_eq!(crate_side, header);
}

#[test]
fn every_flag_matches_its_header_define() {
    let header: BTreeSet<(String, u32)> = header()
        .lines()
        .filter_map(|line| line.trim().strip_prefix("#define PSEUDOCONSOLE_"))
        .map(|rest| {
            let (name, value) = rest.split_once(char::is_whitespace).unwrap();
            let value = value.trim().trim_start_matches('(').trim_end_matches(')');
            let value = u32::from_str_radix(value.trim_start_matches("0x"), 16).unwrap();
            (format!("PSEUDOCONSOLE_{name}"), value)
        })
        .collect();
    let crate_side: BTreeSet<(String, u32)> = [
        (
            "PSEUDOCONSOLE_INHERIT_CURSOR",
            super::PSEUDOCONSOLE_INHERIT_CURSOR,
        ),
        (
            "PSEUDOCONSOLE_GLYPH_WIDTH__MASK",
            super::PSEUDOCONSOLE_GLYPH_WIDTH__MASK,
        ),
        (
            "PSEUDOCONSOLE_GLYPH_WIDTH_GRAPHEMES",
            super::PSEUDOCONSOLE_GLYPH_WIDTH_GRAPHEMES,
        ),
        (
            "PSEUDOCONSOLE_GLYPH_WIDTH_WCSWIDTH",
            super::PSEUDOCONSOLE_GLYPH_WIDTH_WCSWIDTH,
        ),
        (
            "PSEUDOCONSOLE_GLYPH_WIDTH_CONSOLE",
            super::PSEUDOCONSOLE_GLYPH_WIDTH_CONSOLE,
        ),
        (
            "PSEUDOCONSOLE_AMBIGUOUS_IS_WIDE",
            super::PSEUDOCONSOLE_AMBIGUOUS_IS_WIDE,
        ),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value))
    .collect();
    assert_eq!(crate_side, header);
}

#[test]
fn every_architecture_exports_every_named_function() {
    for (arch, machine) in ARCHES {
        let dll = fs::read(assets().join(arch).join("conpty.dll")).unwrap();
        let pe = Pe::parse(&dll);
        assert_eq!(pe.machine, machine, "{arch}");
        let exports = pe.export_names();
        for (name, _, _) in super::SIGNATURES {
            assert!(
                exports.iter().any(|e| e == name),
                "{arch} conpty.dll lacks {name}"
            );
        }
    }
}

#[test]
fn every_binary_is_stamped_with_the_package_version() {
    // Package 1.25.260930003 stamps its binaries 1.25.2609.30003.
    for (arch, machine) in ARCHES {
        for file in ["conpty.dll", "OpenConsole.exe"] {
            let bytes = fs::read(assets().join(arch).join(file)).unwrap();
            let pe = Pe::parse(&bytes);
            assert_eq!(pe.machine, machine, "{arch}/{file}");
            let [major, minor, build, private] = pe.file_version();
            let version = format!(
                "{major}.{minor}.{}",
                u64::from(build) * 100_000 + u64::from(private)
            );
            assert_eq!(version, super::VERSION, "{arch}/{file}");
        }
    }
}

struct Section {
    name: [u8; 8],
    virtual_address: u32,
    virtual_size: u32,
    raw_offset: u32,
    raw_size: u32,
}

/// The few PE fields these tests read; panics on anything malformed.
struct Pe<'a> {
    bytes: &'a [u8],
    machine: u16,
    export_rva: u32,
    sections: Vec<Section>,
}

impl<'a> Pe<'a> {
    fn parse(bytes: &'a [u8]) -> Self {
        assert_eq!(&bytes[..2], b"MZ");
        let pe = u32_at(bytes, 0x3c) as usize;
        assert_eq!(&bytes[pe..pe + 4], b"PE\0\0");
        let coff = pe + 4;
        let machine = u16_at(bytes, coff);
        let section_count = u16_at(bytes, coff + 2) as usize;
        let optional_size = u16_at(bytes, coff + 16) as usize;
        let optional = coff + 20;
        let data_directories = match u16_at(bytes, optional) {
            0x10b => optional + 96,
            0x20b => optional + 112,
            magic => panic!("unknown optional header magic {magic:#x}"),
        };
        let export_rva = u32_at(bytes, data_directories);
        let sections = (0..section_count)
            .map(|i| {
                let at = optional + optional_size + i * 40;
                Section {
                    name: bytes[at..at + 8].try_into().unwrap(),
                    virtual_size: u32_at(bytes, at + 8),
                    virtual_address: u32_at(bytes, at + 12),
                    raw_size: u32_at(bytes, at + 16),
                    raw_offset: u32_at(bytes, at + 20),
                }
            })
            .collect();
        Pe {
            bytes,
            machine,
            export_rva,
            sections,
        }
    }

    fn offset(&self, rva: u32) -> usize {
        let section = self
            .sections
            .iter()
            .find(|s| {
                rva >= s.virtual_address && rva < s.virtual_address + s.virtual_size.max(s.raw_size)
            })
            .unwrap_or_else(|| panic!("RVA {rva:#x} is in no section"));
        (rva - section.virtual_address + section.raw_offset) as usize
    }

    fn export_names(&self) -> Vec<String> {
        assert_ne!(self.export_rva, 0, "no export directory");
        let directory = self.offset(self.export_rva);
        let count = u32_at(self.bytes, directory + 24) as usize;
        let names = self.offset(u32_at(self.bytes, directory + 32));
        (0..count)
            .map(|i| {
                let at = self.offset(u32_at(self.bytes, names + i * 4));
                let len = self.bytes[at..].iter().position(|&b| b == 0).unwrap();
                String::from_utf8(self.bytes[at..at + len].to_vec()).unwrap()
            })
            .collect()
    }

    /// `VS_FIXEDFILEINFO`'s file version, found by its signature in `.rsrc`.
    fn file_version(&self) -> [u16; 4] {
        let rsrc = self
            .sections
            .iter()
            .find(|s| &s.name == b".rsrc\0\0\0")
            .expect("no .rsrc section");
        let start = rsrc.raw_offset as usize;
        let data = &self.bytes[start..start + rsrc.raw_size as usize];
        let at = (0..data.len() - 16)
            .step_by(4)
            .find(|&i| u32_at(data, i) == 0xfeef_04bd)
            .expect("no VS_FIXEDFILEINFO");
        let ms = u32_at(data, at + 8);
        let ls = u32_at(data, at + 12);
        [(ms >> 16) as u16, ms as u16, (ls >> 16) as u16, ls as u16]
    }
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
