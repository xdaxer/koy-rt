//! Derleme betiği: Slint arayüzünü derler ve .resx dil dosyalarını Rust tablolarına çevirir
//! (C# `ResourceManager` karşılığı; çalışma anında XML ayrıştırılmaz).

use std::fmt::Write as _;
use std::path::Path;

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest.find(';').unwrap_or(0);
        let entity = &rest[1..end];
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            e if e.starts_with("#x") => out.push(char::from_u32(u32::from_str_radix(&e[2..], 16).unwrap()).unwrap()),
            e if e.starts_with('#') => out.push(char::from_u32(e[1..].parse().unwrap()).unwrap()),
            _ => panic!("bilinmeyen XML varlığı: &{entity};"),
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// `<data name="X" ...><value>…</value></data>` çiftleri, ada göre sıralı.
fn parse_resx(path: &Path) -> Vec<(String, String)> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut out = Vec::new();
    let mut rest = text.as_str();
    while let Some(i) = rest.find("<data name=\"") {
        rest = &rest[i + 12..];
        let name_end = rest.find('"').unwrap();
        let name = rest[..name_end].to_string();
        let data_end = rest.find("</data>").unwrap();
        let body = &rest[..data_end];
        if let (Some(vs), Some(ve)) = (body.find("<value>"), body.find("</value>")) {
            out.push((name, unescape(&body[vs + 7..ve])));
        } else if body.contains("<value />") {
            out.push((name, String::new()));
        }
        rest = &rest[data_end..];
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

fn table(name: &str, rows: &[(String, String)]) -> String {
    let mut s = format!("pub static {name}: &[(&str, &str)] = &[\n");
    for (k, v) in rows {
        let _ = writeln!(s, "    ({k:?}, {v:?}),");
    }
    s.push_str("];\n");
    s
}

fn main() {
    slint_build::compile("ui/app.slint").expect("slint derlemesi");

    let en = parse_resx(Path::new("resources/Strings.resx"));
    let tr = parse_resx(Path::new("resources/Strings.tr.resx"));
    let out = std::env::var("OUT_DIR").unwrap();
    let code = format!("// Üretildi: build.rs (resources/Strings*.resx).\n{}{}", table("EN", &en), table("TR", &tr));
    std::fs::write(Path::new(&out).join("strings_gen.rs"), code).unwrap();
    println!("cargo:rerun-if-changed=resources/Strings.resx");
    println!("cargo:rerun-if-changed=resources/Strings.tr.resx");
    println!("cargo:rerun-if-changed=ui");
}
