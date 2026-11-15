use crate::appearance::{ColorMath, Rgb};
use crate::text;

/// Albüm kapağından türetilen müzik kartı renkleri. Spectrum App tarafında alfa 0.75 ile kullanılır.
/// `blend` doluysa kapak çok renklidir (ör. kick.com yayınında kapak yerine gelen Chrome logosu):
/// kart tek bir tona boyanmaz, bu renklerin köşelerden yayılan yumuşak karışımı çizilir; `card` o
/// zaman karışımın altındaki nötr zemindir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverPalette {
    pub card: Rgb,
    pub sub_text: Rgb,
    pub spectrum: Rgb,
    pub blend: Option<Vec<Rgb>>,
}

impl CoverPalette {
    pub fn new(card: Rgb, sub_text: Rgb, spectrum: Rgb, blend: Option<Vec<Rgb>>) -> Self {
        CoverPalette { card, sub_text, spectrum, blend }
    }
}

/// Küçük bir kapak resminden (BGRA, örn. 32 px) vurgu paleti çıkarır. Tek geçiş, yığın üstü kovalar;
/// WPF tipi yok. Gri/monokrom kapakta `None` döner (varsayılan tema kullanılır).
pub struct CoverPaletteExtractor;

const HUE_BUCKETS: usize = 24;
const MIN_SATURATION: f64 = 0.20;
const MIN_LIGHTNESS: f64 = 0.08;
const MAX_LIGHTNESS: f64 = 0.95;
const MIN_CANDIDATE_RATIO: f64 = 0.10;
const CARD_LIGHTNESS: f64 = 0.14;
const SUB_TEXT_SATURATION: f64 = 0.60;
const SUB_TEXT_LIGHTNESS: f64 = 0.80;
const MIN_TITLE_CONTRAST: f64 = 7.0;
const TITLE: Rgb = Rgb::new(0xF4, 0xF4, 0xF5);

// Çok renkli kapak: en az üç ayrı ton (aralarında en az 30°), her biri renkli piksellerin %15'inden fazlası.
// Tek bir baskın ton seçmek burada yanıltıcı (Chrome logosunda sarı kazanıp hardal kart veriyordu).
const BLEND_MIN_SHARE: f64 = 0.15;
const BLEND_MIN_SEPARATION: i32 = 2; // kova = 15°, 2 kova = 30° (Chrome kırmızısı ile sarısı ~40° ayrık)
const BLEND_MIN_COLORS: usize = 3;
const BLEND_MAX_COLORS: usize = 4;
const BLEND_BASE: Rgb = Rgb::new(0x14, 0x14, 0x16);
const BLEND_SUB_TEXT: Rgb = Rgb::new(0xC8, 0xC8, 0xCE);

