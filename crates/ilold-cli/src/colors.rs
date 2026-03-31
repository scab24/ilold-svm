use colored::{Colorize, ColoredString};
use ilold_core::classify::entry_points::AccessLevel;

pub fn c_accent(s: &str) -> ColoredString { s.truecolor(140, 170, 210) }
pub fn c_warn(s: &str) -> ColoredString { s.truecolor(180, 150, 80) }
pub fn c_danger(s: &str) -> ColoredString { s.truecolor(170, 90, 90) }
pub fn c_ok(s: &str) -> ColoredString { s.truecolor(100, 160, 110) }
pub fn c_muted(s: &str) -> ColoredString { s.truecolor(110, 120, 140) }
pub fn c_bright(s: &str) -> ColoredString { s.truecolor(190, 200, 215).bold() }

pub fn access_colored(access: &AccessLevel) -> ColoredString {
    match access {
        AccessLevel::Public => c_accent("[P]"),
        AccessLevel::Restricted { .. } => c_warn("[R]"),
        AccessLevel::Internal => c_muted("[I]"),
        AccessLevel::Special { .. } => c_muted("[S]"),
    }
}
