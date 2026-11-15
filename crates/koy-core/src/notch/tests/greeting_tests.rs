use crate::notch::{DayPart, Greeting};

#[test]
fn for_hour_picks_the_day_part() {
    let rows = [
        (5, DayPart::Morning),
        (11, DayPart::Morning),
        (12, DayPart::Afternoon),
        (17, DayPart::Afternoon),
        (18, DayPart::Evening),
        (21, DayPart::Evening),
        (22, DayPart::Night),
        (0, DayPart::Night),
        (4, DayPart::Night),
    ];
    for (hour, expected) in rows {
        assert_eq!(expected, Greeting::for_hour(hour), "saat {hour}");
    }
}
