#!/usr/bin/env python3
"""Stage and build one XSuite Debian binary package on Debian trixie."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


SCRIPT = Path(__file__).resolve()
STANDALONE_APP = SCRIPT.parents[1] if (SCRIPT.parents[1] / "package.json").is_file() else None
ROOT = STANDALONE_APP.parent if STANDALONE_APP else SCRIPT.parents[2]
PRODUCTS = {
    "XWrite": ("Word processor", "Office;WordProcessor;", "application/vnd.openxmlformats-officedocument.wordprocessingml.document;text/plain;text/markdown;text/html;"),
    "XSheet": ("Spreadsheet editor", "Office;Spreadsheet;", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet;text/csv;text/tab-separated-values;"),
    "XSlide": ("Presentation editor", "Office;Presentation;", "application/vnd.openxmlformats-officedocument.presentationml.presentation;"),
    "XChart": ("Chart editor", "Office;", "text/csv;text/tab-separated-values;"),
    "XGraph": ("Vector drawing editor", "Graphics;2DGraphics;VectorGraphics;", "application/vnd.oasis.opendocument.graphics;image/svg+xml;"),
    "XMath": ("Formula editor", "Office;", "application/vnd.oasis.opendocument.formula;application/mathml+xml;text/x-tex;"),
}


def put_text(path: Path, content: str, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    path.chmod(mode)


def copy_file(source: Path, target: Path, mode: int = 0o644) -> None:
    if not source.is_file():
        raise FileNotFoundError(f"Build artifact missing: {source}")
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)
    target.chmod(mode)


def library_dependencies(host: Path, backend: Path, stage: Path) -> str:
    if not shutil.which("dpkg-shlibdeps"):
        raise RuntimeError("dpkg-shlibdeps is required to build the Debian package")
    # dpkg-shlibdeps reads the host system's shlibs metadata. Run this on the
    # target Debian release, with binaries built on that same release.
    debian = stage / "debian"
    debian.mkdir(exist_ok=True)
    put_text(debian / "control", "Source: xsuite\nSection: editors\nPriority: optional\nMaintainer: WaylonJBrown <waylonjacebrown@gmail.com>\nStandards-Version: 4.7.0\n\nPackage: xsuite-temporary\nArchitecture: any\nDescription: temporary dependency scan\n")
    result = subprocess.run(
        ["dpkg-shlibdeps", "-O", "-e" + str(host), "-e" + str(backend)],
        cwd=stage, text=True, capture_output=True, check=True,
    )
    line = next((line for line in result.stdout.splitlines() if line.startswith("shlibs:Depends=")), "")
    if not line:
        raise RuntimeError(f"dpkg-shlibdeps returned no dependencies: {result.stderr}")
    shutil.rmtree(debian)
    deps = line.split("=", 1)[1]
    if "libwebkit2gtk-4.1-0" not in deps:
        deps += ", libwebkit2gtk-4.1-0"
    return deps


def stage_product(name: str, stage: Path, architecture: str, scan_deps: bool, maintainer: str) -> str:
    purpose, categories, mime = PRODUCTS[name]
    slug = name.lower()
    source = STANDALONE_APP or ROOT / name
    version = json.loads((source / "package.json").read_text())["version"]
    app_root = stage / "usr" / "lib" / "xsuite" / slug
    host = app_root / f"{slug}-native"
    backend = app_root / "backend" / slug
    copy_file(source / "native-host" / "target" / "release" / f"{slug}-native", host, 0o755)
    copy_file(source / "target" / "release" / slug, backend, 0o755)
    shutil.copytree(source / "static", app_root / "static")
    copy_file(source / "build" / "icon.png", app_root / "build" / "icon.png")
    copy_file(source / "build" / "icon.png", stage / "usr" / "share" / "icons" / "hicolor" / "256x256" / "apps" / f"{slug}.png")
    logo = source / "static" / "assets" / "logo.svg"
    if logo.is_file():
        copy_file(logo, stage / "usr" / "share" / "icons" / "hicolor" / "scalable" / "apps" / f"{slug}.svg")
    (stage / "usr" / "bin").mkdir(parents=True, exist_ok=True)
    (stage / "usr" / "bin" / slug).symlink_to(f"../lib/xsuite/{slug}/{slug}-native")
    put_text(stage / "usr" / "share" / "applications" / f"{slug}.desktop", (
        "[Desktop Entry]\nType=Application\nVersion=1.0\n"
        f"Name={name}\nComment={purpose}\nExec=/usr/bin/{slug} %f\nIcon={slug}\n"
        f"Categories={categories}\nMimeType={mime}\nTerminal=false\nStartupNotify=true\n"
    ))
    copy_file(source / "LICENSE", stage / "usr" / "share" / "doc" / slug / "copyright")
    copyright_path = stage / "usr" / "share" / "doc" / slug / "copyright"
    with copyright_path.open("a", encoding="utf-8") as notices:
        for title, filename in (
            ("Inter variable font", "Inter-LICENSE.txt"),
            ("Phosphor icons", "phosphor-LICENSE.txt"),
        ):
            notices.write(f"\n\n{title}\n{'=' * len(title)}\n")
            notices.write((source / "static" / "assets" / filename).read_text(encoding="utf-8"))
    copy_file(source / "README.md", stage / "usr" / "share" / "doc" / slug / "README")
    deps = library_dependencies(host, backend, stage) if scan_deps else "libwebkit2gtk-4.1-0"
    control = (
        f"Package: {slug}\nVersion: {version}-1\nSection: editors\nPriority: optional\n"
        f"Architecture: {architecture}\nMaintainer: {maintainer}\n"
        f"Depends: {deps}\nDescription: {purpose} for XSuite\n"
        f" {name} is a local desktop editor with a Rust backend and a native WebKitGTK window.\n"
    )
    put_text(stage / "DEBIAN" / "control", control)
    checksums = []
    for path in sorted((stage / "usr").rglob("*")):
        if path.is_file() and not path.is_symlink():
            checksums.append(f"{hashlib.md5(path.read_bytes()).hexdigest()}  {path.relative_to(stage)}")
    put_text(stage / "DEBIAN" / "md5sums", "\n".join(checksums) + "\n")
    return version


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("product", choices=([STANDALONE_APP.name] if STANDALONE_APP else [*PRODUCTS, "all"]))
    parser.add_argument("--output-dir", type=Path, default=(STANDALONE_APP or ROOT) / "dist" / "debian")
    parser.add_argument("--stage-only", action="store_true", help="validate layout without creating a .deb")
    parser.add_argument("--maintainer", default=os.environ.get("DEB_MAINTAINER", "WaylonJBrown <waylonjacebrown@gmail.com>"), help="Debian control field, e.g. 'Name <email>'")
    args = parser.parse_args()
    if not args.stage_only and not shutil.which("dpkg-deb"):
        parser.error("dpkg-deb is required; build on Debian trixie")
    if not args.stage_only:
        release = Path("/etc/os-release").read_text()
        if "ID=debian" not in release or "VERSION_CODENAME=trixie" not in release:
            parser.error("build Debian packages inside Debian trixie")
    architecture = subprocess.check_output(["dpkg", "--print-architecture"], text=True).strip() if shutil.which("dpkg") else "amd64"
    names = PRODUCTS if args.product == "all" else [args.product]
    args.output_dir.mkdir(parents=True, exist_ok=True)
    for name in names:
        with tempfile.TemporaryDirectory(prefix=f"{name.lower()}-deb-") as temp:
            stage = Path(temp) / "pkg"
            version = stage_product(name, stage, architecture, not args.stage_only,
                                    args.maintainer or "XSuite Test Build <noreply@example.invalid>")
            slug = name.lower()
            desktop = stage / "usr" / "share" / "applications" / f"{slug}.desktop"
            if shutil.which("desktop-file-validate"):
                subprocess.run(["desktop-file-validate", str(desktop)], check=True)
            if args.stage_only:
                print(f"Staged {name}: {version}")
            else:
                output = args.output_dir / f"{slug}_{version}-1_{architecture}.deb"
                subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(stage), str(output)], check=True)
                print(output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
