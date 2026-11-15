use crate::text;
use std::f64::consts::PI;

const FFT_SIZE: i32 = 1024;
const FFT_LEN: usize = FFT_SIZE as usize;
const MIN_FREQUENCY: f64 = 50.0;
const MAX_FREQUENCY: f64 = 14_000.0;
const FLOOR_DB: f64 = -72.0;
const CEILING_DB: f64 = -12.0;
const ATTACK: f32 = 0.65;
const DECAY: f32 = 0.18;

// Müzikte enerji frekans arttıkça düşer (oktav başına birkaç dB); düzeltilmezse sağdaki tiz çubuklar hep dipte kalır.
// Bant ortası 250 Hz'den uzaklaştıkça oktav başına bu kadar dB eklenir (altında aynı oranda düşülür).
const TILT_DB_PER_OCTAVE: f64 = 3.5;
const TILT_PIVOT_HZ: f64 = 250.0;

/// Çalan sesten müzik kartındaki çubukların yüksekliğini hesaplar: son `FFT_SIZE` örneğe Hann penceresi,
/// FFT, logaritmik aralıklı bantlar (bas solda, tiz sağda), dB ölçeği ve yumuşatma (hızlı yükselir, yavaş iner).
/// Saf hesap: ses yakalama ve zamanlama çağıranın işi. Tek iş parçacığından kullanılır.
pub struct SpectrumAnalyzer {
    ring: Vec<f32>,
    ring_pos: usize,
    window: Vec<f64>,
    re: Vec<f64>,
    im: Vec<f64>,
    levels: Vec<f32>,
    band_edges: Vec<i32>,
    band_tilt_db: Vec<f64>,
    edges_for_rate: i32,
}

impl SpectrumAnalyzer {
    pub const FFT_SIZE: i32 = FFT_SIZE;

    pub fn new(band_count: i32) -> Self {
        // ArgumentOutOfRangeException.ThrowIfNegativeOrZero(bandCount)
        assert!(band_count > 0, "bandCount must be a non-negative and non-zero value.");

        let mut window = vec![0.0f64; FFT_LEN];
        for (i, w) in window.iter_mut().enumerate() {
            *w = 0.5 * (1.0 - (2.0 * PI * i as f64 / (FFT_SIZE - 1) as f64).cos());
        }

        SpectrumAnalyzer {
            ring: vec![0.0f32; FFT_LEN],
            ring_pos: 0,
            window,
            re: vec![0.0f64; FFT_LEN],
            im: vec![0.0f64; FFT_LEN],
            levels: vec![0.0f32; band_count as usize],
            band_edges: Vec::new(),
            band_tilt_db: Vec::new(),
            edges_for_rate: 0,
        }
    }

    pub fn band_count(&self) -> i32 {
        self.levels.len() as i32
    }

    /// Yumuşatılmış bant düzeyleri, 0..1. `update` her çağrıldığında yenilenir.
    pub fn levels(&self) -> &[f32] {
        &self.levels
    }

    /// Bütün çubuklar inmiş mi (çizim gereksiz).
    pub fn is_silent(&self) -> bool {
        for &level in &self.levels {
            if level > 0.01f32 {
                return false;
            }
        }

        true
    }

    /// Tek kanala indirilmiş örnekleri (−1..1) halka belleğe ekler; en yeni `FFT_SIZE` örnek tutulur.
    pub fn add_samples(&mut self, mono: &[f32]) {
        if mono.len() >= FFT_LEN {
            self.ring.copy_from_slice(&mono[mono.len() - FFT_LEN..]);
            self.ring_pos = 0;
            return;
        }

        for &sample in mono {
            self.ring[self.ring_pos] = sample;
            self.ring_pos = (self.ring_pos + 1) % FFT_LEN;
        }
    }

    /// Ses gelmediği süre için sessizlik ekler (çubuklar doğal biçimde iner).
    pub fn add_silence(&mut self, count: i32) {
        let count = count.min(FFT_SIZE);
        for _ in 0..count {
            self.ring[self.ring_pos] = 0.0;
            self.ring_pos = (self.ring_pos + 1) % FFT_LEN;
        }
    }

