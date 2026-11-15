//! Rust'a özgü: C# `System.IO.Compression.ZipArchive` yerine yazılan okuyucunun ([`ZipArchive`]) testleri.
//! (C# tarafında karşılığı yok; kurulum testleri okuyucuyu dolaylı sınar.)

use std::io::{Cursor, Read};

use super::store_test_kit::*;
use crate::extensions::store::zip_reader::{ZipArchive, ZipError};

fn read_all(archive: &mut ZipArchive<Cursor<Vec<u8>>>, index: usize) -> Result<Vec<u8>, ZipError> {
    let mut reader = archive.open(index)?;
    let mut data = Vec::new();
    reader.read_to_end(&mut data).map_err(ZipError::from)?;
    Ok(data)
}

fn is_invalid_data<T>(result: &Result<T, ZipError>) -> bool {
    matches!(result, Err(ZipError::InvalidData(_)))
}

#[test]
fn stored_and_deflated_entries_are_listed_and_read() {
    let text = format!("Koy {}", "a".repeat(500));
    let bytes = build_zip(&[
        f("a.txt", "merhaba"),
        ZipFile::new("dir/b.txt", text.clone()).deflated(),
        ZipFile::new("klasor/", Vec::new()),
        ZipFile::new("Türkçe-ş.txt", "ş").deflated(),
    ]);

    let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();

    let names: Vec<&str> = archive.entries().iter().map(|e| e.full_name.as_str()).collect();
    assert_eq!(vec!["a.txt", "dir/b.txt", "klasor/", "Türkçe-ş.txt"], names);
    assert_eq!(7, archive.entries()[0].length);
    assert_eq!(text.len() as i64, archive.entries()[1].length);
    assert!(archive.entries()[1].compressed_length < text.len() as u64);
    assert_eq!(b"merhaba".to_vec(), read_all(&mut archive, 0).unwrap());
    assert_eq!(text.into_bytes(), read_all(&mut archive, 1).unwrap());
    assert_eq!(Vec::<u8>::new(), read_all(&mut archive, 2).unwrap());
    assert_eq!("ş".as_bytes().to_vec(), read_all(&mut archive, 3).unwrap());
    // Sırayla değil, geri dönüp okumak da çalışır.
    assert_eq!(b"merhaba".to_vec(), read_all(&mut archive, 0).unwrap());
}

#[test]
fn get_entry_is_ordinal_and_case_sensitive() {
    let archive = ZipArchive::new(Cursor::new(build_zip(&[f("Manifest.json", "{}"), f("manifest.json", "{}")]))).unwrap();

    assert_eq!(Some(1), archive.get_entry("manifest.json"));
    assert_eq!(Some(0), archive.get_entry("Manifest.json"));
    assert_eq!(None, archive.get_entry("MANIFEST.JSON"));

    let duplicate = ZipArchive::new(Cursor::new(build_zip(&[f("a.txt", "ilk"), f("a.txt", "ikinci")]))).unwrap();
    assert_eq!(Some(0), duplicate.get_entry("a.txt"));
}

#[test]
fn empty_archive_has_no_entries() {
    let archive = ZipArchive::new(Cursor::new(build_zip(&[]))).unwrap();

    assert!(archive.entries().is_empty());
}

#[test]
fn archive_comment_after_the_end_record_is_tolerated() {
    let mut bytes = build_zip(&[f("a.txt", "x")]);
    let comment = b"bir yorum";
    let len = bytes.len();
    bytes[len - 2..].copy_from_slice(&(comment.len() as u16).to_le_bytes());
    bytes.extend_from_slice(comment);

    let archive = ZipArchive::new(Cursor::new(bytes)).unwrap();

    assert_eq!(1, archive.entries().len());
}

#[test]
fn garbage_and_truncated_archives_are_invalid_data() {
    assert!(is_invalid_data(&ZipArchive::new(Cursor::new(Vec::new())).map(|_| ())));
    assert!(is_invalid_data(&ZipArchive::new(Cursor::new(b"this is not a zip file at all".to_vec())).map(|_| ())));
    assert!(is_invalid_data(&ZipArchive::new(Cursor::new(vec![0u8; 100])).map(|_| ())));

    let bytes = build_zip(&[f("a.txt", "merhaba"), f("b.txt", "dunya")]);
    for cut in [bytes.len() - 1, bytes.len() - 22, bytes.len() / 2, 30] {
        assert!(is_invalid_data(&ZipArchive::new(Cursor::new(bytes[..cut].to_vec())).map(|_| ())), "cut={cut}");
    }
}

#[test]
fn central_directory_outside_the_stream_is_invalid_data() {
    let mut bytes = build_zip(&[f("a.txt", "x")]);
    let len = bytes.len();
    // Merkezi dizin ofseti (EOCD + 16) akışın dışına.
    bytes[len - 6..len - 2].copy_from_slice(&0x00FF_FFF0u32.to_le_bytes());

    assert!(is_invalid_data(&ZipArchive::new(Cursor::new(bytes)).map(|_| ())));
}

