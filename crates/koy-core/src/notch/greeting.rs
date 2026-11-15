/// Günün hangi diliminde olduğumuz; Ana'daki selamlama (müzik çalmıyorken) için.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DayPart {
    #[default]
    Morning,
    Afternoon,
    Evening,
    Night,
}

pub struct Greeting;

impl Greeting {
    /// 05-11 sabah, 12-17 gün, 18-21 akşam, 22-04 gece.
    pub fn for_hour(hour: i32) -> DayPart {
        match hour {
            5..=11 => DayPart::Morning,
            12..=17 => DayPart::Afternoon,
            18..=21 => DayPart::Evening,
            _ => DayPart::Night,
        }
    }
}
