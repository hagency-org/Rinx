#!/usr/bin/env python3
"""Build and package Rinx's pinned, independently owned Octos runtime.

Desktop: python3 tools/package-octos.py desktop [--target <triple>]
Android: python3 tools/package-octos.py android --sdk <makepad-sdk> -- \
    cargo makepad android --sdk-path=<makepad-sdk> build -p rinx --release

Desktop output uses cargo-packager's external-binary naming convention.
Android commands receive MAKEPAD_ANDROID_EXTRA_LIBS=liboctos.so=<binary>.
The checkout and build outputs stay under this repository's target directory.
No installed kernel, provider settings, or app data is read or modified.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
ANDROID = "aarch64-linux-android"


def read_lock(root=ROOT):
    lock = json.loads((root / "packaging/octos.lock.json").read_text())
    if (lock.get("schema_version") != 1
            or lock.get("url") != "https://github.com/octos-org/octos.git"
            or not re.fullmatch(r"[0-9a-f]{40}", lock.get("revision", ""))
            or lock.get("features") != ["api", "git", "ast"]):
        raise ValueError("Invalid pinned Octos runtime lock")
    revisions = set(re.findall(
        r'git\+https://github\.com/octos-org/octos\.git\?rev=([0-9a-f]{40})#',
        (root / "Cargo.lock").read_text()))
    # nix may come from a separate historical vendor revision. Only the
    # kernel crate's source determines the runtime contract.
    cli = re.search(r'name = "octos-cli"\nversion = "[^\"]+"\nsource = "git\+https://github\.com/octos-org/octos\.git\?rev=([0-9a-f]{40})#',
                    (root / "Cargo.lock").read_text())
    if not cli or cli[1] != lock["revision"] or lock["revision"] not in revisions:
        raise ValueError("Octos packaging pin differs from Cargo.lock; update them together")
    return lock


def host_target():
    text = subprocess.check_output(["rustc", "-vV"], text=True)
    return next(line.split(": ", 1)[1] for line in text.splitlines() if line.startswith("host: "))


def matches_revision(version, revision):
    # Git chooses the abbreviation length from the checkout's object set:
    # a shallow build commonly reports seven characters, a full clone more.
    match = re.fullmatch(r"octos \S+ \(([0-9a-f]{7,40}) \d{4}-\d{2}-\d{2}\)", version.strip())
    return bool(match and revision.startswith(match[1]))


def prepare_source(lock, work, offline=False):
    source = work / "src"
    if not (source / ".git").exists():
        source.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "init", "--quiet", str(source)], check=True)
    def git(*args, **kwargs):
        return subprocess.run(["git", "-C", str(source), *args], **kwargs)
    dirty = git("status", "--porcelain", capture_output=True, text=True, check=True).stdout
    if dirty:
        raise RuntimeError("Preserving modified kernel checkout: " + str(source))
    have = git("cat-file", "-e", lock["revision"] + "^{commit}", capture_output=True)
    if have.returncode:
        if offline:
            raise RuntimeError("Pinned kernel is not cached; prepare it once without --offline")
        git("fetch", "--quiet", "--depth=1", "--no-tags", lock["url"], lock["revision"], check=True)
    git("checkout", "--quiet", "--detach", lock["revision"], check=True)
    return source


def android_env(sdk):
    bins = list(sdk.glob("ndk/*/toolchains/llvm/prebuilt/*/bin"))
    if not bins:
        raise ValueError("No NDK found under " + str(sdk / "ndk"))
    bins.sort(key=lambda p: tuple(int(n) for n in re.findall(r"\d+", str(p.relative_to(sdk)))))
    ndk = bins[-1]
    clang = ndk / (ANDROID + "33-clang")
    if os.name == "nt":
        clang = clang.with_suffix(".cmd")
    suffix = ".exe" if os.name == "nt" else ""
    if not clang.is_file():
        raise ValueError("Missing Android API 33 compiler: " + str(clang))
    return {
        # CMake-based dependencies need the NDK root as well as cc's compiler
        # wrappers when cross-compiling the independently packaged kernel.
        "ANDROID_NDK_ROOT": str(ndk.parents[4]),
        "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER": str(clang),
        "CARGO_TARGET_AARCH64_LINUX_ANDROID_AR": str(ndk / ("llvm-ar" + suffix)),
        "CC_aarch64_linux_android": str(clang),
        "CXX_aarch64_linux_android": str(clang).replace("-clang", "-clang++"),
        "AR_aarch64_linux_android": str(ndk / ("llvm-ar" + suffix)),
        "RANLIB_aarch64_linux_android": str(ndk / ("llvm-ranlib" + suffix)),
    }


def default_android_sdk():
    """Find cargo-makepad's default SDK in the pinned Makepad checkout."""
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version=1"], cwd=ROOT, text=True))
    platform = next(p for p in metadata["packages"] if p["name"] == "makepad-platform")
    makepad = Path(platform["manifest_path"]).parent.parent
    host = host_target()
    if "apple" in host:
        name = "macos_aarch64" if host.startswith("aarch64") else "macos_x64"
    elif "windows" in host:
        name = "windows_x64"
    else:
        name = "linux_x64"
    return makepad / "tools/cargo_makepad" / ("android_33_" + name)


