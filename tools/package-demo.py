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
    parser.add_argument("--out", type=Path, default=dev.BUILD / "mnemosyne-os-demo.tar.gz")
    args = parser.parse_args()
    iso = dev.image(True, shell=True) if args.build else dev.BUILD / "mnemosyne-os-release.iso"
    if not iso.is_file():
        parser.error("release ISO missing; run guest-test --release first")
    provenance = json.loads(iso.with_suffix(".json").read_text(encoding="utf-8"))
    if provenance.get("sha256") != hashlib.sha256(iso.read_bytes()).hexdigest():
        parser.error("ISO does not match its build provenance; rebuild")
    if provenance.get("profile") != "release" or provenance.get("guest-shell") is not True:
        parser.error("expected the release guest-console ISO")
    files = {"build/mnemosyne-os-release.iso": iso, "build/mnemosyne-os-release.json": iso.with_suffix(".json")}
    for name in ["tools/dev.py", "tools/run-image.py", "tools/ai-plan.py", "README.md", "LICENSE",
                 "THIRD_PARTY.md", "docs/cloud-demo.md", "docs/professor-brief.md", "docs/implementation-status.md"]:
        files[name] = dev.ROOT / name
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
