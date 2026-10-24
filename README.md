# conpty-bin-vendored

Windows Terminal's out-of-band ConPTY as a Rust crate: `conpty.dll` and its `OpenConsole.exe` host, byte-identical to
Microsoft's NuGet package [`Microsoft.Windows.Console.ConPTY`](https://www.nuget.org/packages/Microsoft.Windows.Console.ConPTY),
embedded with `include_bytes!`, together with the API of that version: a function type and an export name for each
function the package's `conpty.h` declares, and its `CreatePseudoConsole` flags. The files are Microsoft's and
Authenticode-signed.

```rust
use conpty_bin_vendored as conpty;

let dir = std::env::temp_dir().join(format!("conpty-{}", conpty::VERSION));
std::fs::create_dir_all(&dir)?;
std::fs::write(dir.join("conpty.dll"), conpty::CONPTY_DLL)?;
std::fs::write(dir.join("OpenConsole.exe"), conpty::OPENCONSOLE_EXE)?;
std::fs::write(dir.join("LICENSE-conpty.txt"), conpty::MICROSOFT_LICENSE)?;
// LoadLibraryExW(dir\conpty.dll), then for each function:
// let create: conpty::ConptyCreatePseudoConsoleFn =
//     transmute(GetProcAddress(module, conpty::export::ConptyCreatePseudoConsole.as_ptr()));
```

`CONPTY_DLL` and `OPENCONSOLE_EXE` hold the files for the compile target's architecture (x86_64, aarch64 or x86) and
do not exist on other architectures. `conpty.dll` starts the `OpenConsole.exe` in its own directory and silently falls
back to the system's conhost when that file is missing, so write both. Microsoft's MIT notice (`MICROSOFT_LICENSE`) must
accompany the files wherever you ship them.

The flags change meaning between ConPTY versions: 0x8 is `PSEUDOCONSOLE_GLYPH_WIDTH_GRAPHEMES` here and
`PSEUDOCONSOLE_PASSTHROUGH_MODE` in older declarations. Take them from this crate.

## Versions

The crate version is the NuGet version with one revision digit appended: `1.25.2609300030` is package `1.25.260930003`
with revision `0`, and `1.25.2609300031` would be a fix to the crate alone. `VERSION` is the package version. A semver
suffix cannot express the revision: `-1` is a prerelease and `+1` is ignored by crates.io.

Depend on an exact version, `conpty-bin-vendored = "=1.25.2609300030"`: Cargo treats every `1.x` release as compatible,
but each one embeds another ConPTY, whose flags and behavior can differ.

## What every release is checked for

`tools/check` runs before each publish, and CI runs it on every change:

- the assets are exactly the files nuget.org serves for `VERSION`;
- each binary has a valid Authenticode signature by Microsoft Corporation, chained to the Microsoft roots in
  `tools/microsoft-roots.pem`, and a file version equal to `VERSION`;
- every exported name and function type matches `conpty.h`, and every flag matches its `#define`;
- the published archive contains exactly these assets.

On Windows, a test also loads the embedded DLL, creates, resizes and closes a pseudoconsole through the crate's names
and types, and requires the bundled `OpenConsole.exe` to be its host.

The archive crates.io serves after each publish has a GitHub artifact attestation:

```sh
curl -fsSLo conpty-bin-vendored.crate https://static.crates.io/crates/conpty-bin-vendored/conpty-bin-vendored-1.25.2609300030.crate
gh attestation verify conpty-bin-vendored.crate --repo felis-terminal/conpty-bin-vendored
```

## License

The crate is MIT (`LICENSE`). The embedded files are Microsoft's, under the MIT license in `LICENSE-MICROSOFT`.
