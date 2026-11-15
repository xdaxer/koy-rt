//! `System.IO.Compression.ZipArchive` (yalnız okuma kipi) için küçük karşılık.
//!
//! `.koyext` paketleri zip'tir; yeni crate eklenmediği için okuyucu elle yazılmıştır: bitiş merkezi dizin kaydı (EOCD),
//! merkezi dizin girdileri, yerel başlıklar; "stored" (0) ve "deflate" (8) yöntemleri ([`super::inflate`]), Zip64 alanları.
//! .NET gibi: girdi adı ve bildirilen açılmış boyut merkezi dizinden gelir, CRC doğrulanmaz, şifreli girdi ve tanınmayan
//! yöntem `InvalidData` (C# `InvalidDataException`) olur. Çok diskli arşiv desteklenmez.

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};

use super::inflate::Inflater;

const EOCD_SIGNATURE: u32 = 0x0605_4B50;
const EOCD_MIN_LENGTH: u64 = 22;
const ZIP64_LOCATOR_SIGNATURE: u32 = 0x0706_4B50;
const ZIP64_EOCD_SIGNATURE: u32 = 0x0606_4B50;
const CENTRAL_SIGNATURE: u32 = 0x0201_4B50;
const LOCAL_SIGNATURE: u32 = 0x0403_4B50;
const CENTRAL_HEADER_LENGTH: u64 = 46;
const LOCAL_HEADER_LENGTH: u64 = 30;
const MAX_COMMENT_LENGTH: u64 = 0xFFFF;

/// `InvalidDataException` (arşiv bozuk/desteklenmiyor) ya da gerçek G/Ç hatası.
#[derive(Debug)]
pub enum ZipError {
    InvalidData(String),
    Io(io::Error),
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZipError::InvalidData(message) => write!(f, "InvalidDataException: {message}"),
            ZipError::Io(error) => write!(f, "IOException: {error}"),
        }
    }
}

impl std::error::Error for ZipError {}

impl From<io::Error> for ZipError {
    // Zip okumasında "dosya erken bitti" ve deflate bozukluğu arşiv bozukluğudur; geri kalanı G/Ç hatasıdır.
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::UnexpectedEof | io::ErrorKind::InvalidData => ZipError::InvalidData(error.to_string()),
            _ => ZipError::Io(error),
        }
    }
}

fn invalid(message: &str) -> ZipError {
    ZipError::InvalidData(message.to_string())
}

/// Merkezi dizindeki bir girdi (`ZipArchiveEntry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// `FullName`: zip'te yazıldığı gibi (klasör girdisi "/" ile biter).
    pub full_name: String,
    /// `Length`: bildirilen açılmış boyut.
    pub length: i64,
    /// `CompressedLength`.
    pub compressed_length: u64,
    method: u16,
    flags: u16,
    local_header_offset: u64,
}

/// `ZipArchive(stream, ZipArchiveMode.Read)`: aranabilir bir akış üzerinde girdi listesi ve açma.
pub struct ZipArchive<R: Read + Seek> {
    reader: R,
    stream_length: u64,
    entries: Vec<ZipEntry>,
}

