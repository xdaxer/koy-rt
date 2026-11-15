/// Bluetooth SIG GATT Battery Service (0x180F) ve Battery Level (0x2A19) karakteristiği için saf ayrıştırıcı.
/// Standart gereği pil seviyesi 0..100 arasında tek bayttır.
pub struct GattBatteryParser;

impl GattBatteryParser {
    /// Ham GATT karakteristik verisini (0x2A19) pil yüzdesine (0..100) çevirir.
    /// Veri boşsa veya 100'den büyükse (Bluetooth SIG reserved) `None` döner.
    pub fn parse(raw: &[u8]) -> Option<i32> {
        let first = *raw.first()?;
        let value = first as i32;
        if value <= 100 { Some(value) } else { None }
    }

    /// Çoklu pil karakteristiği (örn. TWS kulaklıkların sol/sağ modülleri) okunduğunda
    /// kullanıcıya uyarı vermek adına en düşük (kritik) pil seviyesini seçer.
    pub fn parse_lowest(readings: Option<&[&[u8]]>) -> Option<i32> {
        let readings = readings?;

        let mut lowest: Option<i32> = None;
        for reading in readings {
            if let Some(parsed) = Self::parse(reading)
                && lowest.is_none_or(|l| parsed < l)
            {
                lowest = Some(parsed);
            }
        }
        lowest
    }
}
