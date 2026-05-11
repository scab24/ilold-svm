use crate::colors::{c_accent, c_bright, c_muted};

pub use ilold_help::SOLANA_HELP_BLOCKS;
#[allow(unused_imports)]
pub use ilold_help::HelpBlock;

pub fn render_solana_help_block(cmd: &str) -> Option<String> {
    let key = cmd.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }
    let block = SOLANA_HELP_BLOCKS
        .iter()
        .find(|b| b.aliases.iter().any(|a| *a == key))?;

    let mut out = String::new();
    out.push('\n');
    out.push_str(&format!("  {}\n\n", c_bright(block.title)));

    out.push_str(&format!("  {}\n", c_accent("Purpose")));
    out.push_str(&format!("    {}\n", block.purpose));

    if !block.syntax.is_empty() {
        out.push('\n');
        out.push_str(&format!("  {}\n", c_accent("Syntax")));
        let pad = block.syntax.iter().map(|(s, _)| s.len()).max().unwrap_or(0);
        for (form, note) in block.syntax {
            if note.is_empty() {
                out.push_str(&format!("    {}\n", form));
            } else {
                out.push_str(&format!(
                    "    {:<width$}  {}\n",
                    form,
                    c_muted(note),
                    width = pad
                ));
            }
        }
    }

    if !block.flags.is_empty() {
        out.push('\n');
        out.push_str(&format!("  {}\n", c_accent("Flags")));
        let pad = block.flags.iter().map(|(f, _)| f.len()).max().unwrap_or(0);
        for (flag, desc) in block.flags {
            out.push_str(&format!(
                "    {:<width$}  {}\n",
                flag,
                c_muted(desc),
                width = pad
            ));
        }
    }

    if !block.examples.is_empty() {
        out.push('\n');
        out.push_str(&format!("  {}\n", c_accent("Examples")));
        let pad = block.examples.iter().map(|(e, _)| e.len()).max().unwrap_or(0);
        for (ex, note) in block.examples {
            if note.is_empty() {
                out.push_str(&format!("    {}\n", ex));
            } else {
                out.push_str(&format!(
                    "    {:<width$}  {}\n",
                    ex,
                    c_muted(note),
                    width = pad
                ));
            }
        }
    }

    if !block.returns.is_empty() {
        out.push('\n');
        out.push_str(&format!("  {}\n", c_accent("Returns")));
        out.push_str(&format!("    {}\n", block.returns));
    }

    if !block.see_also.is_empty() {
        out.push('\n');
        out.push_str(&format!("  {}\n", c_accent("See also")));
        out.push_str(&format!("    {}\n", block.see_also.join(", ")));
    }

    out.push('\n');
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aliases_for(title_prefix: &str) -> &'static [&'static str] {
        SOLANA_HELP_BLOCKS
            .iter()
            .find(|b| b.title.starts_with(title_prefix))
            .map(|b| b.aliases)
            .expect("block exists")
    }

    #[test]
    fn renders_call_block_with_all_sections() {
        let out = render_solana_help_block("call").expect("call block");
        assert!(out.contains("Purpose"));
        assert!(out.contains("Syntax"));
        assert!(out.contains("Flags"));
        assert!(out.contains("Examples"));
        assert!(out.contains("Returns"));
        assert!(out.contains("See also"));
        assert!(out.contains("--signer="));
        assert!(out.contains("StepAdded"));
    }

    #[test]
    fn aliases_render_identical_text() {
        let by_short = render_solana_help_block("c").expect("c");
        let by_long = render_solana_help_block("call").expect("call");
        assert_eq!(by_short, by_long);

        let info_short = render_solana_help_block("i").expect("i");
        let info_long = render_solana_help_block("info").expect("info");
        assert_eq!(info_short, info_long);
    }

    #[test]
    fn who_block_documents_three_query_kinds() {
        let out = render_solana_help_block("who").expect("who");
        assert!(out.contains("AccountType"));
        assert!(out.contains("Instruction"));
        assert!(out.contains("Field"));
    }

    #[test]
    fn scenario_block_lists_all_subcommands() {
        let out = render_solana_help_block("scenario").expect("scenario");
        for sub in ["new", "list", "switch", "fork", "delete"] {
            assert!(out.contains(sub), "scenario help missing {sub}");
        }
    }

    #[test]
    fn save_block_documents_with_keypairs_flag() {
        let out = render_solana_help_block("save").expect("save");
        assert!(out.contains("--with-keypairs"));
        assert!(out.contains("do NOT commit"));
    }

    #[test]
    fn help_block_returns_self() {
        let out = render_solana_help_block("?").expect("?");
        assert!(out.contains("Append ? to any command"));
    }

    #[test]
    fn unknown_command_returns_none() {
        assert!(render_solana_help_block("xxxx").is_none());
        assert!(render_solana_help_block("").is_none());
    }

    #[test]
    fn omits_sections_when_empty() {
        let out = render_solana_help_block("back").expect("back");
        assert!(!out.contains("Flags"));
        assert!(!out.contains("See also\n    \n"));
        assert!(out.contains("See also"));
    }

    #[test]
    fn lookup_is_case_insensitive_on_input() {
        let lower = render_solana_help_block("call").expect("call");
        let upper = render_solana_help_block("CALL").expect("CALL");
        assert_eq!(lower, upper);
    }

    #[test]
    fn every_solana_command_has_a_help_block() {
        let registered: &[&str] = &[
            "?", "help", "h",
            "quit", "q", "exit",
            "funcs", "functions", "f",
            "funcs-all", "fa",
            "info", "i",
            "vars", "v", "vars-all", "va",
            "coupling", "cp",
            "state",
            "session", "s",
            "back",
            "clear",
            "users",
            "airdrop", "air",
            "time-warp", "tw",
            "pda",
            "inspect", "acc",
            "call", "c",
            "ct", "contracts", "programs", "progs",
            "use",
            "sc", "scenario",
            "note", "n",
            "fi", "finding",
            "seq", "sequence",
            "browser",
            "step", "st",
            "save",
            "load",
            "findings", "fl",
            "export", "ex",
            "who",
            "timeline", "tl",
            "status",
        ];
        for name in registered {
            assert!(
                render_solana_help_block(name).is_some(),
                "missing HelpBlock for command alias `{name}`"
            );
        }
    }

    #[test]
    fn aliases_for_helper_finds_blocks() {
        let call_aliases = aliases_for("c | call");
        assert!(call_aliases.contains(&"c"));
        assert!(call_aliases.contains(&"call"));
    }
}