impl<R: Read + Seek> ZipArchive<R> {
    /// Merkezi dizini okur. Zip değilse ya da bozuksa `ZipError::InvalidData`.
    pub fn new(mut reader: R) -> Result<Self, ZipError> {
        let stream_length = reader.seek(SeekFrom::End(0))?;
        if stream_length < EOCD_MIN_LENGTH {
            return Err(invalid("End of Central Directory record could not be found."));
        }

        let eocd_offset = find_eocd(&mut reader, stream_length)?;
        reader.seek(SeekFrom::Start(eocd_offset))?;
        let mut eocd = [0u8; EOCD_MIN_LENGTH as usize];
        reader.read_exact(&mut eocd)?;

        let disk_number = u16_at(&eocd, 4);
        let central_directory_disk = u16_at(&eocd, 6);
        let mut entry_count = u64::from(u16_at(&eocd, 10));
        let mut central_size = u64::from(u32_at(&eocd, 12));
        let mut central_offset = u64::from(u32_at(&eocd, 16));
        let comment_length = u64::from(u16_at(&eocd, 20));
        if eocd_offset + EOCD_MIN_LENGTH + comment_length > stream_length {
            return Err(invalid("The archive comment extends past the end of the stream."));
        }

        let needs_zip64 = entry_count == 0xFFFF || central_size == 0xFFFF_FFFF || central_offset == 0xFFFF_FFFF;
        if needs_zip64 {
            let (count, size, offset) = read_zip64_eocd(&mut reader, eocd_offset)?;
            entry_count = count;
            central_size = size;
            central_offset = offset;
        } else if disk_number != 0 || central_directory_disk != 0 {
            return Err(invalid("Split or spanned archives are not supported."));
        }

        if central_offset.checked_add(central_size).is_none_or(|end| end > stream_length) {
            return Err(invalid("The central directory is outside the archive."));
        }
        // Her girdi en az sabit başlık kadardır: sayı, dizin boyutuna sığmıyorsa bozuktur (dev bellek ayırmayı önler).
        if entry_count.saturating_mul(CENTRAL_HEADER_LENGTH) > central_size {
            return Err(invalid("The number of entries does not match the central directory."));
        }

        reader.seek(SeekFrom::Start(central_offset))?;
        let mut directory = vec![0u8; central_size as usize];
        reader.read_exact(&mut directory)?;

        let mut entries = Vec::with_capacity(entry_count as usize);
        let mut pos = 0usize;
        for _ in 0..entry_count {
            let entry = read_central_entry(&directory, &mut pos)?;
            entries.push(entry);
        }

        Ok(ZipArchive { reader, stream_length, entries })
    }

    /// `Entries`.
    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    /// `GetEntry(name)`: ordinal, büyük/küçük harfe duyarlı; aynı adlı birden çok girdi varsa ilki (.NET sözlüğe ilk girdiyi koyar).
    pub fn get_entry(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.full_name == name)
    }

    /// `entry.Open()`: girdinin açılmış içeriğini okuyan akış. Şifreli girdi ve tanınmayan yöntem `InvalidData`.
    pub fn open(&mut self, index: usize) -> Result<EntryReader<'_, R>, ZipError> {
        let entry = self.entries.get(index).ok_or_else(|| invalid("No such entry."))?.clone();
        if entry.flags & 0x0001 != 0 {
            return Err(invalid("Encrypted entries are not supported."));
        }
        if entry.method != 0 && entry.method != 8 {
            return Err(invalid("The archive entry was compressed using an unsupported compression method."));
        }

        self.reader.seek(SeekFrom::Start(entry.local_header_offset))?;
        let mut local = [0u8; LOCAL_HEADER_LENGTH as usize];
        self.reader.read_exact(&mut local)?;
        if u32_at(&local, 0) != LOCAL_SIGNATURE {
            return Err(invalid("A local file header is corrupt."));
        }

        let data_start = entry.local_header_offset + LOCAL_HEADER_LENGTH + u64::from(u16_at(&local, 26)) + u64::from(u16_at(&local, 28));
        if data_start.checked_add(entry.compressed_length).is_none_or(|end| end > self.stream_length) {
            return Err(invalid("The entry data extends past the end of the archive."));
        }

        self.reader.seek(SeekFrom::Start(data_start))?;
        let bounded = (&mut self.reader).take(entry.compressed_length);
        Ok(if entry.method == 0 { EntryReader::Stored(bounded) } else { EntryReader::Deflated(Box::new(Inflater::new(bounded))) })
    }

    /// Altta yatan akışı geri verir.
    pub fn into_inner(self) -> R {
        self.reader
    }
}

/// Bir girdinin açılmış içeriği (`ZipArchiveEntry.Open()` akışı).
pub enum EntryReader<'a, R: Read> {
    Stored(io::Take<&'a mut R>),
    Deflated(Box<Inflater<io::Take<&'a mut R>>>),
}

impl<R: Read> Read for EntryReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            EntryReader::Stored(reader) => reader.read(buf),
            EntryReader::Deflated(reader) => reader.read(buf),
        }
    }
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn u64_at(bytes: &[u8], at: usize) -> u64 {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(raw)
}

