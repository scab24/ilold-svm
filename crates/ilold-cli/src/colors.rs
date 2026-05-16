pub use ilold_render::colors::*;

use colored::Colorize;

pub fn header_box(lines: &[&str]) -> String {
    let max_content = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let inner_width = max_content + 2;

    let mut out = String::new();
    out.push_str(&format!(
        "  {}{}{}\n",
        "╭".truecolor(60, 70, 90),
        "─".repeat(inner_width).truecolor(60, 70, 90),
        "╮".truecolor(60, 70, 90),
    ));
    for line in lines {
        let visible_len = line.chars().count();
        let pad = max_content.saturating_sub(visible_len);
        out.push_str(&format!(
            "  {} {}{} {}\n",
            "│".truecolor(60, 70, 90),
            line,
            " ".repeat(pad),
            "│".truecolor(60, 70, 90),
        ));
    }
    out.push_str(&format!(
        "  {}{}{}",
        "╰".truecolor(60, 70, 90),
        "─".repeat(inner_width).truecolor(60, 70, 90),
        "╯".truecolor(60, 70, 90),
    ));
    out
}

pub fn pad_right(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - len))
    }
}