    /// Halka bellekteki son örneklerden bant düzeylerini hesaplar ve yumuşatır.
    pub fn update(&mut self, sample_rate: i32) {
        if sample_rate <= 0 {
            return;
        }

        if self.edges_for_rate != sample_rate {
            self.band_edges = Self::band_edges(self.levels.len() as i32, sample_rate);
            self.band_tilt_db = Self::band_tilt(&self.band_edges, sample_rate);
            self.edges_for_rate = sample_rate;
        }

        for i in 0..FFT_LEN {
            self.re[i] = self.ring[(self.ring_pos + i) % FFT_LEN] as f64 * self.window[i];
            self.im[i] = 0.0;
        }

        fft(&mut self.re, &mut self.im);

        // Hann penceresinin genlik kaybı (0.5) ve tek taraflı spektrum (×2) birlikte: tam ölçekli sinüs ≈ 0 dB.
        const SCALE: f64 = 4.0 / FFT_SIZE as f64;
        for band in 0..self.levels.len() {
            let mut peak = 0.0f64;
            for bin in self.band_edges[band]..self.band_edges[band + 1] {
                let bin = bin as usize;
                let magnitude = (self.re[bin] * self.re[bin] + self.im[bin] * self.im[bin]).sqrt() * SCALE;
                if magnitude > peak {
                    peak = magnitude;
                }
            }

            let db = if peak > 0.0 { 20.0 * peak.log10() + self.band_tilt_db[band] } else { FLOOR_DB };
            let target = text::clamp((db - FLOOR_DB) / (CEILING_DB - FLOOR_DB), 0.0, 1.0) as f32;
            let current = self.levels[band];
            self.levels[band] = if target > current {
                current + (target - current) * ATTACK
            } else {
                current + (target - current) * DECAY
            };
        }
    }

    /// Bant sınırları (FFT kutusu indeksleri, bandCount + 1 tane): log aralıklı, her bant en az bir kutu.
    pub fn band_edges(band_count: i32, sample_rate: i32) -> Vec<i32> {
        let bin_hz = sample_rate as f64 / FFT_SIZE as f64;
        let max_bin = FFT_SIZE / 2;
        let top = MAX_FREQUENCY.min(sample_rate as f64 / 2.0 * 0.95);
        let mut edges = vec![0i32; band_count as usize + 1];
        let ratio = (top / MIN_FREQUENCY).powf(1.0 / band_count as f64);

        let mut previous = 1.max(text::round(MIN_FREQUENCY / bin_hz) as i32);
        edges[0] = previous;
        for i in 1..=band_count {
            let bin = text::round(MIN_FREQUENCY * ratio.powf(i as f64) / bin_hz) as i32;
            let bin = text::clamp(bin.max(previous + 1), 1, max_bin);
            edges[i as usize] = bin;
            previous = bin;
        }

        edges
    }

    /// Her bandın eğim düzeltmesi (dB): bant ortasının 250 Hz'e göre oktav farkı × oktav başına dB.
    pub fn band_tilt(edges: &[i32], sample_rate: i32) -> Vec<f64> {
        let bin_hz = sample_rate as f64 / FFT_SIZE as f64;
        let mut tilt = vec![0.0f64; edges.len() - 1];
        for i in 0..tilt.len() {
            let center = (edges[i] as f64 * edges[i + 1].max(edges[i] + 1) as f64).sqrt() * bin_hz;
            tilt[i] = (center / TILT_PIVOT_HZ).log2() * TILT_DB_PER_OCTAVE;
        }

        tilt
    }
}

// Yerinde radix-2 Cooley–Tukey FFT.
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut i = 1usize;
    let mut j = 0usize;
    while i < n {
        let mut bit = n >> 1;
        while (j & bit) != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;

        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
        i += 1;
    }

    let mut length = 2usize;
    while length <= n {
        let angle = -2.0 * PI / length as f64;
        let w_re = angle.cos();
        let w_im = angle.sin();
        let mut start = 0usize;
        while start < n {
            let mut cur_re = 1.0f64;
            let mut cur_im = 0.0f64;
            for k in 0..length / 2 {
                let a = start + k;
                let b = a + length / 2;
                let t_re = re[b] * cur_re - im[b] * cur_im;
                let t_im = re[b] * cur_im + im[b] * cur_re;
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let next_re = cur_re * w_re - cur_im * w_im;
                cur_im = cur_re * w_im + cur_im * w_re;
                cur_re = next_re;
            }
            start += length;
        }
        length <<= 1;
    }
}
