use super::DisplayMonitor;
use crate::text;

/// Ekranlar sayfasındaki şemada tek bir monitörün dikdörtgeni (şema koordinatları, DIP).
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaTile {
    pub id: String,
    pub number: i32,
    pub is_primary: bool,
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// Şemanın toplam boyutu ve içindeki dikdörtgenler.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaLayout {
    pub width: f64,
    pub height: f64,
    pub tiles: Vec<SchemaTile>,
}

impl SchemaLayout {
    /// `SchemaLayout.Empty` (C#'ta tek paylaşılan örnek; burada eşitlikle karşılaştırılır).
    pub fn empty() -> SchemaLayout {
        SchemaLayout { width: 0.0, height: 0.0, tiles: Vec::new() }
    }
}

/// Gerçek monitör yerleşimini (sanal masaüstü koordinatları) orantılı küçültüp şemaya sığdıran saf hesap.
/// Ekran numarası: ana ekran 1, diğerleri soldan sağa, sonra yukarıdan aşağı. Windows'un kendi numarası
/// sürücü sırasına bağlı olduğundan ayrı, tahmin edilebilir bir sıra kullanılır; "Ekranları tanımla" da aynı numarayı gösterir.
pub struct DisplaySchema;

impl DisplaySchema {
    /// Ekranları numara sırasına dizer (ana ekran, sonra soldan sağa, yukarıdan aşağı).
    pub fn order(monitors: &[DisplayMonitor]) -> Vec<DisplayMonitor> {
        let mut list = monitors.to_vec();
        // OrderByDescending(IsPrimary).ThenBy(Left).ThenBy(Top).ThenBy(Id, OrdinalIgnoreCase): kararlı sıralama.
        list.sort_by(|a, b| {
            b.is_primary
                .cmp(&a.is_primary)
                .then(a.bounds.left.cmp(&b.bounds.left))
                .then(a.bounds.top.cmp(&b.bounds.top))
                .then_with(|| text::cmp_ignore_case(&a.id, &b.id))
        });
        list
    }

    /// Monitörün şema/tanımla numarası (1'den başlar); listede yoksa 0.
    pub fn number_of(monitors: &[DisplayMonitor], id: &str) -> i32 {
        let ordered = Self::order(monitors);
        for (i, m) in ordered.iter().enumerate() {
            if text::eq_ignore_case(&m.id, id) {
                return i as i32 + 1;
            }
        }
        0
    }

    /// Ekranları en fazla `max_width` × `max_height` alana tek ölçekle sığdırır (en-boy oranı korunur, büyütme
    /// yapılmaz). Bitişik ekranlar tek blok görünmesin diye her dikdörtgen `gap` kadar içeri çekilir.
    /// C# varsayılanı `gap = 8`.
    pub fn compute(monitors: &[DisplayMonitor], max_width: f64, max_height: f64, gap: f64) -> SchemaLayout {
        let valid: Vec<DisplayMonitor> = monitors.iter().filter(|m| !m.bounds.is_empty()).cloned().collect();
        if valid.is_empty() || max_width <= 0.0 || max_height <= 0.0 {
            return SchemaLayout::empty();
        }

        let min_x = valid.iter().map(|m| m.bounds.left).min().unwrap();
        let min_y = valid.iter().map(|m| m.bounds.top).min().unwrap();
        let span_x = (valid.iter().map(|m| m.bounds.right).max().unwrap() - min_x) as f64;
        let span_y = (valid.iter().map(|m| m.bounds.bottom).max().unwrap() - min_y) as f64;

        // Sanal masaüstü koordinatları fiziksel pikseldir ve ekranlar arasında tutarlıdır; ölçek tek.
        let scale = f64::min(f64::min(max_width / span_x, max_height / span_y), 1.0);

        let ordered = Self::order(&valid);
        let mut tiles = Vec::with_capacity(ordered.len());
        for (i, m) in ordered.iter().enumerate() {
            let width = m.bounds.width() as f64 * scale;
            let height = m.bounds.height() as f64 * scale;
            let inset_x = f64::min(gap / 2.0, width / 4.0);
            let inset_y = f64::min(gap / 2.0, height / 4.0);

            tiles.push(SchemaTile {
                id: m.id.clone(),
                number: i as i32 + 1,
                is_primary: m.is_primary,
                left: (m.bounds.left - min_x) as f64 * scale + inset_x,
                top: (m.bounds.top - min_y) as f64 * scale + inset_y,
                width: width - 2.0 * inset_x,
                height: height - 2.0 * inset_y,
            });
        }

        SchemaLayout { width: span_x * scale, height: span_y * scale, tiles }
    }
}
