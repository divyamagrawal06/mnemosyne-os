"""Package the tested release ISO, Python runner, documentation, and licenses."""
import argparse
from datetime import datetime, timezone
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile
import dev


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", action="store_true", help="build the release ISO before packaging")
    parser.add_argument("--desktop", action="store_true", help="package the native framebuffer desktop instead of the web/serial demo")
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    args.out = args.out or dev.BUILD / ("mnemosyne-desktop-demo.tar.gz" if args.desktop else "mnemosyne-os-demo.tar.gz")
    iso = (dev.image(True, desktop=True) if args.desktop else dev.image(True, shell=True)) if args.build else dev.BUILD / ("mnemosyne-desktop-release.iso" if args.desktop else "mnemosyne-os-release.iso")
    if not iso.is_file():
        parser.error("release ISO missing; run guest-test --release first")
    provenance = json.loads(iso.with_suffix(".json").read_text(encoding="utf-8"))
    if provenance.get("sha256") != hashlib.sha256(iso.read_bytes()).hexdigest():
        parser.error("ISO does not match its build provenance; rebuild")
    mode = "guest-desktop" if args.desktop else "guest-shell"
    if provenance.get("profile") != "release" or provenance.get(mode) is not True:
        parser.error("expected the matching release ISO")
    files = {"build/" + iso.name: iso, "build/" + iso.with_suffix(".json").name: iso.with_suffix(".json")}
    names = (["tools/run-desktop.py", "tools/test-desktop.py", "deployment/start-desktop.sh",
              "docs/ec2-desktop-demo.md", "LICENSE", "THIRD_PARTY.md"] if args.desktop else
             ["tools/dev.py", "tools/run-image.py", "tools/ai-plan.py", "tools/dashboard.py",
                 "tools/guest-controller.py", "tools/guest-e2e.py", "README.md", "LICENSE", "THIRD_PARTY.md",
                 "docs/raspberry-pi-demo.md", "docs/cloud-demo.md", "docs/professor-brief.md",
                 "docs/implementation-status.md", "docs/end-to-end-status.md"])
    for name in names:
        files[name] = dev.ROOT / name
    for directory in (() if args.desktop else ("ui", "deployment", "presentation/evidence")):
        for path in (dev.ROOT / directory).rglob("*"):
            if path.is_file() and "__pycache__" not in path.parts and path.suffix != ".pyc":
                files[path.relative_to(dev.ROOT).as_posix()] = path
    if not args.desktop:
        for path in (dev.ROOT / "docs/evidence").glob("*"):
            files["docs/evidence/" + path.name] = path
    for path in (dev.ROOT / "third-party/licenses").glob("*"):
        files["licenses/project/" + path.name] = path
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=dev.ROOT))
    dependencies = []
    for package in metadata["packages"]:
        if not package.get("source"):
            continue
        base = Path(package["manifest_path"]).parent
        label = package["name"] + "-" + package["version"]
        notices = sorted({p for pattern in ("LICENSE*", "LICENCE*", "COPYING*", "NOTICE*") for p in base.glob(pattern) if p.is_file()})
        paths = []
        for notice in notices:
            destination = f"licenses/dependencies/{label}/{notice.name}"
            files[destination] = notice
            paths.append(destination)
        if not notices:
            if package["name"] in {"wasmi", "wasmi_core", "wasmi_ir", "wasmi_collections"}:
                supplemental = ["wasmi-mit.txt", "wasmi-apache.txt"]
            elif package["name"] in {"wat", "wast", "wasm-encoder", "wasmparser"}:
                supplemental = ["wasm-tools-mit.txt", "wasm-tools-apache.txt", "wasm-tools-llvm-exception.txt"]
            elif package["name"] == "pic8259":
                supplemental = ["pic8259-readme.md", "apache-2.0.txt"]
            else:
                parser.error(f"Missing dependency license notice: {label}")
            for name in supplemental:
                destination = "licenses/project/" + name
                if destination not in files:
                    parser.error(f"Missing retained upstream notice: {name}")
                paths.append(destination)
        dependencies.append({"package": label, "license": package.get("license"), "notice-files": paths})
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=dev.ROOT, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=dev.ROOT, text=True).strip())
    if provenance.get("source-commit") != revision or provenance.get("source-dirty") or dirty:
        parser.error("package a clean, committed checkout with an ISO built from that commit")
    manifest = {"built-at": datetime.now(timezone.utc).isoformat(), "commit": revision, "dirty": dirty,
                "profile": "release", "architecture": "x86-64", "qemu-vcpus": 1, "qemu-memory-mib": 256,
                "deployment": "64-bit Linux; ARM64 hosts use x86-64 TCG emulation, not native ARM boot",
                "interface": "native guest framebuffer / PS2 keyboard and mouse" if args.desktop else "host web dashboard / guest serial console",
                "files": {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in files.items()},
                "dependency-notices": dependencies}
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(args.out, "w:gz") as archive:
        for name, path in sorted(files.items()):
            archive.add(path, arcname="mnemosyne-os-demo/" + name, recursive=False)
        content = (json.dumps(manifest, indent=2) + "\n").encode()
        info = tarfile.TarInfo("mnemosyne-os-demo/manifest.json")
        info.size = len(content)
        archive.addfile(info, io.BytesIO(content))
    print(args.out)
    print("sha256=" + hashlib.sha256(args.out.read_bytes()).hexdigest())
    print(f"source={revision} dirty={dirty} size={args.out.stat().st_size}")


if __name__ == "__main__":
    main()
