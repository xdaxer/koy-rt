//! Karşılığı: `WeatherCodeMapperTests.cs`.

use crate::weather::WeatherCodeMapper;

#[test]
fn maps_wmo_codes_correctly() {
    let cases = [
        (0, "WeatherCondClear", "Sun"),
        (1, "WeatherCondMostlyClear", "PartlyCloudy"),
        (2, "WeatherCondPartlyCloudy", "PartlyCloudy"),
        (3, "WeatherCondOvercast", "Cloud"),
        (45, "WeatherCondFog", "Fog"),
        (61, "WeatherCondLightRain", "Rain"),
        (71, "WeatherCondLightSnow", "Snow"),
        (95, "WeatherCondThunderstorm", "Thunderstorm"),
        (999, "WeatherCondUnknown", "Cloud"),
    ];
    for (code, expected_key, expected_icon) in cases {
        assert_eq!(expected_key, WeatherCodeMapper::to_description_key(code), "{code}");
        assert_eq!(expected_icon, WeatherCodeMapper::to_icon_kind(code), "{code}");
    }
}
