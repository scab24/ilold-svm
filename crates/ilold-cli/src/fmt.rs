use colored::Colorize;

use ilold_core::model::expression::AssignOperator;
use ilold_core::narrative::trace::{FlowKind, FlowNode, FlowTree};

use crate::colors::{c_accent, c_bright, c_danger, c_muted, c_ok, c_warn};

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

// Flow tree renderer — trace command output.

pub fn render_flow_tree(tree: &FlowTree) -> String {
    let mut out = String::new();

    let title = format!("{}::{}", tree.contract, tree.signature);
    let mods = if tree.modifiers.is_empty() {
        "modifiers: (none)".to_string()
    } else {
        format!("modifiers: {}", tree.modifiers.join(", "))
    };
    let depth = format!("max inlining depth: {}", tree.max_depth);
    let lines: Vec<&str> = vec![&title, &mods, &depth];
    out.push_str(&header_box(&lines));
    out.push('\n');
    out.push('\n');

    let mut step: usize = 1;
    render_flow_root(&tree.root, &mut step, &mut out);

    out
}

fn render_flow_root(root: &FlowNode, step: &mut usize, out: &mut String) {
    let (icon, text) = format_flow_label(&root.kind);
    let gutter = format_gutter(*step);
    *step += 1;
    out.push_str(&format!(
        "{} {} {}\n",
        gutter,
        c_accent(icon),
        c_bright(&text),
    ));

    let n = root.children.len();
    for (i, child) in root.children.iter().enumerate() {
        render_flow_node(child, "", i == n - 1, step, out);
    }
}

fn render_flow_node(
    node: &FlowNode,
    parent_prefix: &str,
    is_last: bool,
    step: &mut usize,
    out: &mut String,
) {
    let connector = if is_last { "└─ " } else { "├─ " };
    let (icon, text) = format_flow_label(&node.kind);
    let gutter = format_gutter(*step);
    *step += 1;

    let suffix = match &node.from_modifier {
        Some(name) => format!("  {}", c_warn(&format!("[from: {}]", name))),
        None => String::new(),
    };

    let colored_icon = color_for_kind(&node.kind, icon);
    let colored_text = color_for_kind_text(&node.kind, &text);

    out.push_str(&format!(
        "{} {}{}{} {}{}\n",
        gutter,
        c_muted(parent_prefix),
        c_muted(connector),
        colored_icon,
        colored_text,
        suffix,
    ));

    let extension = if is_last { "   " } else { "│  " };
    let new_prefix = format!("{}{}", parent_prefix, extension);
    let n = node.children.len();
    for (i, child) in node.children.iter().enumerate() {
        render_flow_node(child, &new_prefix, i == n - 1, step, out);
    }
}

fn format_gutter(step: usize) -> String {
    format!("  {} {}", c_accent(&format!("{:03}", step)), c_muted("│"))
}

fn format_flow_label(kind: &FlowKind) -> (&'static str, String) {
    match kind {
        FlowKind::Entry { signature } => ("▶", signature.clone()),
        FlowKind::Require { condition, message } => {
            let text = match message {
                Some(m) => format!("require({}, {})", condition, m),
                None => format!("require({})", condition),
            };
            ("◇", text)
        }
        FlowKind::Assert { condition } => ("◇", format!("assert({})", condition)),
        FlowKind::Write { target, value, op } => {
            ("✏", format!("{} {} {}", target, assign_op_str(*op), value))
        }
        FlowKind::StateWrite { variable } => ("✏", format!("write {}", variable)),
        FlowKind::StateRead { variable } => ("▸", format!("read {}", variable)),
        FlowKind::InternalCall {
            function,
            origin,
            depth_limited,
            ops_count,
        } => {
            let mut text = function.clone();
            if !origin.is_empty() {
                text.push_str(&format!("  [from: {}]", origin));
            }
            if *depth_limited {
                text.push_str(&format!("  [+{} ops, depth limited]", ops_count));
            }
            ("○", text)
        }
        FlowKind::ExternalCall { target, function } => {
            ("→", format!("{}.{}", target, function))
        }
        FlowKind::EmitEvent { name } => ("◆", format!("emit {}", name)),
        FlowKind::BranchTrue { condition } => ("?", format!("if ({}) == true", condition)),
        FlowKind::BranchFalse { condition } => ("?", format!("if ({}) == false", condition)),
        FlowKind::LoopHeader { kind } => ("↻", format!("loop ({})", kind)),
        FlowKind::Return => ("✓", "return".to_string()),
        FlowKind::Revert { reason } => {
            let text = match reason {
                Some(r) => format!("revert({})", r),
                None => "revert".to_string(),
            };
            ("✗", text)
        }
        FlowKind::EthTransfer { to } => ("$", format!("transfer ETH → {}", to)),
        FlowKind::AssemblyBlock => ("⚙", "assembly { … }".to_string()),
        FlowKind::DepthLimit => ("…", "depth limit".to_string()),
    }
}

