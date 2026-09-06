"""Generates the app icon set. Run: python3 resources/make-icons.py
Two overlapping rounded squares (swap) on an indigo tile; tray variant is a flat glyph."""
from PIL import Image, ImageDraw
import os
here = os.path.dirname(os.path.abspath(__file__))

def rounded(draw, box, r, fill):
    draw.rounded_rectangle(box, radius=r, fill=fill)

def app_icon(size):
    s = size * 4  # supersample
    img = Image.new('RGBA', (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    pad = s * 0.06
    rounded(d, (pad, pad, s - pad, s - pad), s * 0.22, (94, 106, 210, 255))  # indigo tile
    # two cards, the back one lighter
    w = s * 0.42
    h = s * 0.30
    r = s * 0.06
    x1, y1 = s * 0.20, s * 0.24
    x2, y2 = s * 0.38, s * 0.46
    rounded(d, (x1, y1, x1 + w, y1 + h), r, (255, 255, 255, 80))
    rounded(d, (x2, y2, x2 + w, y2 + h), r, (255, 255, 255, 255))
    # arrow-ish notch on the front card (active dot)
    d.ellipse((x2 + w * 0.12, y2 + h * 0.34, x2 + w * 0.12 + h * 0.32, y2 + h * 0.66), fill=(94, 106, 210, 255))
    return img.resize((size, size), Image.LANCZOS)

def tray_icon(size, fill, outline=None):
    """White glyph with a dark outline stays visible on a light *and* a dark tray."""
    s = size * 4
    img = Image.new('RGBA', (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    w, h, r = s * 0.60, s * 0.42, s * 0.10
    x1, y1 = s * 0.07, s * 0.11
    x2, y2 = s * 0.33, s * 0.47
    stroke = int(s * 0.055)
    for (x, y) in ((x1, y1), (x2, y2)):
        box = (x, y, x + w, y + h)
        if outline:
            d.rounded_rectangle(box, radius=r, fill=fill, outline=outline, width=stroke)
        else:
            d.rounded_rectangle(box, radius=r, fill=fill)
    # a notch in the back card so the two read as separate cards, not one blob
    if outline:
        d.rounded_rectangle((x2 - stroke, y2 - stroke, x2 + w + stroke, y2 + h + stroke), radius=r, outline=outline, width=stroke)
    return img.resize((size, size), Image.LANCZOS)

app_icon(512).save(os.path.join(here, 'icon.png'))
app_icon(256).save(os.path.join(here, 'icon-256.png'))
tray_icon(16, (0, 0, 0, 255)).save(os.path.join(here, 'trayTemplate.png'))  # macOS template: pure black on transparent
# Electron picks the @2x/@3x variants by scale factor, so ship a set rather than
# one bitmap the OS has to resample.
WHITE, DARK = (255, 255, 255, 255), (24, 24, 32, 235)
tray_icon(16, WHITE, DARK).save(os.path.join(here, 'tray.png'))
tray_icon(32, WHITE, DARK).save(os.path.join(here, 'tray@2x.png'))
tray_icon(48, WHITE, DARK).save(os.path.join(here, 'tray@3x.png'))
tray_icon(32, (0, 0, 0, 255)).save(os.path.join(here, 'trayTemplate@2x.png'))
# Windows .ico with several sizes
app_icon(256).save(os.path.join(here, 'icon.ico'), sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
print('icons written')
