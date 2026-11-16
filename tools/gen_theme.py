"""Tema belirteçlerini C# XAML sözlüklerinden üretir (bir kez çalıştırılır, çıktı repoya girer).

Girdi: koy/src/App/Resources/{Theme.Dark,Theme.Light,Tokens}.xaml
Çıktı: crates/koy-app/src/resources/palette.rs, crates/koy-app/ui/theme.slint
"""
import re, sys, pathlib

src = pathlib.Path(sys.argv[1])
out = pathlib.Path(sys.argv[2])

def brushes(text):
    return re.findall(r'<SolidColorBrush x:Key="(\w+)" Color="(#[0-9A-Fa-f]+)"', text)

dark = brushes((src / "Theme.Dark.xaml").read_text(encoding="utf-8"))
light = brushes((src / "Theme.Light.xaml").read_text(encoding="utf-8"))
tok = (src / "Tokens.xaml").read_text(encoding="utf-8")
tok_colors = brushes(tok) + re.findall(r'<Color x:Key="(\w+)">(#[0-9A-Fa-f]+)</Color>', tok)
numbers = [(k, float(v)) for k, v in re.findall(r'<sys:Double x:Key="(\w+)">([\d.]+)</sys:Double>', tok)]
radii = []
for k, v in re.findall(r'<CornerRadius x:Key="(\w+)">([\d.,]+)</CornerRadius>', tok):
    p = [float(x) for x in v.split(",")]
    if len(p) == 1:
        p = p * 4
    radii.append((k, p))
fonts = re.findall(r'<FontFamily x:Key="(\w+)">([^<]+)</FontFamily>', tok)
weight = re.findall(r'<FontWeight x:Key="(\w+)">(\w+)</FontWeight>', tok)
assert [k for k, _ in dark] == [k for k, _ in light], "tema anahtarları farklı"

def argb(h):
    h = h[1:]
    if len(h) == 6:
        h = "FF" + h
    return "0x" + h.upper()

def kebab(key):
    k = key[3:] if key.startswith("Koy") else key
    return re.sub(r'(?<=[a-z0-9])(?=[A-Z])', '-', k).lower()

rs = ["//! Üretildi: tools/gen_theme.py (koy/src/App/Resources/*.xaml). Elle düzenlemeyin.\n"]
def tbl(name, rows, ty, fmt):
    rs.append(f"pub static {name}: &[(&str, {ty})] = &[\n")
    for k, v in rows:
        rs.append(f"    (\"{k}\", {fmt(v)}),\n")
    rs.append("];\n\n")
tbl("DARK", dark, "u32", argb)
tbl("LIGHT", light, "u32", argb)
tbl("TOKEN_COLORS", tok_colors, "u32", argb)
tbl("TOKEN_NUMBERS", numbers, "f64", lambda v: repr(v))
tbl("TOKEN_RADII", radii, "[f64; 4]", lambda p: "[" + ", ".join(repr(x) for x in p) + "]")
tbl("TOKEN_FONTS", fonts, "&str", lambda v: '"' + v + '"')
rs.append(f"pub const FONT_WEIGHT_KEY: &str = \"{weight[0][0]}\";\n")
(out / "crates/koy-app/src/resources/palette.rs").write_text("".join(rs), encoding="utf-8")

sl = ["// Üretildi: tools/gen_theme.py. Elle düzenlemeyin.\n",
      "// WPF DynamicResource karşılığı: değerler Rust'taki çözülmüş kaynak tablosundan okunur; `rev` artınca yenilenir.\n\n",
      "export global Theme {\n",
      "    in property <int> rev;\n",
      "    pure callback get-color(string, int) -> color;\n",
      "    pure callback get-number(string, int) -> float;\n",
      "    pure callback get-radius(string, int, int) -> float;\n",
      "    pure callback get-font(string, int) -> string;\n",
      "    pure callback get-weight(int) -> int;\n\n"]
for k, _ in dark + tok_colors:
    sl.append(f"    out property <color> {kebab(k)}: self.get-color(\"{k}\", self.rev);\n")
for k, _ in numbers:
    sl.append(f"    out property <length> {kebab(k)}: self.get-number(\"{k}\", self.rev) * 1px;\n")
for k, p in radii:
    if len(set(p)) == 1 and not k.endswith(("Top", "BottomLeft")):
        sl.append(f"    out property <length> {kebab(k)}: self.get-radius(\"{k}\", 0, self.rev) * 1px;\n")
    else:
        for i, c in enumerate(["tl", "tr", "br", "bl"]):
            sl.append(f"    out property <length> {kebab(k)}-{c}: self.get-radius(\"{k}\", {i}, self.rev) * 1px;\n")
for k, _ in fonts:
    sl.append(f"    out property <string> {kebab(k)}: self.get-font(\"{k}\", self.rev);\n")
sl.append("    out property <int> font-weight-base: self.get-weight(self.rev);\n}\n")
(out / "crates/koy-app/ui/theme.slint").write_text("".join(sl), encoding="utf-8")
print(len(dark), len(tok_colors), len(numbers), len(radii), len(fonts))
