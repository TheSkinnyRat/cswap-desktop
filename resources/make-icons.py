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

def tray_icon(size, color):
    s = size * 4
    img = Image.new('RGBA', (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    w, h, r = s * 0.62, s * 0.44, s * 0.10
    x1, y1 = s * 0.06, s * 0.10
    x2, y2 = s * 0.32, s * 0.46
    d.rounded_rectangle((x1, y1, x1 + w, y1 + h), radius=r, outline=color, width=int(s * 0.09))
    d.rounded_rectangle((x2, y2, x2 + w, y2 + h), radius=r, fill=color)
    return img.resize((size, size), Image.LANCZOS)

app_icon(512).save(os.path.join(here, 'icon.png'))
app_icon(256).save(os.path.join(here, 'icon-256.png'))
tray_icon(32, (255, 255, 255, 255)).save(os.path.join(here, 'trayTemplate.png'))
tray_icon(32, (60, 60, 70, 255)).save(os.path.join(here, 'tray.png'))
# Windows .ico with several sizes
app_icon(256).save(os.path.join(here, 'icon.ico'), sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
print('icons written')
