"""Simge geometrilerini C# XAML sözlüklerinden Slint'e üretir (Lucide, ISC Lisansı).

Girdi: koy/src/App/Resources/{Icons,IconsNotch}.xaml — Çıktı: crates/koy-app/ui/icons.slint
"""
import re, sys, pathlib
src, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
rows = []
for f in ["Icons.xaml", "IconsNotch.xaml"]:
    rows += re.findall(r'<Geometry x:Key="(\w+)">([^<]+)</Geometry>', (src / f).read_text(encoding="utf-8"))
rows = list({k: (k, v) for k, v in rows}.values())  # WPF: sonra birleştirilen sözlük kazanır
def kebab(k):
    k = k[4:] if k.startswith("Icon") else k
    return re.sub(r'(?<=[a-z0-9])(?=[A-Z])', '-', k).lower()
s = ["// Üretildi: tools/gen_icons.py. Lucide simgeleri (https://lucide.dev), ISC Lisansı.\n",
     "// Copyright (c) Cole Bemis 2013-2022 (Feather, MIT); Lucide Contributors 2022.\n\n",
     "export global Icons {\n"]
for k, v in rows:
    s.append(f"    out property <string> {kebab(k)}: \"{v.strip()}\";\n")
s.append("}\n\n")
s.append("""// WPF KoyIcon karşılığı: 24x24 birimlik çizgi simgesi; renk `color`, boyut 16 (varsayılan) ya da 20, çizgi 24 birimde 2.
export component KoyIcon inherits Path {
    in property <string> data;
    in property <length> size: 16px;
    in property <color> color: gray;
    in property <float> stroke-units: 2;
    width: size;
    height: size;
    commands: data;
    viewbox-width: 24;
    viewbox-height: 24;
    fill: transparent;
    stroke: color;
    stroke-width: stroke-units * size / 24;
    stroke-line-cap: round;
    stroke-line-join: round;
}
""")
(out / "crates/koy-app/ui/icons.slint").write_text("".join(s), encoding="utf-8")
print(len(rows))
