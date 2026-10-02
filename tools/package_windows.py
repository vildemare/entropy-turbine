"""Zip a Windows build with the assets Bevy loads at runtime.

Usage: python3 tools/package_windows.py path/to/entropy-turbine.exe [output.zip]
"""

from pathlib import Path
from sys import argv
from zipfile import ZIP_DEFLATED, ZipFile

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = "Entropy-Turbine"


def main() -> None:
    if len(argv) not in (2, 3):
        raise SystemExit(__doc__)
    executable = Path(argv[1]).resolve()
    if not executable.is_file() or executable.suffix.lower() != ".exe":
        raise SystemExit(f"Windows executable not found: {executable}")
    with executable.open("rb") as binary:
        if binary.read(2) != b"MZ":
            raise SystemExit(f"Not a Windows executable: {executable}")
        binary.seek(0x3C)
        header_offset = int.from_bytes(binary.read(4), "little")
        binary.seek(header_offset)
        if binary.read(4) != b"PE\0\0":
            raise SystemExit(f"Not a Windows PE executable: {executable}")
    assets = ROOT / "assets"
    if not assets.is_dir():
        raise SystemExit(f"Assets directory not found: {assets}")
    output = Path(argv[2]).resolve() if len(argv) == 3 else ROOT / "dist" / "entropy-turbine-windows.zip"
    output.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(output, "w", ZIP_DEFLATED, compresslevel=6) as archive:
        archive.write(executable, f"{PACKAGE}/entropy-turbine.exe")
        for path in sorted(assets.rglob("*")):
            if path.is_file():
                archive.write(path, f"{PACKAGE}/assets/{path.relative_to(assets).as_posix()}")
        archive.writestr(
            f"{PACKAGE}/READ ME.txt",
            "Extract the whole folder, then double-click entropy-turbine.exe.\n"
            "Keep the assets folder beside the executable.\n",
        )
    print(f"Created {output}")


if __name__ == "__main__":
    main()
