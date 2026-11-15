//! Karşılığı: `Weather/WeatherCodeMapper.cs`.

pub struct WeatherCodeMapper;

impl WeatherCodeMapper {
    /// WMO hava kodunun çeviri anahtarı; metni arayüz katmanı çözer (Strings.resx).
    pub fn to_description_key(code: i32) -> &'static str {
        match code {
            0 => "WeatherCondClear",
            1 => "WeatherCondMostlyClear",
            2 => "WeatherCondPartlyCloudy",
            3 => "WeatherCondOvercast",
            45 => "WeatherCondFog",
            48 => "WeatherCondRimeFog",
            51 => "WeatherCondLightDrizzle",
            53 => "WeatherCondModerateDrizzle",
            55 => "WeatherCondDenseDrizzle",
            56 | 57 => "WeatherCondFreezingDrizzle",
            61 => "WeatherCondLightRain",
            63 => "WeatherCondRain",
            65 => "WeatherCondHeavyRain",
            66 | 67 => "WeatherCondFreezingRain",
            71 => "WeatherCondLightSnow",
            73 => "WeatherCondSnow",
            75 => "WeatherCondHeavySnow",
            77 => "WeatherCondSnowGrains",
            80 => "WeatherCondLightShowers",
            81 => "WeatherCondShowers",
            82 => "WeatherCondHeavyShowers",
            85 => "WeatherCondLightSnowShowers",
            86 => "WeatherCondHeavySnowShowers",
            95 => "WeatherCondThunderstorm",
            96 | 99 => "WeatherCondHailStorm",
            _ => "WeatherCondUnknown",
        }
    }

    pub fn to_icon_kind(code: i32) -> &'static str {
        match code {
            0 => "Sun",
            1 | 2 => "PartlyCloudy",
            3 => "Cloud",
            45 | 48 => "Fog",
            51 | 53 | 55 | 56 | 57 => "Drizzle",
            61 | 63 | 65 | 66 | 67 | 80 | 81 | 82 => "Rain",
            71 | 73 | 75 | 77 | 85 | 86 => "Snow",
            95 | 96 | 99 => "Thunderstorm",
            _ => "Cloud",
        }
    }
}
