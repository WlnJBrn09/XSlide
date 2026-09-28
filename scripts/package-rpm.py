#!/usr/bin/env python3
"""Stage and build one XSuite RPM binary package (Fedora/RHEL/openSUSE)."""

import argparse
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


def stage_payload(name: str, payload: Path) -> str:
    """Stage the package payload under payload/usr/... exactly like the final install tree."""
    purpose, categories, mime = PRODUCTS[name]
    slug = name.lower()
    source = STANDALONE_APP or ROOT / name
    version = json.loads((source / "package.json").read_text())["version"]
    app_root = payload / "usr" / "lib" / "xsuite" / slug
    host = app_root / f"{slug}-native"
    backend = app_root / "backend" / slug
    copy_file(source / "native-host" / "target" / "release" / f"{slug}-native", host, 0o755)
    copy_file(source / "target" / "release" / slug, backend, 0o755)
    shutil.copytree(source / "static", app_root / "static")
    copy_file(source / "build" / "icon.png", app_root / "build" / "icon.png")
    copy_file(source / "build" / "icon.png", payload / "usr" / "share" / "icons" / "hicolor" / "256x256" / "apps" / f"{slug}.png")
    logo = source / "static" / "assets" / "logo.svg"
    if logo.is_file():
        copy_file(logo, payload / "usr" / "share" / "icons" / "hicolor" / "scalable" / "apps" / f"{slug}.svg")
    (payload / "usr" / "bin").mkdir(parents=True, exist_ok=True)
    (payload / "usr" / "bin" / slug).symlink_to(f"../lib/xsuite/{slug}/{slug}-native")
    put_text(payload / "usr" / "share" / "applications" / f"{slug}.desktop", (
        "[Desktop Entry]\nType=Application\nVersion=1.0\n"
        f"Name={name}\nComment={purpose}\nExec=/usr/bin/{slug} %f\nIcon={slug}\n"
        f"Categories={categories}\nMimeType={mime}\nTerminal=false\nStartupNotify=true\n"
    ))
    copy_file(source / "LICENSE", payload / "usr" / "share" / "doc" / slug / "copyright")
    copyright_path = payload / "usr" / "share" / "doc" / slug / "copyright"
    with copyright_path.open("a", encoding="utf-8") as notices:
        for title, filename in (
            ("Inter variable font", "Inter-LICENSE.txt"),
            ("Material Symbols Outlined variable font", "material-symbols-LICENSE.txt"),
        ):
            notices.write(f"\n\n{title}\n{'=' * len(title)}\n")
            notices.write((source / "static" / "assets" / filename).read_text(encoding="utf-8"))
    copy_file(source / "README.md", payload / "usr" / "share" / "doc" / slug / "README")
    return version


def spec_file_list(payload: Path, slug: str) -> str:
    """Build an explicit %files list; symlinks and executables get correct attrs."""
    lines = []
    bin_link = payload / "usr" / "bin" / slug
    lines.append(f"/usr/bin/{slug}")
    lines.append(f"%attr(0755, root, root) /usr/lib/xsuite/{slug}/{slug}-native")
    lines.append(f"%attr(0755, root, root) /usr/lib/xsuite/{slug}/backend/{slug}")
    lines.append(f"/usr/lib/xsuite/{slug}/build/icon.png")
    lines.append(f"/usr/lib/xsuite/{slug}/static")
    lines.append(f"/usr/share/applications/{slug}.desktop")
    lines.append(f"/usr/share/icons/hicolor/256x256/apps/{slug}.png")
    if (payload / "usr" / "share" / "icons" / "hicolor" / "scalable" / "apps" / f"{slug}.svg").is_file():
        lines.append(f"/usr/share/icons/hicolor/scalable/apps/{slug}.svg")
    lines.append(f"%license /usr/share/doc/{slug}/copyright")
    lines.append(f"%doc /usr/share/doc/{slug}/README")
    del bin_link
    return "\n".join(lines) + "\n"


def build_spec(name: str, version: str, payload: Path, release: str) -> str:
    purpose, _categories, _mime = PRODUCTS[name]
    slug = name.lower()
    files = spec_file_list(payload, slug)
    return f"""Name: {slug}
Version: {version}
Release: {release}%{{?dist}}
Summary: {purpose} for XSuite
License: MIT
URL: https://github.com/WaylonJBrown/CognitienceSW
BuildArch: %{{_arch}}
Requires: webkit2gtk4.1
AutoReqProv: yes

%description
{name} is a local desktop editor with a Rust backend and a native
WebKitGTK window. Part of the XSuite office suite.

%install
mkdir -p %{{buildroot}}
cp -a {payload}/usr %{{buildroot}}/

%files
{files}

%post
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -qtf /usr/share/icons/hicolor >/dev/null 2>&1 || :
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications >/dev/null 2>&1 || :
fi

%postun
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -qtf /usr/share/icons/hicolor >/dev/null 2>&1 || :
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications >/dev/null 2>&1 || :
fi

%changelog
* {os.environ.get("RPM_CHANGELOG_DATE", "Mon Jan 01 2024")} {os.environ.get("RPM_MAINTAINER", "XSuite Build <noreply@example.invalid>")} - {version}-{release}
- Automated build
"""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("product", choices=([STANDALONE_APP.name] if STANDALONE_APP else [*PRODUCTS, "all"]))
    parser.add_argument("--output-dir", type=Path, default=(STANDALONE_APP or ROOT) / "dist" / "rpm")
    parser.add_argument("--stage-only", action="store_true", help="validate layout without creating an .rpm")
    parser.add_argument("--release", default="1", help="RPM release number (default: 1)")
    args = parser.parse_args()
    if not args.stage_only and not shutil.which("rpmbuild"):
        parser.error("rpmbuild is required; install the 'rpm-build' (Fedora/RHEL) or 'rpm-build' / 'build' (openSUSE) package")
    names = PRODUCTS if args.product == "all" else [args.product]
    args.output_dir.mkdir(parents=True, exist_ok=True)
    for name in names:
        with tempfile.TemporaryDirectory(prefix=f"{name.lower()}-rpm-") as temp:
            payload = Path(temp) / "payload"
            version = stage_payload(name, payload)
            slug = name.lower()
            desktop = payload / "usr" / "share" / "applications" / f"{slug}.desktop"
            if shutil.which("desktop-file-validate"):
                subprocess.run(["desktop-file-validate", str(desktop)], check=True)
            if args.stage_only:
                print(f"Staged {name}: {version}")
                continue
            spec_text = build_spec(name, version, payload, args.release)
            spec_path = Path(temp) / f"{slug}.spec"
            put_text(spec_path, spec_text)
            topdir = Path(temp) / "rpmbuild"
            for sub in ("BUILD", "RPMS", "SOURCES", "SPECS", "SRPMS"):
                (topdir / sub).mkdir(parents=True, exist_ok=True)
            subprocess.run(
                ["rpmbuild", "--define", f"_topdir {topdir}", "-bb", str(spec_path)],
                check=True,
            )
            built = list(topdir.rglob("*.rpm"))
            if not built:
                raise RuntimeError("rpmbuild produced no .rpm file")
            for rpm in built:
                output = args.output_dir / rpm.name
                shutil.copy2(rpm, output)
                print(output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
