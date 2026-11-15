//! RFC 1951 (DEFLATE) açıcı: C# `System.IO.Compression.DeflateStream` (okuma) karşılığı, bağımlılıksız.
//!
//! `.koyext` paketlerinin zip girdileri deflate ile sıkıştırılır; yeni crate eklenmediği için açıcı elle yazılmıştır
//! (Mark Adler'in `puff.c` yaklaşımı: bit bit kod çözme, kanonik Huffman sayaç/sembol tabloları). Akışlıdır:
//! [`Inflater`] bir [`Read`]'dir ve çıktıyı istenen parça kadar üretir; böylece çağıran çıktı boyutunu okurken
//! sınırlayabilir (zip bombası savunması, `ExtensionPackageInstaller::extract`). Bozuk veri `InvalidData`,
//! sıkıştırılmış akışın erken bitmesi (`UnexpectedEof` yerine) yine `InvalidData` olarak bildirilir.

use std::io::{self, Read};

/// En büyük geri başvuru uzaklığı (RFC 1951): 32 KiB.
const WINDOW_SIZE: usize = 32 * 1024;
const WINDOW_MASK: usize = WINDOW_SIZE - 1;

const MAX_BITS: usize = 15;
const MAX_L_CODES: usize = 286;
const MAX_D_CODES: usize = 30;
const FIX_L_CODES: usize = 288;

