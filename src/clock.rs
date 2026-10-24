//! Local time without a date library: the phone gives us milliseconds and the time zone offset.

use crate::platform;

#[derive(Default, Clone, Debug)]
pub struct ClockInfo {
    pub hour: u32,
    pub min: u32,
    pub sec: u32,
    pub day: u32,
    pub month: u32,
    pub weekday: usize, // 0 = Sunday
    pub h24: bool,
}

const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const DAYS_FULL: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

impl ClockInfo {
    pub fn now() -> ClockInfo {
        let ms = platform::local_millis();
        let days = ms.div_euclid(86_400_000);
        let secs = ms.rem_euclid(86_400_000) / 1000;
        // days since 1970 to a calendar date (Howard Hinnant's algorithm)
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        ClockInfo {
            hour: (secs / 3600) as u32,
            min: ((secs % 3600) / 60) as u32,
            sec: (secs % 60) as u32,
            day: (doy - (153 * mp + 2) / 5 + 1) as u32,
            month: (if mp < 10 { mp + 3 } else { mp - 9 }) as u32,
            weekday: (days + 4).rem_euclid(7) as usize,
            h24: platform::is_24h(),
        }
    }

    pub fn hour_shown(&self) -> u32 {
        if self.h24 { self.hour } else if self.hour % 12 == 0 { 12 } else { self.hour % 12 }
    }

    /// "21:47" or "9:47"
    pub fn time(&self) -> String {
        if self.h24 { format!("{:02}:{:02}", self.hour, self.min) } else { format!("{}:{:02}", self.hour_shown(), self.min) }
    }

    /// "Sat, Oct 24"
    pub fn date(&self) -> String {
        format!("{}, {} {}", DAYS[self.weekday], MONTHS[(self.month - 1) as usize], self.day)
    }

    /// "SAT 24 OCT"
    pub fn short(&self) -> String {
        format!("{} {} {}", DAYS[self.weekday].to_uppercase(), self.day, MONTHS[(self.month - 1) as usize].to_uppercase())
    }

    pub fn weekday_full(&self) -> &'static str {
        DAYS_FULL[self.weekday]
    }
}
