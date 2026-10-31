#!/usr/bin/env python3
"""Regenerate deterministic PDF fixtures with an independent PDF producer.

Development only: reportlab==5.0.1 and Pillow==12.3.0. Product builds and
ordinary Rust/browser tests consume the checked-in PDFs without Python.
"""
from io import BytesIO
from pathlib import Path
import random

from PIL import Image, ImageDraw, ImageFont
from reportlab import rl_config
from reportlab.lib.utils import ImageReader
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.pdfgen import canvas

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]
rl_config.useA85 = False
PAGE_SIZE = (500, 420)
ENGLISH = "Independent PDF fixture: searchable text, vectors and links."
KOREAN = "한글 문서 압축 테스트: 텍스트와 글꼴을 보존합니다."


def pixels(mode, width, height):
    rng = random.Random(62718)
    components = 3 if mode == "RGB" else 1
    data = bytes(
        min(255, max(0, int(x / width * 180 + 30) + rng.randrange(-20, 21)))
        for _y in range(height)
        for x in range(width)
        for _component in range(components)
    )
    return Image.frombytes(mode, (width, height), data)


def jpeg(image):
    content = BytesIO()
    image.save(content, format="JPEG", quality=100, subsampling=0)
    content.seek(0)
    return ImageReader(content)


def document(name):
    pdf = canvas.Canvas(
        str(ROOT / name), pagesize=PAGE_SIZE, pageCompression=1,
        pdfVersion=(1, 4), invariant=1,
    )
    pdf.setTitle(name)
    return pdf


def heading(pdf, text=ENGLISH, font="Helvetica"):
    pdf.setFont(font, 12)
    pdf.drawString(30, 385, text)
    pdf.setStrokeColorRGB(0, 0, 1)
    pdf.rect(28, 78, 444, 285, stroke=1, fill=0)
    pdf.linkURL("https://example.com/pdf-fixture", (30, 375, 450, 400))


def image_document(name, source):
    pdf = document(name)
    for _page in range(2):
        heading(pdf)
        pdf.drawImage(source, 30, 80, width=440, height=280)
        pdf.showPage()
    pdf.save()


image_document("rgb_jpeg.pdf", jpeg(pixels("RGB", 600, 400)))
image_document("rgb_flate.pdf", ImageReader(pixels("RGB", 300, 200)))
image_document("gray_jpeg.pdf", jpeg(pixels("L", 600, 400)))

# D2Coding is already distributed in UPeg with its OFL notices. ReportLab
# embeds a subset and a ToUnicode map; no extra font is vendored for tests.
pdfmetrics.registerFont(TTFont("D2CodingFixture", str(REPO / "flutter_app/fonts/D2Coding.ttf")))
pdf = document("mixed_hangul.pdf")
heading(pdf, KOREAN, "D2CodingFixture")
pdf.drawImage(jpeg(pixels("RGB", 600, 400)), 30, 80, width=440, height=280)
pdf.showPage()

# An actual raster page: small text is drawn into an image, without a PDF
# text layer. The JPEG encoder/resize path must leave it renderable.
scan = Image.new("L", (800, 600), "white")
draw = ImageDraw.Draw(scan)
font = ImageFont.truetype(str(REPO / "flutter_app/fonts/JetBrainsMono-Regular.ttf"), 16)
for row in range(28):
    draw.text((20, 15 + row * 20), f"Scan row {row:02d}: small text and 0123456789", font=font, fill=20)
pdf.drawImage(jpeg(scan), 0, 0, width=500, height=420)
pdf.showPage()
heading(pdf, KOREAN, "D2CodingFixture")
pdf.drawImage(ImageReader(pixels("RGB", 300, 200)), 30, 80, width=440, height=280)
pdf.showPage()
pdf.save()

for path in sorted(ROOT.glob("*.pdf")):
    print(f"{path.name}: {path.stat().st_size} bytes")