// Bitiş merkezi dizin kaydını sondan geriye doğru arar (en çok 65535 baytlık yorum payıyla).
fn find_eocd<R: Read + Seek>(reader: &mut R, stream_length: u64) -> Result<u64, ZipError> {
    let window = stream_length.min(EOCD_MIN_LENGTH + MAX_COMMENT_LENGTH);
    let start = stream_length - window;
    reader.seek(SeekFrom::Start(start))?;
    let mut buffer = vec![0u8; window as usize];
    reader.read_exact(&mut buffer)?;

    let last = buffer.len() - EOCD_MIN_LENGTH as usize;
    for i in (0..=last).rev() {
        if u32_at(&buffer, i) == EOCD_SIGNATURE {
            return Ok(start + i as u64);
        }
    }
    Err(invalid("End of Central Directory record could not be found."))
}

fn read_zip64_eocd<R: Read + Seek>(reader: &mut R, eocd_offset: u64) -> Result<(u64, u64, u64), ZipError> {
    if eocd_offset < 20 {
        return Err(invalid("The Zip64 end of central directory locator is missing."));
    }
    reader.seek(SeekFrom::Start(eocd_offset - 20))?;
    let mut locator = [0u8; 20];
    reader.read_exact(&mut locator)?;
    if u32_at(&locator, 0) != ZIP64_LOCATOR_SIGNATURE {
        return Err(invalid("The Zip64 end of central directory locator is missing."));
    }

    reader.seek(SeekFrom::Start(u64_at(&locator, 8)))?;
    let mut record = [0u8; 56];
    reader.read_exact(&mut record)?;
    if u32_at(&record, 0) != ZIP64_EOCD_SIGNATURE {
        return Err(invalid("The Zip64 end of central directory record is corrupt."));
    }
    if u32_at(&record, 16) != 0 || u32_at(&record, 20) != 0 {
        return Err(invalid("Split or spanned archives are not supported."));
    }

    Ok((u64_at(&record, 32), u64_at(&record, 40), u64_at(&record, 48)))
}

fn read_central_entry(directory: &[u8], pos: &mut usize) -> Result<ZipEntry, ZipError> {
    let header = directory.get(*pos..*pos + CENTRAL_HEADER_LENGTH as usize).ok_or_else(|| invalid("A central directory header is truncated."))?;
    if u32_at(header, 0) != CENTRAL_SIGNATURE {
        return Err(invalid("A central directory header is corrupt."));
    }

    let flags = u16_at(header, 8);
    let method = u16_at(header, 10);
    let mut compressed = u64::from(u32_at(header, 20));
    let mut uncompressed = u64::from(u32_at(header, 24));
    let name_length = usize::from(u16_at(header, 28));
    let extra_length = usize::from(u16_at(header, 30));
    let comment_length = usize::from(u16_at(header, 32));
    let mut local_offset = u64::from(u32_at(header, 42));

    let name_start = *pos + CENTRAL_HEADER_LENGTH as usize;
    let extra_start = name_start + name_length;
    let next = extra_start + extra_length + comment_length;
    if next > directory.len() {
        return Err(invalid("A central directory header is truncated."));
    }

    // Zip64 genişletilmiş bilgi (kimlik 0x0001): yalnız 0xFFFFFFFF olan alanlar sırayla (açılmış, sıkıştırılmış, ofset).
    let extra = &directory[extra_start..extra_start + extra_length];
    let mut at = 0;
    while at + 4 <= extra.len() {
        let id = u16_at(extra, at);
        let size = usize::from(u16_at(extra, at + 2));
        let body_end = (at + 4 + size).min(extra.len());
        if id == 0x0001 {
            let mut field = at + 4;
            let mut take = |value: &mut u64, marker: bool| {
                if marker && field + 8 <= body_end {
                    *value = u64_at(extra, field);
                    field += 8;
                }
            };
            take(&mut uncompressed, u32_at(header, 24) == 0xFFFF_FFFF);
            take(&mut compressed, u32_at(header, 20) == 0xFFFF_FFFF);
            take(&mut local_offset, u32_at(header, 42) == 0xFFFF_FFFF);
        }
        at += 4 + size;
    }

    let length = i64::try_from(uncompressed).map_err(|_| invalid("An entry length is out of range."))?;
    let full_name = String::from_utf8_lossy(&directory[name_start..extra_start]).into_owned();
    *pos = next;
    Ok(ZipEntry { full_name, length, compressed_length: compressed, method, flags, local_header_offset: local_offset })
}