impl CoverPaletteExtractor {
    pub fn extract(bgra: &[u8], width: i32, height: i32, stride: i32) -> Option<CoverPalette> {
        if width <= 0 || height <= 0 || stride < width.wrapping_mul(4) {
            return None;
        }
        if (bgra.len() as i64) < (height as i64 - 1) * stride as i64 + width.wrapping_mul(4) as i64 {
            return None;
        }

        let mut weight = [0.0f64; HUE_BUCKETS];
        let mut hue_sum = [0.0f64; HUE_BUCKETS];
        let mut sat_sum = [0.0f64; HUE_BUCKETS];
        let mut total: i32 = 0;
        let mut candidates: i32 = 0;

        for y in 0..height {
            let start = (y as usize) * (stride as usize);
            let row = &bgra[start..start + (width as usize) * 4];
            for px in row.chunks_exact(4) {
                if px[3] < 16 {
                    continue; // saydam piksel kapağın parçası değil
                }

                total += 1;
                let hsl = ColorMath::to_hsl_bytes(px[2], px[1], px[0]);
                if hsl.s < MIN_SATURATION || hsl.l < MIN_LIGHTNESS || hsl.l > MAX_LIGHTNESS {
                    continue;
                }

                candidates += 1;
                let bucket = ((hsl.h / (360.0 / HUE_BUCKETS as f64)) as i32).min(HUE_BUCKETS as i32 - 1) as usize;
                weight[bucket] += hsl.s;
                hue_sum[bucket] += hsl.h * hsl.s;
                sat_sum[bucket] += hsl.s * hsl.s;
            }
        }

        if total == 0 || (candidates as f64) < total as f64 * MIN_CANDIDATE_RATIO {
            return None;
        }

        if let Some(blend) = Self::blend_colors(&weight, &hue_sum, &sat_sum) {
            return Some(CoverPalette::new(BLEND_BASE, BLEND_SUB_TEXT, BLEND_SUB_TEXT, Some(blend)));
        }

        let mut best = 0;
        for i in 1..HUE_BUCKETS {
            if weight[i] > weight[best] {
                best = i;
            }
        }

        if weight[best] <= 0.0 {
            return None;
        }

        let hue = hue_sum[best] / weight[best];
        let card_sat = text::clamp(sat_sum[best] / weight[best], 0.35, 0.55);

        let mut card_l = CARD_LIGHTNESS;
        let mut card = ColorMath::from_hsl(hue, card_sat, card_l);
        while card_l > 0.0 && ColorMath::contrast(TITLE, card) < MIN_TITLE_CONTRAST {
            card_l = (card_l - 0.01).max(0.0);
            card = ColorMath::from_hsl(hue, card_sat, card_l);
        }

        let sub = ColorMath::from_hsl(hue, SUB_TEXT_SATURATION, SUB_TEXT_LIGHTNESS);
        Some(CoverPalette::new(card, sub, sub, None))
    }

    fn blend_colors(weight: &[f64], hue_sum: &[f64], sat_sum: &[f64]) -> Option<Vec<Rgb>> {
        let mut total = 0.0f64;
        for w in weight {
            total += w;
        }
        if total <= 0.0 {
            return None;
        }

        let mut order: Vec<usize> = (0..weight.len()).collect();
        // C# `List<T>.Sort` kararsızdır (IntroSort): eşit ağırlıklı kovaların sırası algoritmaya bağlı olduğundan
        // .NET'in aynı algoritması birebir taşındı.
        dotnet_sort(&mut order, |a, b| compare_to(weight[*b], weight[*a]));

        let mut picked: Vec<usize> = Vec::new();
        for &i in &order {
            if weight[i] / total < BLEND_MIN_SHARE {
                break;
            }

            let far = picked.iter().all(|&j| {
                let d = (i as i32 - j as i32).abs();
                d.min(HUE_BUCKETS as i32 - d) >= BLEND_MIN_SEPARATION
            });
            if far {
                picked.push(i);
            }
            if picked.len() == BLEND_MAX_COLORS {
                break;
            }
        }

        if picked.len() < BLEND_MIN_COLORS {
            return None;
        }

        // Karışımda renkler canlı kalsın ama yazıyı boğmasın: orta açıklık, yüksek doygunluk (App alfa ~0,35 ile çizer).
        let mut colors = Vec::with_capacity(picked.len());
        for &i in &picked {
            colors.push(ColorMath::from_hsl(hue_sum[i] / weight[i], text::clamp(sat_sum[i] / weight[i], 0.55, 0.85), 0.5));
        }
        Some(colors)
    }
}

/// `double.CompareTo`: NaN her değerden küçük sayılır, iki NaN eşittir.
fn compare_to(a: f64, b: f64) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else if a == b {
        0
    } else if a.is_nan() {
        if b.is_nan() { 0 } else { -1 }
    } else {
        1
    }
}

// --- .NET `ArraySortHelper<T>.IntrospectiveSort` (Comparison sürümü), birebir ---

const INTROSORT_SIZE_THRESHOLD: usize = 16;

fn dotnet_sort<T: Clone>(keys: &mut [T], comparer: impl Fn(&T, &T) -> i32 + Copy) {
    if keys.len() > 1 {
        let depth_limit = 2 * (keys.len().ilog2() as usize + 1);
        intro_sort(keys, depth_limit, comparer);
    }
}

