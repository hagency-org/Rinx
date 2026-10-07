"""Collect the aligned APK from the pinned Makepad tool, checking its runtime."""
from pathlib import Path
import shutil
import json
import subprocess
import zipfile

root = Path(__file__).resolve().parents[1]
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"], cwd=root))
version = next(package["version"] for package in metadata["packages"] if package["name"] == "rinx")
candidates = [
    root / base / "makepad-android-apk/rinx/apk/rinx.apk"
    for base in ("target", "target/android", "android")
]
found = [p for p in candidates if p.is_file()]
if len(found) != 1:
    raise SystemExit(f"Expected one aligned Rinx APK; found {found}")
with zipfile.ZipFile(found[0]) as apk:
    if "lib/arm64-v8a/liboctos.so" not in apk.namelist():
        raise SystemExit("APK is missing its standalone Octos runtime")
output = root / "dist" / f"Rinx-{version}-android-aarch64.apk"
output.parent.mkdir(exist_ok=True)
shutil.copy2(found[0], output)
print(output)
