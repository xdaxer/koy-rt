//! Karşılığı: `koy/src/Core/Weather/`.
//!
//! Open-Meteo hava durumu/geocoding yanıt ayrıştırıcıları, Nominatim yer adı ayrıştırıcısı, WMO kod eşlemesi,
//! konum ve yenileme ilkeleri. **Eşzamansızlık modeli:** C# `Task` dönen sözleşmeler burada bloklayan çağrıdır
//! (`Async` soneki atılır); iptal `CancellationToken` ile bildirilir (`Result<_, OperationCanceled>`).

pub mod i_weather_service;
pub mod open_meteo_parser;
pub mod place_name_parser;
pub mod weather_code_mapper;
pub mod weather_location;
pub mod weather_location_policy;
pub mod weather_refresh_interval;
pub mod weather_refresh_policy;

pub use i_weather_service::*;
pub use open_meteo_parser::*;
pub use place_name_parser::*;
pub use weather_code_mapper::*;
pub use weather_location::*;
pub use weather_location_policy::*;
pub use weather_refresh_interval::*;
pub use weather_refresh_policy::*;

#[cfg(test)]
mod tests;