fn swap_if_greater<T>(keys: &mut [T], comparer: impl Fn(&T, &T) -> i32, i: usize, j: usize) {
    if i != j && comparer(&keys[i], &keys[j]) > 0 {
        keys.swap(i, j);
    }
}

fn intro_sort<T: Clone>(keys: &mut [T], mut depth_limit: usize, comparer: impl Fn(&T, &T) -> i32 + Copy) {
    let mut partition_size = keys.len();
    while partition_size > 1 {
        if partition_size <= INTROSORT_SIZE_THRESHOLD {
            if partition_size == 2 {
                swap_if_greater(keys, comparer, 0, 1);
                return;
            }
            if partition_size == 3 {
                swap_if_greater(keys, comparer, 0, 1);
                swap_if_greater(keys, comparer, 0, 2);
                swap_if_greater(keys, comparer, 1, 2);
                return;
            }
            insertion_sort(&mut keys[..partition_size], comparer);
            return;
        }

        if depth_limit == 0 {
            heap_sort(&mut keys[..partition_size], comparer);
            return;
        }
        depth_limit -= 1;

        let p = pick_pivot_and_partition(&mut keys[..partition_size], comparer);

        // Eksenin etrafında bölündü; eksenin yeniden taşınmasına gerek yok.
        intro_sort(&mut keys[p + 1..partition_size], depth_limit, comparer);
        partition_size = p;
    }
}

fn pick_pivot_and_partition<T: Clone>(keys: &mut [T], comparer: impl Fn(&T, &T) -> i32 + Copy) -> usize {
    let hi = keys.len() - 1;
    // Üç değerin ortancasını hesapla.
    let middle = hi >> 1;
    swap_if_greater(keys, comparer, 0, middle);
    swap_if_greater(keys, comparer, 0, hi);
    swap_if_greater(keys, comparer, middle, hi);

    let pivot = keys[middle].clone();
    keys.swap(middle, hi - 1);
    let mut left = 0usize;
    let mut right = hi - 1;

    while left < right {
        loop {
            left += 1;
            if !(comparer(&keys[left], &pivot) < 0) {
                break;
            }
        }
        loop {
            right -= 1;
            if !(comparer(&pivot, &keys[right]) < 0) {
                break;
            }
        }

        if left >= right {
            break;
        }

        keys.swap(left, right);
    }

    if left != hi - 1 {
        keys.swap(left, hi - 1);
    }
    left
}

fn heap_sort<T: Clone>(keys: &mut [T], comparer: impl Fn(&T, &T) -> i32 + Copy) {
    let n = keys.len();
    let mut i = n >> 1;
    while i >= 1 {
        down_heap(keys, i, n, comparer);
        i -= 1;
    }

    let mut i = n;
    while i > 1 {
        keys.swap(0, i - 1);
        down_heap(keys, 1, i - 1, comparer);
        i -= 1;
    }
}

fn down_heap<T: Clone>(keys: &mut [T], mut i: usize, n: usize, comparer: impl Fn(&T, &T) -> i32) {
    let d = keys[i - 1].clone();
    while i <= n >> 1 {
        let mut child = 2 * i;
        if child < n && comparer(&keys[child - 1], &keys[child]) < 0 {
            child += 1;
        }

        if !(comparer(&d, &keys[child - 1]) < 0) {
            break;
        }

        keys[i - 1] = keys[child - 1].clone();
        i = child;
    }
    keys[i - 1] = d;
}

fn insertion_sort<T: Clone>(keys: &mut [T], comparer: impl Fn(&T, &T) -> i32) {
    for i in 0..keys.len().saturating_sub(1) {
        let t = keys[i + 1].clone();
        let mut j = i as isize;
        while j >= 0 && comparer(&t, &keys[j as usize]) < 0 {
            keys[(j + 1) as usize] = keys[j as usize].clone();
            j -= 1;
        }
        keys[(j + 1) as usize] = t;
    }
}
