use colored::Colorize;

pub fn term_width() -> usize {
    terminal_size::terminal_size()
        .map(|(terminal_size::Width(w), _)| w as usize)
        .unwrap_or(80)
}

pub fn separator(label: &str) -> String {
    let width = term_width().min(100);
    let inner = format!("[ {} ]", label);
    let remaining = width.saturating_sub(inner.len() + 2);
    let left = remaining / 2;
    let right = remaining - left;
    format!(
        "  {}{}{}",
        "═".repeat(left).truecolor(60, 70, 90),
        inner.truecolor(190, 200, 215),
        "═".repeat(right).truecolor(60, 70, 90),
    )
}

/// Pass PLAIN text lines (no ANSI colors). The box handles its own coloring.
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