fn color_for_kind(kind: &FlowKind, icon: &str) -> colored::ColoredString {
    match kind {
        FlowKind::Entry { .. } => c_accent(icon),
        FlowKind::Require { .. } | FlowKind::Assert { .. } => c_muted(icon),
        FlowKind::Write { .. } | FlowKind::StateWrite { .. } => c_danger(icon),
        FlowKind::StateRead { .. } => c_muted(icon),
        FlowKind::InternalCall { .. } => c_warn(icon),
        FlowKind::ExternalCall { .. } | FlowKind::EthTransfer { .. } => c_danger(icon),
        FlowKind::EmitEvent { .. } => c_accent(icon),
        FlowKind::BranchTrue { .. } | FlowKind::BranchFalse { .. } => c_warn(icon),
        FlowKind::LoopHeader { .. } => c_warn(icon),
        FlowKind::Return => c_ok(icon),
        FlowKind::Revert { .. } => c_danger(icon),
        FlowKind::AssemblyBlock => c_warn(icon),
        FlowKind::DepthLimit => c_muted(icon),
    }
}

fn color_for_kind_text(kind: &FlowKind, text: &str) -> colored::ColoredString {
    match kind {
        FlowKind::Entry { .. } => c_bright(text),
        FlowKind::Write { .. } | FlowKind::StateWrite { .. } => c_danger(text),
        FlowKind::ExternalCall { .. } | FlowKind::EthTransfer { .. } => c_danger(text),
        FlowKind::InternalCall { .. } => c_warn(text),
        FlowKind::BranchTrue { .. } | FlowKind::BranchFalse { .. } => c_warn(text),
        FlowKind::Revert { .. } => c_danger(text),
        FlowKind::Return => c_ok(text),
        _ => c_muted(text),
    }
}

fn assign_op_str(op: AssignOperator) -> &'static str {
    match op {
        AssignOperator::Assign => "=",
        AssignOperator::AddAssign => "+=",
        AssignOperator::SubAssign => "-=",
        AssignOperator::MulAssign => "*=",
        AssignOperator::DivAssign => "/=",
        AssignOperator::ModAssign => "%=",
        AssignOperator::BitAndAssign => "&=",
        AssignOperator::BitOrAssign => "|=",
        AssignOperator::BitXorAssign => "^=",
        AssignOperator::ShlAssign => "<<=",
        AssignOperator::ShrAssign => ">>=",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_ansi(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut in_esc = false;
        for c in s.chars() {
            if in_esc {
                if c == 'm' {
                    in_esc = false;
                }
                continue;
            }
            if c == '\u{1b}' {
                in_esc = true;
                continue;
            }
            out.push(c);
        }
        out
    }

    fn make_require_tree(message: Option<&str>) -> FlowTree {
        FlowTree {
            contract: "C".into(),
            function: "f".into(),
            signature: "f()".into(),
            modifiers: vec![],
            max_depth: 2,
            root: FlowNode {
                step_id: 0,
                depth: 0,
                kind: FlowKind::Entry { signature: "f()".into() },
                from_modifier: None,
                children: vec![FlowNode {
                    step_id: 1,
                    depth: 1,
                    kind: FlowKind::Require {
                        condition: "x == 1".into(),
                        message: message.map(|s| s.into()),
                    },
                    from_modifier: None,
                    children: vec![],
                }],
            },
        }
    }

    #[test]
    fn render_require_message_not_double_escaped() {
        // Regression: `{:?}` on the message used to produce `"\"LOCKED\""`.
        // After the fix with `{}`, the output should contain the unescaped
        // form `"LOCKED"`.
        let tree = make_require_tree(Some("\"LOCKED\""));
        let rendered = strip_ansi(&render_flow_tree(&tree));

        assert!(
            rendered.contains("require(x == 1, \"LOCKED\")"),
            "expected unescaped message in output.\nrendered:\n{}",
            rendered,
        );
        assert!(
            !rendered.contains("\\\""),
            "rendered output must not contain backslash-escaped quotes.\nrendered:\n{}",
            rendered,
        );
    }

    #[test]
    fn render_require_without_message() {
        let tree = make_require_tree(None);
        let rendered = strip_ansi(&render_flow_tree(&tree));
        assert!(
            rendered.contains("require(x == 1)"),
            "rendered:\n{}",
            rendered,
        );
    }
}