const LENGTH_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] =
    [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const CODE_LENGTH_ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

/// Kanonik Huffman kodu: uzunluk başına kod sayısı ve koda göre sıralı semboller.
struct Huffman {
    count: [u16; MAX_BITS + 1],
    symbol: [u16; FIX_L_CODES],
}

impl Huffman {
    /// Kod uzunluklarından tablo kurar. Dönüşteki sayı kullanılmayan kod alanıdır: 0 tam, > 0 eksik, < 0 aşırı dolu.
    fn build(lengths: &[u8]) -> (Huffman, i32) {
        let mut h = Huffman { count: [0; MAX_BITS + 1], symbol: [0; FIX_L_CODES] };
        for &len in lengths {
            h.count[len as usize] += 1;
        }
        if h.count[0] as usize == lengths.len() {
            // Hiç kod yok: tamam sayılır (yalnız uzunluk/uzaklık kodlarında anlamlı).
            return (h, 0);
        }

        let mut left: i32 = 1;
        for len in 1..=MAX_BITS {
            left <<= 1;
            left -= i32::from(h.count[len]);
            if left < 0 {
                return (h, left);
            }
        }

        let mut offs = [0u16; MAX_BITS + 1];
        for len in 1..MAX_BITS {
            offs[len + 1] = offs[len] + h.count[len];
        }
        for (symbol, &len) in lengths.iter().enumerate() {
            if len != 0 {
                h.symbol[offs[len as usize] as usize] = symbol as u16;
                offs[len as usize] += 1;
            }
        }
        (h, left)
    }
}

/// Bit okuyucu: baytları tek tek tüketir (en alt bit önce), gerektiğinden fazla önden okumaz; böylece saklı blokta
/// kalan bitler atılınca bayt hizası tam olur.
struct BitReader<R: Read> {
    reader: R,
    input: Box<[u8; 4096]>,
    input_pos: usize,
    input_len: usize,
    bit_buffer: u32,
    bit_count: u32,
}

impl<R: Read> BitReader<R> {
    fn new(reader: R) -> Self {
        BitReader { reader, input: Box::new([0; 4096]), input_pos: 0, input_len: 0, bit_buffer: 0, bit_count: 0 }
    }

    // Girdi tamponu boşsa kaynaktan yeniden doldurur; kaynak bitmişse sıkıştırılmış akış erken bitmiştir.
    fn fill(&mut self) -> io::Result<()> {
        while self.input_pos == self.input_len {
            match self.reader.read(&mut self.input[..]) {
                Ok(0) => return Err(invalid("Unexpected end of the compressed data.")),
                Ok(n) => {
                    self.input_len = n;
                    self.input_pos = 0;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn next_byte(&mut self) -> io::Result<u8> {
        self.fill()?;
        let byte = self.input[self.input_pos];
        self.input_pos += 1;
        Ok(byte)
    }

    /// `need` (en çok 16) bit okur.
    fn bits(&mut self, need: u32) -> io::Result<u32> {
        let mut value = self.bit_buffer;
        while self.bit_count < need {
            value |= u32::from(self.next_byte()?) << self.bit_count;
            self.bit_count += 8;
        }
        self.bit_buffer = value >> need;
        self.bit_count -= need;
        Ok(value & ((1u32 << need) - 1))
    }

    /// Bayt sınırına hizalar (kalan bitleri atar).
    fn align(&mut self) {
        self.bit_buffer = 0;
        self.bit_count = 0;
    }

    /// Hizalanmış durumda `buf`'u doldurabildiği kadar bayt okur (girdi tamponundan, sonra kaynaktan).
    fn read_aligned(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        self.fill()?;
        let available = self.input_len - self.input_pos;
        let n = available.min(buf.len());
        buf[..n].copy_from_slice(&self.input[self.input_pos..self.input_pos + n]);
        self.input_pos += n;
        Ok(n)
    }

    /// Bir Huffman sembolü çözer (kod uzunluğuna göre bit bit).
    fn decode(&mut self, h: &Huffman) -> io::Result<usize> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for len in 1..=MAX_BITS {
            code |= self.bits(1)? as i32;
            let count = i32::from(h.count[len]);
            if code - count < first {
                return Ok(h.symbol[(index + (code - first)) as usize] as usize);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err(invalid("Invalid Huffman code in the compressed data."))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    BlockHeader,
    Stored,
    Codes,
    Done,
}

/// Akışlı DEFLATE açıcı (ham deflate: zlib/gzip başlığı yok, zip girdileri böyledir).
pub struct Inflater<R: Read> {
    bits: BitReader<R>,
    window: Box<[u8; WINDOW_SIZE]>,
    /// Şimdiye kadar üretilen toplam bayt (pencere konumu ve uzaklık denetimi).
    produced: u64,
    state: State,
    final_block: bool,
    stored_remaining: usize,
    lit_code: Huffman,
    dist_code: Huffman,
    copy_length: usize,
    copy_distance: usize,
}

impl<R: Read> Inflater<R> {
    pub fn new(reader: R) -> Self {
        Inflater {
            bits: BitReader::new(reader),
            window: Box::new([0; WINDOW_SIZE]),
            produced: 0,
            state: State::BlockHeader,
            final_block: false,
            stored_remaining: 0,
            lit_code: Huffman { count: [0; MAX_BITS + 1], symbol: [0; FIX_L_CODES] },
            dist_code: Huffman { count: [0; MAX_BITS + 1], symbol: [0; FIX_L_CODES] },
            copy_length: 0,
            copy_distance: 0,
        }
    }

    /// Şimdiye kadar üretilen çıktı baytı.
    pub fn total_out(&self) -> u64 {
        self.produced
    }

    fn emit(&mut self, byte: u8) {
        self.window[(self.produced as usize) & WINDOW_MASK] = byte;
        self.produced += 1;
    }

    fn read_block_header(&mut self) -> io::Result<()> {
        self.final_block = self.bits.bits(1)? == 1;
        match self.bits.bits(2)? {
            0 => {
                self.bits.align();
                let length = self.bits.bits(16)?;
                let inverse = self.bits.bits(16)?;
                if length != (!inverse & 0xFFFF) {
                    return Err(invalid("Stored block length check failed."));
                }
                self.stored_remaining = length as usize;
                self.state = State::Stored;
            }
            1 => {
                self.set_fixed_codes();
                self.state = State::Codes;
            }
            2 => {
                self.read_dynamic_codes()?;
                self.state = State::Codes;
            }
            _ => return Err(invalid("Invalid deflate block type.")),
        }
        Ok(())
    }

    fn set_fixed_codes(&mut self) {
        let mut lengths = [0u8; FIX_L_CODES];
        for (symbol, length) in lengths.iter_mut().enumerate() {
            *length = match symbol {
                0..=143 => 8,
                144..=255 => 9,
                256..=279 => 7,
                _ => 8,
            };
        }
        self.lit_code = Huffman::build(&lengths).0;
        // 30 uzaklık kodu 5 bit (31 ve 32 geçersiz).
        self.dist_code = Huffman::build(&[5u8; MAX_D_CODES]).0;
    }

    fn read_dynamic_codes(&mut self) -> io::Result<()> {
        let nlen = self.bits.bits(5)? as usize + 257;
        let ndist = self.bits.bits(5)? as usize + 1;
        let ncode = self.bits.bits(4)? as usize + 4;
        if nlen > MAX_L_CODES || ndist > MAX_D_CODES {
            return Err(invalid("Too many length or distance codes."));
        }

        // Kod uzunlukları kodunun uzunlukları.
        let mut lengths = [0u8; MAX_L_CODES + MAX_D_CODES];
        for &position in CODE_LENGTH_ORDER.iter().take(ncode) {
            lengths[position] = self.bits.bits(3)? as u8;
        }
        let (length_code, left) = Huffman::build(&lengths[..19]);
        if left != 0 {
            return Err(invalid("Code length code is incomplete."));
        }

        // Uzunluk/değişmez ve uzaklık kod uzunlukları.
        lengths = [0u8; MAX_L_CODES + MAX_D_CODES];
        let mut index = 0;
        while index < nlen + ndist {
            let symbol = self.bits.decode(&length_code)?;
            if symbol < 16 {
                lengths[index] = symbol as u8;
                index += 1;
                continue;
            }

            let (length, repeat) = match symbol {
                16 => {
                    if index == 0 {
                        return Err(invalid("Repeat with no previous code length."));
                    }
                    (lengths[index - 1], 3 + self.bits.bits(2)? as usize)
                }
                17 => (0, 3 + self.bits.bits(3)? as usize),
                _ => (0, 11 + self.bits.bits(7)? as usize),
            };
            if index + repeat > nlen + ndist {
                return Err(invalid("Too many code lengths."));
            }
            for _ in 0..repeat {
                lengths[index] = length;
                index += 1;
            }
        }

        if lengths[256] == 0 {
            return Err(invalid("Missing end-of-block code."));
        }

        let (lit_code, left) = Huffman::build(&lengths[..nlen]);
        if left < 0 || (left > 0 && nlen != usize::from(lit_code.count[0]) + usize::from(lit_code.count[1])) {
            return Err(invalid("Literal/length code is over-subscribed or incomplete."));
        }
        let (dist_code, left) = Huffman::build(&lengths[nlen..nlen + ndist]);
        if left < 0 || (left > 0 && ndist != usize::from(dist_code.count[0]) + usize::from(dist_code.count[1])) {
            return Err(invalid("Distance code is over-subscribed or incomplete."));
        }

        self.lit_code = lit_code;
        self.dist_code = dist_code;
        Ok(())
    }

    // Bir uzunluk/uzaklık çifti çözer ve kopyayı hazırlar.
    fn read_match(&mut self, symbol: usize) -> io::Result<()> {
        let symbol = symbol - 257;
        if symbol >= 29 {
            return Err(invalid("Invalid length symbol."));
        }
        let length = usize::from(LENGTH_BASE[symbol]) + self.bits.bits(u32::from(LENGTH_EXTRA[symbol]))? as usize;

        let distance_symbol = self.bits.decode(&self.dist_code)?;
        if distance_symbol >= MAX_D_CODES {
            return Err(invalid("Invalid distance symbol."));
        }
        let distance = usize::from(DIST_BASE[distance_symbol]) + self.bits.bits(u32::from(DIST_EXTRA[distance_symbol]))? as usize;
        if distance as u64 > self.produced {
            return Err(invalid("Distance is too far back."));
        }

        self.copy_length = length;
        self.copy_distance = distance;
        Ok(())
    }
}

impl<R: Read> Read for Inflater<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let mut written = 0;
        while written < out.len() {
            if self.copy_length > 0 {
                // Çakışan kopya (uzaklık < uzunluk) bayt bayt çalışır.
                let byte = self.window[(self.produced as usize).wrapping_sub(self.copy_distance) & WINDOW_MASK];
                self.emit(byte);
                out[written] = byte;
                written += 1;
                self.copy_length -= 1;
                continue;
            }

            match self.state {
                State::Done => break,
                State::BlockHeader => self.read_block_header()?,
                State::Stored => {
                    if self.stored_remaining == 0 {
                        self.state = if self.final_block { State::Done } else { State::BlockHeader };
                        continue;
                    }

                    let want = self.stored_remaining.min(out.len() - written);
                    let n = self.bits.read_aligned(&mut out[written..written + want])?;
                    for i in 0..n {
                        let byte = out[written + i];
                        self.emit(byte);
                    }
                    written += n;
                    self.stored_remaining -= n;
                }
                State::Codes => {
                    let symbol = self.bits.decode(&self.lit_code)?;
                    match symbol {
                        0..=255 => {
                            self.emit(symbol as u8);
                            out[written] = symbol as u8;
                            written += 1;
                        }
                        256 => self.state = if self.final_block { State::Done } else { State::BlockHeader },
                        _ => self.read_match(symbol)?,
                    }
                }
            }
        }

        Ok(written)
    }
}

/// Bütün ham deflate akışını belleğe açar (testler ve küçük girdiler için).
pub fn inflate(data: &[u8]) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    Inflater::new(data).read_to_end(&mut output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        let clean: String = text.split_whitespace().collect();
        (0..clean.len()).step_by(2).map(|i| u8::from_str_radix(&clean[i..i + 2], 16).unwrap()).collect()
    }

    /// Test için en küçük DEFLATE yazıcısı: sabit Huffman bloğu, harf ve (uzunluk, uzaklık) işlemleri.
    enum Op {
        Literal(u8),
        Match(usize, usize),
    }

    struct BitWriter {
        bytes: Vec<u8>,
        buffer: u64,
        count: u32,
    }

    impl BitWriter {
        fn new() -> Self {
            BitWriter { bytes: Vec::new(), buffer: 0, count: 0 }
        }

        /// Bitleri en alt bit önce yazar (veri ögeleri ve uzaklık/ek bitler).
        fn put(&mut self, value: u32, bits: u32) {
            self.buffer |= u64::from(value) << self.count;
            self.count += bits;
            while self.count >= 8 {
                self.bytes.push(self.buffer as u8);
                self.buffer >>= 8;
                self.count -= 8;
            }
        }

        /// Huffman kodları en üst bit önce yazılır.
        fn put_code(&mut self, code: u32, bits: u32) {
            for i in (0..bits).rev() {
                self.put((code >> i) & 1, 1);
            }
        }

        fn finish(mut self) -> Vec<u8> {
            if self.count > 0 {
                self.bytes.push(self.buffer as u8);
            }
            self.bytes
        }
    }

    fn put_fixed_literal_length(w: &mut BitWriter, symbol: u32) {
        match symbol {
            0..=143 => w.put_code(0x30 + symbol, 8),
            144..=255 => w.put_code(0x190 + (symbol - 144), 9),
            256..=279 => w.put_code(symbol - 256, 7),
            _ => w.put_code(0xC0 + (symbol - 280), 8),
        }
    }

    fn encode_fixed(ops: &[Op]) -> Vec<u8> {
        let mut w = BitWriter::new();
        w.put(1, 1); // son blok
        w.put(1, 2); // sabit Huffman
        for op in ops {
            match *op {
                Op::Literal(b) => put_fixed_literal_length(&mut w, u32::from(b)),
                Op::Match(length, distance) => {
                    let li = LENGTH_BASE.iter().rposition(|&b| usize::from(b) <= length).unwrap();
                    put_fixed_literal_length(&mut w, 257 + li as u32);
                    w.put((length - usize::from(LENGTH_BASE[li])) as u32, u32::from(LENGTH_EXTRA[li]));
                    let di = DIST_BASE.iter().rposition(|&b| usize::from(b) <= distance).unwrap();
                    w.put_code(di as u32, 5);
                    w.put((distance - usize::from(DIST_BASE[di])) as u32, u32::from(DIST_EXTRA[di]));
                }
            }
        }
        put_fixed_literal_length(&mut w, 256);
        w.finish()
    }

    #[test]
    fn empty_fixed_block() {
        assert_eq!(Vec::<u8>::new(), inflate(&hex("03 00")).unwrap());
    }

    #[test]
    fn stored_block() {
        // Python: zlib.compressobj(0, DEFLATED, -15) ile "hello" (saklı blok).
        assert_eq!(b"hello".to_vec(), inflate(&hex("01 05 00 fa ff 68 65 6c 6c 6f")).unwrap());
    }

    #[test]
    fn stored_block_followed_by_more_blocks() {
        // Saklı blok (son değil) + sabit boş blok.
        let data = hex("00 03 00 fc ff 61 62 63 03 00");
        assert_eq!(b"abc".to_vec(), inflate(&data).unwrap());
    }

    #[test]
    fn fixed_huffman_real_vector() {
        // Python: zlib.compressobj(9, DEFLATED, -15).compress(b"hello hello hello hello")
        assert_eq!(b"hello hello hello hello".to_vec(), inflate(&hex("cb 48 cd c9 c9 57 c8 40 27 01")).unwrap());
    }

    #[test]
    fn dynamic_huffman_real_vector() {
        let expected = dynamic_sample();
        assert_eq!(expected, inflate(&hex(DYNAMIC_SAMPLE_DEFLATE)).unwrap());
    }

    #[test]
    fn multiple_blocks_with_full_flush() {
        let expected: Vec<u8> = [&b"first chunk of text. "[..], &b"second chunk of text. "[..], &b"third chunk."[..]].concat();
        assert_eq!(expected, inflate(&hex(FLUSHED_SAMPLE_DEFLATE)).unwrap());
    }

    #[test]
    fn overlapping_copy_repeats_the_pattern() {
        // 'a' sonra (uzunluk 10, uzaklık 1): "a" * 11.
        let data = encode_fixed(&[Op::Literal(b'a'), Op::Match(10, 1)]);
        assert_eq!(vec![b'a'; 11], inflate(&data).unwrap());
    }

    #[test]
    fn copies_across_the_whole_32k_window_and_wrap_around() {
        let mut ops: Vec<Op> = Vec::new();
        let mut expected: Vec<u8> = Vec::new();
        // 40 000 harf (pencere iki kez döner), sonra en büyük uzaklıkla ve en büyük uzunlukla kopyalar.
        for i in 0..40_000u32 {
            let b = (i.wrapping_mul(2_654_435_761) >> 24) as u8;
            ops.push(Op::Literal(b));
            expected.push(b);
        }
        for _ in 0..5 {
            ops.push(Op::Match(258, 32_768));
            let start = expected.len() - 32_768;
            for k in 0..258 {
                let b = expected[start + k];
                expected.push(b);
            }
        }
        ops.push(Op::Match(3, 1));
        for _ in 0..3 {
            let b = *expected.last().unwrap();
            expected.push(b);
        }

        assert_eq!(expected, inflate(&encode_fixed(&ops)).unwrap());
    }

    #[test]
    fn read_in_small_chunks_gives_the_same_output() {
        let expected = dynamic_sample();
        let compressed = hex(DYNAMIC_SAMPLE_DEFLATE);
        let mut inflater = Inflater::new(&compressed[..]);
        let mut output = Vec::new();
        let mut buffer = [0u8; 7];
        loop {
            let n = inflater.read(&mut buffer).unwrap();
            if n == 0 {
                break;
            }
            output.extend_from_slice(&buffer[..n]);
        }
        assert_eq!(expected, output);
        assert_eq!(expected.len() as u64, inflater.total_out());
    }

    #[test]
    fn distance_before_the_start_is_rejected() {
        let data = encode_fixed(&[Op::Literal(b'a'), Op::Match(3, 2)]);
        assert_eq!(io::ErrorKind::InvalidData, inflate(&data).unwrap_err().kind());
    }

    #[test]
    fn reserved_block_type_and_bad_stored_length_are_rejected() {
        assert_eq!(io::ErrorKind::InvalidData, inflate(&[0x07]).unwrap_err().kind());
        assert_eq!(io::ErrorKind::InvalidData, inflate(&hex("01 05 00 fa fe 68 65 6c 6c 6f")).unwrap_err().kind());
    }

    #[test]
    fn truncated_input_is_invalid_data() {
        let full = hex(DYNAMIC_SAMPLE_DEFLATE);
        for cut in [0, 1, 5, full.len() / 2, full.len() - 1] {
            let error = inflate(&full[..cut]).unwrap_err();
            assert_eq!(io::ErrorKind::InvalidData, error.kind(), "cut={cut}");
        }
    }

    #[test]
    fn garbage_never_panics() {
        let mut state = 12345u32;
        for round in 0..200 {
            let mut data = Vec::new();
            for _ in 0..(round % 40 + 1) {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                data.push((state >> 24) as u8);
            }
            let _ = inflate(&data);
        }
    }

    #[test]
    fn output_can_be_limited_by_the_caller() {
        // 1 MB sıfır: sınırlı okuyan çağıran baştan keser.
        let ops = [Op::Literal(0), Op::Match(258, 1), Op::Match(258, 1)];
        let data = encode_fixed(&ops);
        let mut limited = Inflater::new(&data[..]).take(100);
        let mut out = Vec::new();
        limited.read_to_end(&mut out).unwrap();
        assert_eq!(vec![0u8; 100], out);
    }

    fn dynamic_sample() -> Vec<u8> {
        let mut text = Vec::new();
        for i in 0..40 {
            text.extend_from_slice(format!("Satir {i}: Koy eklenti magazasi, tema ve simge paketi. ").as_bytes());
            text.extend_from_slice(b"The quick brown fox jumps over the lazy dog.\n");
        }
        text
    }

    // Python: zlib.compressobj(9, zlib.DEFLATED, -15) ile `dynamic_sample()` içeriği (dinamik Huffman bloğu).
    const DYNAMIC_SAMPLE_DEFLATE: &str = concat!(
    "bdd6cd51c3400c86e13b55a800c61349107e5ac83134b081c52cf67a137b9d10574f263dbc3eebf09e9ef9b40f358db2",
    "79975db94aecfa38d42439b46109537a941a7390739429e536ca3174b1a6463e7ea29ce6f4d9c9612c9741becb9ffcce",
    "f9384939c751eaeddc87e52a5fa56d1ef6f782e205c30b8e179ef0c2335ed8e28517bcf08a17de78712ba0e6552bcf5a",
    "79d7cac3565eb6f2b495b7ad3c6ee5751bafdb56d86c5eb7f1ba8dd76dbc6ee3751bafdb78ddc6eb765eb7f3ba7d8597",
    "9cd7edbc6ee7753bafdb79ddceeb7652f73f",
);

    // Python: compressobj(6, DEFLATED, -15); compress(a) + flush(Z_FULL_FLUSH) + compress(b) + flush(Z_FULL_FLUSH) + compress(c) + flush(Z_FINISH).
    const FLUSHED_SAMPLE_DEFLATE: &str = "4acb2c2a2e5148ce28cdcb56c84f532849ad28d15300000000ffff2a4e4dcecf4b5148ce28cdcb56c84f532849ad28d15300000000ffff2bc9c82c4a5148ce28cdcbd60300";
}
