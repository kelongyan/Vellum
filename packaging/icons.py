#!/usr/bin/env python3
"""Regenerate the PdfCraft app icons from assets/app-icon/pdfcraft-logo.png.

This fork's app-icon master is a raster (pdfcraft-logo.png), so every derived file is
rendered with Pillow instead of upstream's resvg pipeline. The Windows PDF file icon is
still rendered from pdfcraft-document.svg when resvg is on PATH; otherwise the committed
pdfcraft-document.ico is left untouched.

Needs Python 3 and Pillow (`pip install pillow`). The .icns is written here, so iconutil
(macOS only) is no longer required. The outputs are committed, so building and packaging
never need these tools.

    python3 packaging/icons.py
"""

import base64
import io
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
DIR = ROOT / "assets" / "app-icon"
ID = "ai.storyteller.pdfcraft"

# Linux hicolor theme sizes; 256 is also the runtime window icon on Windows and Linux.
HICOLOR = (16, 24, 32, 48, 64, 128, 256, 512)
# Windows .ico entries (PNG-compressed), as upstream's icons.sh wrote them.
ICO_SIZES = (16, 20, 24, 32, 40, 48, 64, 128, 256)
# macOS .icns entries. The icXX types are the slots iconutil writes (icp4-icp6 are the
# legacy 16/32/64 ones; ic11-ic14 are the @2x duplicates).
ICNS = (
    ("ic04", 16),
    ("ic05", 32),
    ("ic07", 128),
    ("ic08", 256),
    ("ic09", 512),
    ("ic10", 1024),
    ("ic11", 32),
    ("ic12", 64),
    ("ic13", 256),
    ("ic14", 512),
)

# Apple's icon grid: an 824 px body centred on a transparent 1024 px canvas.
MAC_BODY = 824.0
MAC_CANVAS = 1024.0


def main() -> int:
    master = Image.open(DIR / "pdfcraft-logo.png").convert("RGBA")

    def full_bleed(size: int) -> Image.Image:
        return master.resize((size, size), Image.LANCZOS)

    def mac_grid(size: int) -> Image.Image:
        body = round(MAC_BODY / MAC_CANVAS * size)
        offset = round((MAC_CANVAS - MAC_BODY) / 2.0 / MAC_CANVAS * size)
        canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        canvas.alpha_composite(master.resize((body, body), Image.LANCZOS), (offset, offset))
        return canvas

    # 1024 px PNG on Apple's grid (also the runtime Dock icon on macOS).
    mac_grid(1024).save(DIR / "pdfcraft-1024.png", "PNG", optimize=True)

    # Linux hicolor theme (full bleed).
    for size in HICOLOR:
        path = DIR / "hicolor" / f"{size}x{size}" / "apps" / f"{ID}.png"
        path.parent.mkdir(parents=True, exist_ok=True)
        full_bleed(size).save(path, "PNG", optimize=True)

    # Linux scalable icon: a self-contained SVG holding a 512 px raster, so a copy installed
    # under /usr/share/icons never needs a sibling file.
    blob = png_blob(full_bleed(512))
    scalable = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">\n'
        f'<image href="data:image/png;base64,{base64.b64encode(blob).decode("ascii")}" width="512" height="512"/>\n'
        "</svg>\n"
    )
    path = DIR / "hicolor" / "scalable" / "apps" / f"{ID}.svg"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(scalable, encoding="utf-8", newline="\n")

    # Windows .ico: the app icon remains resource 1; the document icon has resource 2.
    write_ico(DIR / "pdfcraft.ico", [png_blob(full_bleed(size)) for size in ICO_SIZES])

    # macOS .icns.
    entries = b"".join(t.encode("ascii") + struct.pack(">I", len(data) + 8) + data for t, data in
                       ((t, png_blob(mac_grid(size))) for t, size in ICNS))
    (DIR / "pdfcraft.icns").write_bytes(b"icns" + struct.pack(">I", len(entries) + 8) + entries)

    if shutil.which("resvg"):
        write_document_icon()
    else:
        print("icons: resvg not found; pdfcraft-document.ico left as it is", file=sys.stderr)
    print(f"icons written to {DIR}")
    return 0


def png_blob(image: Image.Image) -> bytes:
    out = io.BytesIO()
    image.save(out, "PNG", optimize=True)
    return out.getvalue()


def write_ico(path: Path, blobs: list[bytes]) -> None:
    head = struct.pack("<HHH", 0, 1, len(blobs))
    entries, data, offset = b"", b"", 6 + 16 * len(blobs)
    for blob in blobs:
        # IHDR width/height are the 8 bytes after the 16-byte PNG signature + chunk header.
        width, height = struct.unpack(">II", blob[16:24])
        entries += struct.pack("<BBBBHHII", width % 256, height % 256, 0, 0, 1, 32, len(blob), offset)
        data += blob
        offset += len(blob)
    path.write_bytes(head + entries + data)


def write_document_icon() -> None:
    svg = DIR / "pdfcraft-document.svg"
    blobs: list[bytes] = []
    with tempfile.TemporaryDirectory() as tmp:
        for size in ICO_SIZES:
            out = Path(tmp) / f"{size}.png"
            subprocess.run(
                ["resvg", "--skip-system-fonts", "-w", str(size), "-h", str(size), str(svg), str(out)],
                check=True,
            )
            blobs.append(png_blob(Image.open(out).convert("RGBA")))
    write_ico(DIR / "pdfcraft-document.ico", blobs)


if __name__ == "__main__":
    sys.exit(main())
