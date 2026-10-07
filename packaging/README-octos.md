# Standalone assistant runtime

Standalone desktop and Android packages ship one Octos executable owned by
Rinx. `packaging/octos.lock.json` pins its source and features. The helper
refuses to package a revision different from `octos-cli` in `Cargo.lock`.
Update the System Apps dependency, lockfile and packaging pin together.

OctoSense module builds do not use this packaging step. They link Rinx with
`default-features = false, features = ["octosense-module"]` and inject the
shell's scoped app-peer service. Mini apps open contexts on that peer; they
do not receive kernels of their own.

## Desktop

The existing cargo-packager hook builds Rinx, then runs:

```sh
python3 tools/package-octos.py desktop
```

This builds the pinned kernel with `--locked --no-default-features --features
api,git,ast`, stages `dist/runtime/octos-<target>`, its SHA-256 receipt and license,
and cargo-packager installs it beside Rinx. Runtime staging stays below
`dist/runtime/` so the release uploader cannot mistake a Windows kernel `.exe`
for a Rinx installer. macOS signs the nested kernel
before signing and notarizing the app. For cross-packaging, set
`RINX_KERNEL_TARGET` to the same target passed to cargo-packager; the Rust
target and platform linker must already be installed.

For a development build:

```sh
cargo build --profile fast
python3 tools/package-octos.py desktop --app-binary target/fast/rinx
cargo run --profile fast
```

The macOS development runner copies and signs the adjacent kernel inside
its development bundle. `--kernel <path>` can reuse a native executable
whose version reports the pinned revision. It is not a cross-build option.

## Windows

Install Python 3 and the build prerequisites in the [build guide](../README.md#build-and-run).
The desktop helper builds the same pinned kernel for the host target. On x86-64 MSVC, the
artifact it reports under `dist/runtime/` is
`octos-x86_64-pc-windows-msvc.exe`. Rinx discovers it as **`octos.exe` beside
`rinx.exe`** (or at `RINX_OCTOS_BIN`), so a development staging copies it to
that name:

```powershell
cargo build --profile fast
python3 tools/package-octos.py desktop --app-binary target/fast/rinx.exe
cargo run --profile fast
```

Use `python` instead of `python3` if that is your Python 3 command. Adjust
`target/fast` if you use another Cargo profile or a custom `CARGO_TARGET_DIR`.
The helper does not sign the Windows executable. The executable Cargo builds is `rinx.exe`: for
`cargo run` to find the kernel it has to end up beside it, in the same target
directory. Runtime staging stays below `dist/runtime/` here too, for the same
reason as on macOS.

The development kernel can open a console window and take focus; click the Rinx
window if typing no longer reaches it.

## Android

Use the pinned cargo-makepad and a **full** NDK (its minimal installation
does not contain the CMake metadata needed by Rinx's native dependencies).
CMake and a working native build tool must be on `PATH`.

```sh
cargo makepad android install-toolchain --full-ndk
rustup target add aarch64-linux-android
python3 tools/package-octos.py android -- \
  cargo makepad android --abi=aarch64 build -p rinx --release --locked
```

With an explicitly installed SDK, pass its path both to the helper's
`--sdk` and cargo-makepad's `--sdk-path`. Otherwise the helper resolves
cargo-makepad's default SDK from the pinned Makepad checkout.

The helper checks that the kernel is an aarch64 ELF PIE executable and
passes it as `MAKEPAD_ANDROID_EXTRA_LIBS=liboctos.so=<path>`. Android executes
it from the APK's native library directory, rather than Rinx's writable
data directory. Android release CI stages the same binary before packaging.

These packaging steps cover desktop and Android. They do not provide a
local subprocess kernel on iOS.

## Validation

```sh
python3 tools/test_package_octos.py
RINX_TEST_OCTOS_BIN=/absolute/path/to/octos cargo test --locked \
  --test standalone_local --test standalone_remote -- --test-threads=1
```

To verify production discovery, copy the `standalone_local` test executable
and the packaged kernel into a private directory beside one another (kernel
name `octos`, or `octos.exe` on Windows), then run the test with
`RINX_TEST_PACKAGED_OCTOS=1` as well. This clears `RINX_OCTOS_BIN` and exercises
Rinx's normal adjacent-executable lookup. The process tests currently use
Unix process inspection and are run on macOS; this is not a Windows test claim.

Keep real-device installation and signed-in native UI evidence separate from
the process tests. A successful cross-build alone does not verify phone behavior.