#[test]
fn entry_count_that_cannot_fit_the_directory_is_invalid_data() {
    let mut bytes = build_zip(&[f("a.txt", "x")]);
    let len = bytes.len();
    // Girdi sayısı 1 → 500 (EOCD + 10): merkezi dizin bu kadar girdiyi taşıyamaz.
    bytes[len - 12..len - 10].copy_from_slice(&500u16.to_le_bytes());

    assert!(is_invalid_data(&ZipArchive::new(Cursor::new(bytes)).map(|_| ())));
}

fn central_header_at(bytes: &[u8]) -> usize {
    bytes.windows(4).rposition(|w| w == [0x50, 0x4B, 0x01, 0x02]).unwrap()
}

#[test]
fn encrypted_and_unsupported_entries_cannot_be_opened() {
    let mut encrypted = build_zip(&[f("a.txt", "x")]);
    let at = central_header_at(&encrypted);
    encrypted[at + 8] |= 0x01;
    let mut archive = ZipArchive::new(Cursor::new(encrypted)).unwrap();
    assert!(is_invalid_data(&archive.open(0).map(|_| ())));

    let mut unsupported = build_zip(&[f("a.txt", "x")]);
    let at = central_header_at(&unsupported);
    unsupported[at + 10] = 12; // bzip2
    let mut archive = ZipArchive::new(Cursor::new(unsupported)).unwrap();
    assert!(is_invalid_data(&archive.open(0).map(|_| ())));
}

#[test]
fn corrupt_local_header_and_data_past_the_end_are_invalid_data() {
    let mut bad_signature = build_zip(&[f("a.txt", "x")]);
    bad_signature[0] = 0;
    let mut archive = ZipArchive::new(Cursor::new(bad_signature)).unwrap();
    assert!(is_invalid_data(&archive.open(0).map(|_| ())));

    let mut too_long = build_zip(&[f("a.txt", "x")]);
    let at = central_header_at(&too_long);
    too_long[at + 20..at + 24].copy_from_slice(&0x0010_0000u32.to_le_bytes()); // sıkıştırılmış boyut
    let mut archive = ZipArchive::new(Cursor::new(too_long)).unwrap();
    assert!(is_invalid_data(&archive.open(0).map(|_| ())));
}

#[test]
fn truncated_deflate_data_reads_as_invalid_data() {
    let text = "Koy eklenti magazasi ".repeat(40);
    let bytes = build_zip(&[ZipFile::new("dir/b.txt", text).deflated()]);
    let mut archive = ZipArchive::new(Cursor::new(bytes.clone())).unwrap();
    let mut reader = archive.open(0).unwrap();
    let mut sink = Vec::new();
    // Veri bütün: hata yok.
    assert!(reader.read_to_end(&mut sink).is_ok());

    // Merkezi dizinde sıkıştırılmış boyutu yarıya indir: akış erken biter.
    let mut cut = bytes;
    let at = central_header_at(&cut);
    let compressed = u32::from_le_bytes(cut[at + 20..at + 24].try_into().unwrap());
    cut[at + 20..at + 24].copy_from_slice(&(compressed / 2).to_le_bytes());
    let mut archive = ZipArchive::new(Cursor::new(cut)).unwrap();
    let mut reader = archive.open(0).unwrap();
    let error = reader.read_to_end(&mut Vec::new()).unwrap_err();
    assert!(matches!(ZipError::from(error), ZipError::InvalidData(_)));
}

#[test]
fn zip64_extra_field_supplies_the_real_sizes() {
    // Elle kurulan zip: merkezi dizin girdisinde boyut alanları 0xFFFFFFFF, gerçek değerler Zip64 ek alanında.
    let data = b"merhaba";
    let name = b"a.txt";
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&0x0403_4B50u32.to_le_bytes());
    out.extend_from_slice(&[45, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // sürüm, bayraklar, yöntem, saat, tarih
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    out.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    out.extend_from_slice(&(name.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(name);
    out.extend_from_slice(data);

    let central_offset = out.len() as u32;
    let mut central: Vec<u8> = Vec::new();
    central.extend_from_slice(&0x0201_4B50u32.to_le_bytes());
    central.extend_from_slice(&[45, 0, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0]); // sürümler, bayraklar, yöntem, saat, tarih
    central.extend_from_slice(&0u32.to_le_bytes());
    central.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // sıkıştırılmış
    central.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // açılmış
    central.extend_from_slice(&(name.len() as u16).to_le_bytes());
    central.extend_from_slice(&20u16.to_le_bytes()); // ek alan: 4 başlık + 16 gövde
    central.extend_from_slice(&[0u8; 2 + 2 + 2 + 4]); // yorum, disk, iç öznitelik, dış öznitelik
    central.extend_from_slice(&0u32.to_le_bytes()); // yerel başlık ofseti
    central.extend_from_slice(name);
    central.extend_from_slice(&1u16.to_le_bytes());
    central.extend_from_slice(&16u16.to_le_bytes());
    central.extend_from_slice(&(data.len() as u64).to_le_bytes()); // açılmış
    central.extend_from_slice(&(data.len() as u64).to_le_bytes()); // sıkıştırılmış
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4B50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&central_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());

    let mut archive = ZipArchive::new(Cursor::new(out)).unwrap();

    assert_eq!(7, archive.entries()[0].length);
    assert_eq!(7, archive.entries()[0].compressed_length);
    assert_eq!(data.to_vec(), read_all(&mut archive, 0).unwrap());
}