def build_kernel(source, lock, work, target, sdk=None, offline=False):
    env = os.environ.copy()
    # Rinx's package/resource flags must not leak into the independent kernel.
    env.pop("MAKEPAD", None)
    env.pop("MAKEPAD_PACKAGE_DIR", None)
    env.pop("RUSTFLAGS", None)
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    env["CARGO_TARGET_DIR"] = str(work / "build")
    if target == ANDROID:
        if sdk is None:
            sdk = default_android_sdk()
        env.update(android_env(sdk))
    args = ["cargo", "build", "--locked", "--release", "--target", target,
            "-p", "octos-cli", "--bin", "octos", "--no-default-features",
            "--features", ",".join(lock["features"])]
    if offline:
        args.append("--offline")
    subprocess.run(args, cwd=source, env=env, check=True)
    return work / "build" / target / "release" / ("octos.exe" if "windows" in target else "octos")


def stage(kernel, lock, target, source, out=ROOT / "dist/runtime", prebuilt=False):
    # Android executes this PIE binary out of nativeLibraryDir. Packaging a
    # host binary under liboctos.so would install successfully but fail at run.
    if target == ANDROID:
        with kernel.open("rb") as stream:
            header = stream.read(20)
        if (len(header) != 20 or header[:6] != b"\x7fELF\x02\x01"
                or struct.unpack_from("<HH", header, 16) != (3, 183)):
            raise ValueError("Android kernel must be an aarch64 ELF PIE executable")
    out.mkdir(parents=True, exist_ok=True)
    name = "octos-" + target + (".exe" if "windows" in target else "")
    destination = out / name
    shutil.copy2(kernel, destination)
    if os.name != "nt":
        destination.chmod(0o755)
    digest = hashlib.sha256(destination.read_bytes()).hexdigest()
    (out / (name + ".json")).write_text(json.dumps({
        "revision": lock["revision"], "target": target, "sha256": digest,
        "source": "prebuilt-verified-version" if prebuilt else "pinned-source",
        "features": None if prebuilt else lock["features"],
    }, indent=2) + "\n")
    licenses = out / "licenses"
    licenses.mkdir(exist_ok=True)
    shutil.copy2(source / "LICENSE", licenses / "octos-LICENSE")
    for name in ("NOTICE", "NOTICE.md"):
        if (source / name).is_file():
            shutil.copy2(source / name, licenses / "octos-NOTICE")
    return destination


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    command = []
    if "--" in argv:
        split = argv.index("--")
        argv, command = argv[:split], argv[split + 1:]
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("mode", choices=["desktop", "android"])
    parser.add_argument("--target", default=os.environ.get("RINX_KERNEL_TARGET") or os.environ.get("CARGO_BUILD_TARGET"))
    parser.add_argument("--sdk", type=Path)
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--kernel", type=Path, help="Reuse a native prebuilt kernel after verifying its pinned version")
    parser.add_argument("--app-binary", type=Path, help="Also stage beside this development executable")
    args = parser.parse_args(argv)
    target = args.target or (ANDROID if args.mode == "android" else host_target())
    if args.mode == "android" and target != ANDROID:
        parser.error("Android packages currently target aarch64-linux-android")
    lock = read_lock()
    work = ROOT / "target/octos-runtime"
    source = prepare_source(lock, work, args.offline)
    if args.kernel:
        if target != host_target():
            parser.error("Prebuilt reuse is limited to the native target; cross builds must use the pinned source")
        kernel = args.kernel.resolve(strict=True)
        version = subprocess.check_output([str(kernel), "--version"], text=True)
        if not matches_revision(version, lock["revision"]):
            parser.error("Prebuilt kernel does not report the pinned revision")
    else:
        kernel = build_kernel(source, lock, work, target, args.sdk, args.offline)
    packaged = stage(kernel, lock, target, source, prebuilt=bool(args.kernel))
    if args.app_binary:
        destination = args.app_binary.resolve().parent / ("octos.exe" if "windows" in target else "octos")
        shutil.copy2(packaged, destination)
    print("Packaged Octos: " + str(packaged), flush=True)
    if command:
        env = os.environ.copy()
        if args.mode == "android":
            extra = env.get("MAKEPAD_ANDROID_EXTRA_LIBS", "")
            env["MAKEPAD_ANDROID_EXTRA_LIBS"] = (extra + ";" if extra else "") + "liboctos.so=" + str(packaged)
        return subprocess.run(command, cwd=ROOT, env=env).returncode
    return 0


if __name__ == "__main__":
    sys.exit(main())
