use crate::colors::{c_accent, c_bright, c_muted};

pub struct HelpBlock {
    pub title: &'static str,
    pub aliases: &'static [&'static str],
    pub purpose: &'static str,
    pub syntax: &'static [(&'static str, &'static str)],
    pub flags: &'static [(&'static str, &'static str)],
    pub examples: &'static [(&'static str, &'static str)],
    pub returns: &'static str,
    pub see_also: &'static [&'static str],
}

pub const SOLANA_HELP_BLOCKS: &[HelpBlock] = &[
    HelpBlock {
        title: "c | call",
        aliases: &["c", "call"],
        purpose: "Run an Anchor instruction against the LiteSVM and append the result as a step on the active scenario.",
        syntax: &[
            ("c <ix> arg=val acc=user", "Concise key=value form (signers auto-resolved from IDL)"),
            ("c <ix> {json}", "Full JSON form: {\"args\":{...},\"accounts\":{...},\"signers\":[...]}"),
        ],
        flags: &[
            ("--signer=a,b", "Force these accounts to sign (override IDL defaults)"),
            ("--no-signer=name", "Remove a default signer (test negative cases)"),
        ],
        examples: &[
            ("c stake amount=1000 pool=pool user_stake=alice_stake user=alice", "Stake 1000 from alice"),
            ("c initialize_pool reward_rate=10 pool=pool admin=admin", "Bootstrap a pool"),
            ("c stake {\"args\":{\"amount\":1000},\"accounts\":{\"pool\":\"pool\",\"user_stake\":\"alice_stake\",\"user\":\"alice\"}}", "Same call in JSON form"),
        ],
        returns: "StepAdded { step_index, instruction, logs_excerpt, account_diffs_count, compute_units } on success, or CallFailed { instruction, logs_excerpt, compute_units, error } when the VM rejects.",
        see_also: &["info", "pda", "state", "step", "back"],
    },
    HelpBlock {
        title: "b | back",
        aliases: &["b", "back"],
        purpose: "Remove the last step from the active scenario and rewind the VM to that point.",
        syntax: &[("b", "")],
        flags: &[],
        examples: &[("b", "Drop the most recent call before resuming exploration")],
        returns: "ScenarioUpdated with the truncated step list.",
        see_also: &["clear", "step", "session"],
    },
    HelpBlock {
        title: "cl | clear",
        aliases: &["cl", "clear"],
        purpose: "Reset the active scenario steps and the underlying VM state.",
        syntax: &[("cl", "")],
        flags: &[],
        examples: &[("cl", "Wipe the timeline before starting a new attack flow")],
        returns: "ScenarioUpdated with an empty step list.",
        see_also: &["back", "scenario", "session"],
    },
    HelpBlock {
        title: "state",
        aliases: &["state"],
        purpose: "Show the decoded view of every account mutated during the active scenario.",
        syntax: &[("state", "")],
        flags: &[],
        examples: &[("state", "Inspect post-step balances and PDA contents at the latest step")],
        returns: "StateView { accounts: [{ pubkey, decoded_fields, owner_program, ... }] }.",
        see_also: &["timeline", "inspect", "session"],
    },
    HelpBlock {
        title: "s | session",
        aliases: &["s", "session"],
        purpose: "Print the active scenario summary: ordered steps, findings, notes.",
        syntax: &[("s", "")],
        flags: &[],
        examples: &[("s", "Recap what has been called so far and which findings are attached")],
        returns: "SessionView { scenario_name, steps, findings, notes }.",
        see_also: &["scenario", "state", "findings"],
    },
    HelpBlock {
        title: "ct | programs | contracts",
        aliases: &["ct", "programs", "progs", "contracts"],
        purpose: "List every program detected in the workspace (multi-program Anchor workspaces included).",
        syntax: &[("ct", "")],
        flags: &[],
        examples: &[("ct", "Discover the available programs before switching with use")],
        returns: "Plain list of program names with the active one marked.",
        see_also: &["use", "funcs", "vars"],
    },
    HelpBlock {
        title: "use",
        aliases: &["use"],
        purpose: "Switch the active program so subsequent commands target it.",
        syntax: &[("use <program>", "")],
        flags: &[],
        examples: &[("use staking", "Make staking the active program")],
        returns: "Updates the prompt label and the completer source.",
        see_also: &["programs", "funcs", "vars"],
    },
    HelpBlock {
        title: "f | funcs | functions",
        aliases: &["f", "funcs", "functions"],
        purpose: "List the instructions exposed by the active program.",
        syntax: &[("f", "")],
        flags: &[],
        examples: &[("f", "Enumerate callable instructions with arg/account counts")],
        returns: "FuncsList { instructions: [{ name, arg_count, account_count, signer_count, pda_count }] }.",
        see_also: &["funcs-all", "info", "vars"],
    },
    HelpBlock {
        title: "fa | funcs-all",
        aliases: &["fa", "funcs-all"],
        purpose: "List instructions with full counts plus admin-gating and coupling hints (T-R50 ProgramView).",
        syntax: &[("fa", "")],
        flags: &[],
        examples: &[("fa", "Spot admin-only entry points and shared-writable couplings at a glance")],
        returns: "FuncsAll { instructions: [{ name, args, accounts, signers, pdas, admin_gated, couples_with }] }.",
        see_also: &["funcs", "info", "coupling"],
    },
    HelpBlock {
        title: "i | info",
        aliases: &["i", "info"],
        purpose: "Detail an instruction: typed args, accounts with flags, signers, PDAs, discriminator.",
        syntax: &[("i <ix>", "")],
        flags: &[],
        examples: &[
            ("i stake", "Inspect the stake instruction in full"),
            ("info initialize_pool", ""),
        ],
        returns: "InstructionDetail { name, args, accounts, signers, pdas, discriminator }.",
        see_also: &["funcs-all", "pda", "who", "call"],
    },
    HelpBlock {
        title: "v | vars",
        aliases: &["v", "vars", "vars-all", "va"],
        purpose: "List declared account types with their Anchor discriminators.",
        syntax: &[
            ("v", "Compact view"),
            ("va", "Same as v in current Solana backend (full layout)"),
        ],
        flags: &[],
        examples: &[("v", "List Pool, UserStake, etc. with discriminators")],
        returns: "VarsList { account_types: [{ name, discriminator, fields }] }.",
        see_also: &["who", "inspect", "funcs"],
    },
    HelpBlock {
        title: "users",
        aliases: &["users"],
        purpose: "Manage the keypair set in the active scenario: list existing users or mint a new one.",
        syntax: &[
            ("users", "List keypairs"),
            ("users new <name> [lamports]", "Create keypair and airdrop (default 10 SOL)"),
        ],
        flags: &[],
        examples: &[
            ("users", "Show all named keypairs"),
            ("users new alice", "Create alice with 10 SOL"),
            ("users new bob 5000000000", "Create bob with 5 SOL"),
        ],
        returns: "UsersList { users: [{ name, pubkey, lamports }] } or UsersUpdated.",
        see_also: &["airdrop", "inspect", "scenario"],
    },
    HelpBlock {
        title: "airdrop",
        aliases: &["airdrop", "air"],
        purpose: "Top up an existing keypair with extra lamports.",
        syntax: &[("airdrop <user> <lamports>", "")],
        flags: &[],
        examples: &[("airdrop alice 1000000000", "Add 1 SOL to alice")],
        returns: "UsersUpdated reflecting the new balance.",
        see_also: &["users", "inspect"],
    },
    HelpBlock {
        title: "tw | time-warp",
        aliases: &["tw", "time-warp"],
        purpose: "Advance (or rewind) the Clock sysvar so vesting / reward / lockup logic can be exercised.",
        syntax: &[("tw <delta_seconds>", "Positive moves forward, negative back")],
        flags: &[],
        examples: &[
            ("tw 86400", "Skip one day"),
            ("tw -3600", "Rewind one hour"),
        ],
        returns: "ClockUpdated { unix_timestamp, slot }.",
        see_also: &["state", "call"],
    },
    HelpBlock {
        title: "pda",
        aliases: &["pda"],
        purpose: "List the PDAs declared by an instruction (Anchor seeds, derived addresses).",
        syntax: &[("pda <ix>", "")],
        flags: &[],
        examples: &[("pda stake", "See seeds + bump seeds for the stake instruction")],
        returns: "PdaList { instruction, pdas: [{ name, seeds, bump }] }.",
        see_also: &["info", "inspect", "call"],
    },
    HelpBlock {
        title: "inspect",
        aliases: &["inspect", "acc"],
        purpose: "Read a VM account by pubkey and decode it via the Anchor discriminator.",
        syntax: &[("inspect <pubkey>", "")],
        flags: &[],
        examples: &[
            ("inspect alice", "Decode alice's PDA / wallet"),
            ("inspect 6Yg7...", "Decode by raw base58 pubkey"),
        ],
        returns: "AccountView { pubkey, owner, lamports, data_decoded }.",
        see_also: &["state", "timeline", "vars"],
    },
    HelpBlock {
        title: "st | step",
        aliases: &["st", "step"],
        purpose: "Re-inspect a specific step of the active scenario: CU, logs, decoded diffs.",
        syntax: &[("st <index>", "")],
        flags: &[],
        examples: &[
            ("st 0", "Inspect the first step"),
            ("step 3", ""),
        ],
        returns: "StepDetail { index, instruction, logs, account_diffs, compute_units }.",
        see_also: &["session", "state", "timeline"],
    },
    HelpBlock {
        title: "who",
        aliases: &["who"],
        purpose: "Resolve a query against the IDL: account type, instruction, or struct field.",
        syntax: &[("who <AccountType|ix_name|field_name>", "")],
        flags: &[],
        examples: &[
            ("who Pool", "AccountType: list ix that touch it with args+fields"),
            ("who pool", "Same — case-insensitive snake_to_pascal fallback"),
            ("who claim_rewards", "Instruction: list accounts it touches with type+fields"),
            ("who total_staked", "Field: identify owner type, list ix that write it (heuristic)"),
        ],
        returns: "WhoList { query_kind: AccountType|Instruction|Field|NotFound, ... }.",
        see_also: &["info", "funcs", "vars", "coupling"],
    },
    HelpBlock {
        title: "tl | timeline",
        aliases: &["tl", "timeline"],
        purpose: "Show the cross-step mutation history of an account, decoded.",
        syntax: &[("tl <pubkey>", "Pubkey or named keypair")],
        flags: &[],
        examples: &[
            ("tl alice", "Trace alice across every step"),
            ("timeline pool", "Watch the pool PDA evolve"),
        ],
        returns: "Timeline { pubkey, entries: [{ step, decoded_fields, diff }] }.",
        see_also: &["state", "inspect", "step"],
    },
    HelpBlock {
        title: "coupling | cp",
        aliases: &["coupling", "cp"],
        purpose: "List instruction pairs that share a writable account (coupling heuristic from T-R50).",
        syntax: &[("coupling", "")],
        flags: &[],
        examples: &[("coupling", "Surface ix that may interfere via shared writable state")],
        returns: "CouplingList { pairs: [{ ix_a, ix_b, shared_writable: [..] }] }.",
        see_also: &["who", "funcs-all", "info"],
    },
    HelpBlock {
        title: "coverage | cov",
        aliases: &["coverage", "cov"],
        purpose: "Aggregated runtime metrics over the active scenario: calls, failures, CU stats, CPI edges (T-R52 RuntimeOverlay).",
        syntax: &[("coverage", "")],
        flags: &[],
        examples: &[("cov", "Spot ix never called, ix that always fail, programs reached via CPI")],
        returns: "Coverage { overlay: { program, scenario, calls_per_ix, failed_per_ix, cu_stats_per_ix, cpi_edges } }.",
        see_also: &["session", "state", "funcs"],
    },
    HelpBlock {
        title: "fi | finding",
        aliases: &["fi", "finding"],
        purpose: "Record a security finding tied to the latest step of the active scenario.",
        syntax: &[("fi <severity> <title>", "")],
        flags: &[
            ("--rec=\"...\"", "Optional remediation recommendation (quote it if it has spaces)"),
        ],
        examples: &[
            ("fi high reentrancy via stake", "Severity + free-form title"),
            ("finding critical missing signer --rec=\"require admin signature\"", ""),
        ],
        returns: "FindingRecorded { id, severity, title, step_index }.",
        see_also: &["findings", "note", "status", "export"],
    },
    HelpBlock {
        title: "fl | findings",
        aliases: &["fl", "findings"],
        purpose: "List every finding recorded in the active scenario.",
        syntax: &[("fl", "")],
        flags: &[],
        examples: &[("fl", "")],
        returns: "FindingsList { findings: [{ id, severity, title, step_index, recommendation }] }.",
        see_also: &["finding", "export", "session"],
    },
    HelpBlock {
        title: "n | note",
        aliases: &["n", "note"],
        purpose: "Attach a free-form annotation to the active scenario.",
        syntax: &[("n <text>", "")],
        flags: &[],
        examples: &[("n suspicious admin path here", "Drop a context note before moving on")],
        returns: "NoteAdded.",
        see_also: &["finding", "session"],
    },
    HelpBlock {
        title: "status",
        aliases: &["status"],
        purpose: "Set the review status of an instruction: open, reviewed, or finding.",
        syntax: &[("status <ix> <open|reviewed|finding>", "")],
        flags: &[],
        examples: &[
            ("status stake reviewed", "Mark stake as reviewed"),
            ("status claim_rewards finding", "Flag claim_rewards as having an issue"),
        ],
        returns: "StatusUpdated { ix, status }.",
        see_also: &["finding", "findings", "export"],
    },
    HelpBlock {
        title: "ex | export",
        aliases: &["ex", "export"],
        purpose: "Generate the audit deliverable (Markdown) with sequence, findings, and program info.",
        syntax: &[("ex", "")],
        flags: &[
            ("--auditor=<name>", "Auditor identity in the report metadata"),
            ("--version=<v>", "Project version pinned in the report"),
            ("--date=<YYYY-MM-DD>", "Audit date override (defaults to today)"),
        ],
        examples: &[
            ("ex", "Export with no metadata"),
            ("export --auditor=\"Alba S.\" --version=v1.2 --date=2026-05-09", ""),
        ],
        returns: "ExportArtifact { path, markdown }.",
        see_also: &["findings", "finding", "session"],
    },
    HelpBlock {
        title: "sc | scenario",
        aliases: &["sc", "scenario"],
        purpose: "Manage scenarios: create, list, switch, fork from a step, delete.",
        syntax: &[
            ("sc list", "List all scenarios in the workspace (default action)"),
            ("sc new <name>", "Create a fresh empty scenario"),
            ("sc switch <name>", "Activate an existing scenario"),
            ("sc fork <name> [step]", "Branch from the active scenario at an optional step index"),
            ("sc delete <name>", "Remove a scenario"),
        ],
        flags: &[],
        examples: &[
            ("sc new reentrancy", "Start a new scenario named reentrancy"),
            ("sc fork attack-v2 3", "Fork the active scenario at step 3"),
            ("sc switch main", ""),
        ],
        returns: "ScenarioList or ScenarioUpdated depending on the sub-command.",
        see_also: &["session", "save", "load"],
    },
    HelpBlock {
        title: "save",
        aliases: &["save"],
        purpose: "Serialise the active scenario to ~/.ilold/sessions/<name>.json.",
        syntax: &[("save <name>", "")],
        flags: &[
            ("--with-keypairs", "Bundle plaintext test keypairs (do NOT commit the file)"),
        ],
        examples: &[
            ("save reentrancy-attack", ""),
            ("save reentrancy-attack --with-keypairs", "Bundle keypairs for full reproducibility"),
        ],
        returns: "SessionSaved { json } — the CLI writes the file and warns if keypairs are bundled.",
        see_also: &["load", "scenario", "export"],
    },
    HelpBlock {
        title: "load",
        aliases: &["load"],
        purpose: "Restore a scenario JSON from disk and replay it into the VM.",
        syntax: &[("load <name>", "")],
        flags: &[],
        examples: &[("load reentrancy-attack", "Replay the saved scenario step-by-step")],
        returns: "SessionLoaded { steps } — VM is rebuilt deterministically.",
        see_also: &["save", "scenario", "session"],
    },
    HelpBlock {
        title: "seq | sequence",
        aliases: &["seq", "sequence"],
        purpose: "Render the narrative of the active scenario (Solana falls back to the session view).",
        syntax: &[("seq", "")],
        flags: &[],
        examples: &[("seq", "Read the step-by-step story so far")],
        returns: "SessionView (Solana parity with the Solidity sequence command).",
        see_also: &["session", "step", "state"],
    },
    HelpBlock {
        title: "browser",
        aliases: &["browser"],
        purpose: "Print the URL of the local web canvas (the explore REPL serves it next to the API).",
        syntax: &[("browser", "")],
        flags: &[],
        examples: &[("browser", "")],
        returns: "Plain text with the API base URL.",
        see_also: &["session", "state"],
    },
    HelpBlock {
        title: "? | help",
        aliases: &["?", "help", "h"],
        purpose: "Print the command menu. Append ? to any command for the full reference (e.g. call?, who?).",
        syntax: &[("?", "")],
        flags: &[],
        examples: &[
            ("?", "Top-level menu"),
            ("call?", "Full reference for call"),
            ("who?", "Full reference for who"),
        ],
        returns: "Formatted help text.",
        see_also: &[],
    },
    HelpBlock {
        title: "q | quit | exit",
        aliases: &["q", "quit", "exit"],
        purpose: "Exit the explore REPL.",
        syntax: &[("q", "")],
        flags: &[],
        examples: &[("q", "")],
        returns: "Terminates the session.",
        see_also: &[],
    },
];

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
        // Every command identifier accepted by handle_solana_input must have
        // either its own HelpBlock or be aliased into one. If you add a new
        // command branch in explore.rs, register it here too.
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
