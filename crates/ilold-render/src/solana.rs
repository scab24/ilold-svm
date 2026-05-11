use std::fmt::Write;

use ilold_solana_core::exploration::SolanaCommandResult;
use ilold_solana_core::exploration::commands::{WhoEntry, WhoIxAccount, WhoQueryKind};
use ilold_solana_core::overlay::RuntimeOverlay;
use ilold_solana_core::view::{
    AccountKind, AccountView, ArgView, CouplingPair, FieldView, IxAccountView, IxView,
    describe_seed_view,
};

use crate::colors::{c_accent, c_danger, c_muted, c_ok, c_warn};
use crate::fmt::pad_right;

/// Render a SolanaCommandResult to text. Output is byte-identical to the
/// legacy `print_solana_result` body (without the leading/trailing blank
/// line); callers add framing whitespace where appropriate.
pub fn render_solana_result(result: &SolanaCommandResult) -> String {
    let mut out = String::new();
    match result {
        SolanaCommandResult::StepAdded {
            step_index,
            instruction,
            logs_excerpt,
            account_diffs_count,
            compute_units,
            error,
        } => {
            let failed = error.is_some()
                || logs_excerpt.iter().any(|l| {
                    let s = l.as_str();
                    s.contains("AnchorError") || s.contains("failed:") || s.contains("panicked")
                });
            let (mark, label) = if failed {
                (c_danger("✗").to_string(), c_danger("FAILED").to_string())
            } else {
                (c_ok("✓").to_string(), c_ok("ok").to_string())
            };
            writeln!(
                out,
                "  {} step {} [{}]: {} {}",
                mark,
                step_index,
                label,
                c_accent(instruction),
                c_muted(&format!(
                    "({} CU, {} diffs)",
                    compute_units, account_diffs_count
                ))
            )
            .ok();
            for log in logs_excerpt {
                if log.contains("AnchorError")
                    || log.contains("failed:")
                    || log.contains("panicked")
                {
                    writeln!(out, "    {}", c_danger(log)).ok();
                } else {
                    writeln!(out, "    {}", c_muted(log)).ok();
                }
            }
        }
        SolanaCommandResult::CallFailed {
            instruction,
            logs_excerpt,
            compute_units,
            error,
        } => {
            writeln!(
                out,
                "  {} {}: {} {}",
                c_danger("✗"),
                c_danger("FAILED"),
                c_accent(instruction),
                c_muted(&format!("({} CU, not recorded)", compute_units)),
            )
            .ok();
            writeln!(out, "    {} {}", c_danger("error:"), error).ok();
            for log in logs_excerpt {
                if log.contains("AnchorError")
                    || log.contains("failed:")
                    || log.contains("panicked")
                {
                    writeln!(out, "    {}", c_danger(log)).ok();
                } else {
                    writeln!(out, "    {}", c_muted(log)).ok();
                }
            }
        }
        SolanaCommandResult::StepRemoved { remaining } => {
            writeln!(out, "  {} step undone ({} remaining)", c_ok("✓"), remaining).ok();
        }
        SolanaCommandResult::Cleared => {
            writeln!(out, "  {} session cleared", c_ok("✓")).ok();
        }
        SolanaCommandResult::InstructionList { items } => {
            for ix in items {
                let badge = if ix.has_pdas { c_accent("[PDA]") } else { c_muted("[ix]") };
                let signers = if ix.signers.is_empty() {
                    String::new()
                } else {
                    format!(" signers: {}", ix.signers.join(","))
                };
                writeln!(
                    out,
                    "  {} {} {}{}",
                    badge,
                    ix.name,
                    c_muted(&format!(
                        "(args:{} accounts:{})",
                        ix.args_count, ix.accounts_count
                    )),
                    c_muted(&signers)
                )
                .ok();
            }
        }
        SolanaCommandResult::StateView { accounts } => {
            if accounts.is_empty() {
                writeln!(out, "  {}", c_muted("No accounts mutated yet")).ok();
            }
            for a in accounts {
                writeln!(
                    out,
                    "  {} {} {} {}",
                    c_accent("[A]"),
                    c_accent(&a.label),
                    c_muted(&format!("({} lamports)", a.lamports)),
                    c_muted(&a.pubkey),
                )
                .ok();
                match a.decoded.as_ref() {
                    None => {
                        writeln!(out, "      {}", c_muted("<not decoded>")).ok();
                    }
                    Some(serde_json::Value::Object(map)) => {
                        let max = map
                            .keys()
                            .map(|k| k.chars().count())
                            .max()
                            .unwrap_or(0);
                        for (k, v) in map {
                            let val = match v {
                                serde_json::Value::String(s) => s.clone(),
                                _ => serde_json::to_string(v).unwrap_or_default(),
                            };
                            writeln!(
                                out,
                                "      {} {}",
                                c_muted(&format!("{:width$}", k, width = max)),
                                val,
                            )
                            .ok();
                        }
                    }
                    Some(other) => {
                        writeln!(
                            out,
                            "      {}",
                            c_muted(&serde_json::to_string(other).unwrap_or_default()),
                        )
                        .ok();
                    }
                }
            }
        }
        SolanaCommandResult::SessionView {
            program,
            scenario,
            steps,
            findings_count,
        } => {
            writeln!(
                out,
                "  program={} scenario={} steps={} findings={}",
                c_accent(program),
                c_accent(scenario),
                steps.len(),
                findings_count
            )
            .ok();
            for (i, s) in steps.iter().enumerate() {
                writeln!(out, "    {} {}", c_muted(&format!("{i}.")), s).ok();
            }
        }
        SolanaCommandResult::UserList { users } => {
            if users.is_empty() {
                writeln!(
                    out,
                    "  {}",
                    c_muted("No users — create with 'users new <name>'")
                )
                .ok();
            }
            for u in users {
                writeln!(
                    out,
                    "  {} {} {} {}",
                    c_accent("[U]"),
                    u.name,
                    c_muted(&u.pubkey),
                    c_muted(&format!("{} lamports", u.lamports))
                )
                .ok();
            }
        }
        SolanaCommandResult::UserCreated { name, pubkey, lamports } => {
            writeln!(
                out,
                "  {} user {} created at {} with {} lamports",
                c_ok("✓"),
                c_accent(name),
                c_muted(pubkey),
                lamports
            )
            .ok();
        }
        SolanaCommandResult::Airdropped { name, pubkey, total_lamports } => {
            writeln!(
                out,
                "  {} {} now {} lamports {}",
                c_ok("✓"),
                c_accent(name),
                total_lamports,
                c_muted(pubkey)
            )
            .ok();
        }
        SolanaCommandResult::TimeWarped { unix_timestamp, slot } => {
            writeln!(
                out,
                "  {} clock now ts={} slot={}",
                c_ok("✓"),
                unix_timestamp,
                slot
            )
            .ok();
        }
        SolanaCommandResult::PdaList { instruction, pdas } => {
            if pdas.is_empty() {
                writeln!(out, "  {} {}", c_muted("no PDAs declared in"), instruction).ok();
            }
            for p in pdas {
                writeln!(
                    out,
                    "  {} {} seeds=[{}] program={}",
                    c_accent("[PDA]"),
                    p.account_name,
                    p.seeds.join(", "),
                    c_muted(&p.program)
                )
                .ok();
            }
        }
        SolanaCommandResult::AccountInspected {
            pubkey,
            owner,
            lamports,
            data_len,
            decoded,
        } => {
            writeln!(
                out,
                "  {} owner={} lamports={} data_len={}",
                c_accent(pubkey),
                c_muted(owner),
                lamports,
                data_len
            )
            .ok();
            if let Some(d) = decoded {
                writeln!(
                    out,
                    "    {}",
                    serde_json::to_string_pretty(d).unwrap_or_default()
                )
                .ok();
            }
        }
        SolanaCommandResult::FindingAdded { id } => {
            writeln!(out, "  {} finding {}", c_ok("✓"), c_accent(id)).ok();
        }
        SolanaCommandResult::NoteAdded => {
            writeln!(out, "  {} note recorded", c_ok("✓")).ok();
        }
        SolanaCommandResult::StatusUpdated => {
            writeln!(out, "  {} status updated", c_ok("✓")).ok();
        }
        SolanaCommandResult::SessionSaved { json } => {
            writeln!(
                out,
                "  {} session JSON ({} bytes)",
                c_ok("✓"),
                json.len()
            )
            .ok();
        }
        SolanaCommandResult::SessionLoaded { program, steps } => {
            writeln!(
                out,
                "  {} loaded program={} steps={}",
                c_ok("✓"),
                c_accent(program),
                steps.len()
            )
            .ok();
        }
        SolanaCommandResult::ScenarioList { items } => {
            for it in items {
                let marker = if it.active {
                    c_ok(" ← active").to_string()
                } else {
                    String::new()
                };
                writeln!(
                    out,
                    "  {} {} {}{}",
                    c_accent("[S]"),
                    it.name,
                    c_muted(&format!("({} steps)", it.step_count)),
                    marker
                )
                .ok();
            }
        }
        SolanaCommandResult::ScenarioCreated { name } => {
            writeln!(out, "  {} scenario {} created", c_ok("✓"), c_accent(name)).ok();
        }
        SolanaCommandResult::ScenarioSwitched { from, to } => {
            writeln!(out, "  {} {} → {}", c_ok("→"), from, c_accent(to)).ok();
        }
        SolanaCommandResult::ScenarioForked { from, to, at_step } => {
            writeln!(
                out,
                "  {} forked {} → {} at step {}",
                c_ok("✓"),
                from,
                c_accent(to),
                at_step
            )
            .ok();
        }
        SolanaCommandResult::ScenarioDeleted { name } => {
            writeln!(out, "  {} scenario {} deleted", c_ok("✓"), name).ok();
        }
        SolanaCommandResult::StepDetail {
            step_index,
            instruction,
            runtime_trace,
            diff_summary,
        } => {
            writeln!(
                out,
                "  {} step {} · {}",
                c_accent("·"),
                step_index,
                c_accent(instruction)
            )
            .ok();
            if let Some(trace) = runtime_trace {
                if let Some(cu) = trace.get("compute_units").and_then(|v| v.as_u64()) {
                    writeln!(out, "    {} {} CU", c_muted("compute units:"), cu).ok();
                }
                if let Some(err) = trace.get("error").and_then(|v| v.as_str()) {
                    writeln!(out, "    {} {}", c_danger("error:"), err).ok();
                }
                if let Some(logs) = trace.get("logs").and_then(|v| v.as_array()) {
                    writeln!(out, "    {} ({} lines)", c_muted("logs:"), logs.len()).ok();
                    for line in logs.iter().take(20) {
                        if let Some(s) = line.as_str() {
                            writeln!(out, "      {}", c_muted(s)).ok();
                        }
                    }
                    if logs.len() > 20 {
                        writeln!(
                            out,
                            "      {}",
                            c_muted(&format!("... +{} more", logs.len() - 20))
                        )
                        .ok();
                    }
                }
            }
            if !diff_summary.is_empty() {
                writeln!(
                    out,
                    "    {} ({})",
                    c_muted("account diffs:"),
                    diff_summary.len()
                )
                .ok();
                for d in diff_summary {
                    let label = d.name.clone().unwrap_or_else(|| d.address.clone());
                    let lam = if d.lamports_delta != 0 {
                        format!(" Δlamports={}", d.lamports_delta)
                    } else {
                        String::new()
                    };
                    writeln!(
                        out,
                        "      {} {}{}",
                        c_accent("·"),
                        c_accent(&label),
                        c_muted(&lam),
                    )
                    .ok();
                    match (d.decoded_before.as_ref(), d.decoded_after.as_ref()) {
                        (None, None) => {
                            if d.data_changed {
                                writeln!(out, "        {}", c_muted("data changed (not decoded)"))
                                    .ok();
                            }
                        }
                        (None, Some(after)) => {
                            writeln!(
                                out,
                                "        {}",
                                c_muted("(new account, decoded fields:)")
                            )
                            .ok();
                            write_decoded_fields(&mut out, after, "          ");
                        }
                        (Some(_), None) => {
                            writeln!(out, "        {}", c_muted("(account closed)")).ok();
                        }
                        (Some(before), Some(after)) => {
                            write_decoded_diff(&mut out, before, after, "        ");
                        }
                    }
                }
            }
        }
        SolanaCommandResult::FindingsList { items } => {
            if items.is_empty() {
                writeln!(out, "  {}", c_muted("no findings recorded")).ok();
            } else {
                for f in items {
                    writeln!(
                        out,
                        "  {} {} [{}] {}",
                        c_accent(&f.id),
                        c_warn(&f.severity),
                        c_muted(&f.created_at),
                        c_accent(&f.title)
                    )
                    .ok();
                    if !f.description.is_empty() {
                        writeln!(out, "    {}", c_muted(&f.description)).ok();
                    }
                }
            }
        }
        SolanaCommandResult::Exported { markdown, bytes } => {
            writeln!(out, "  {} markdown report ({} bytes)", c_ok("✓"), bytes).ok();
            writeln!(out).ok();
            for line in markdown.lines() {
                writeln!(out, "  {}", line).ok();
            }
        }
        SolanaCommandResult::WhoList {
            account_type,
            instructions,
            query_kind,
            field_owner,
            field_type,
            owner_fields,
            ix_args,
            ix_discriminator_hex,
            ix_accounts,
        } => {
            render_who_list(
                &mut out,
                account_type,
                instructions,
                *query_kind,
                field_owner.as_deref(),
                field_type.as_deref(),
                owner_fields.as_deref(),
                ix_args.as_deref(),
                ix_discriminator_hex.as_deref(),
                ix_accounts.as_deref(),
            );
        }
        SolanaCommandResult::TimelineView { pubkey, label, entries } => {
            let header = label.clone().unwrap_or_else(|| pubkey.clone());
            writeln!(
                out,
                "  {} timeline for {} ({})",
                c_accent("·"),
                c_accent(&header),
                c_muted(pubkey)
            )
            .ok();
            if entries.is_empty() {
                writeln!(out, "    {}", c_muted("no mutations recorded for this pubkey")).ok();
            } else {
                for e in entries {
                    let lam = if e.lamports_delta != 0 {
                        format!(" Δlamports={}", e.lamports_delta)
                    } else {
                        String::new()
                    };
                    let dat = if e.data_changed { " data" } else { "" };
                    writeln!(
                        out,
                        "    {} #{} {} ({}){}{}",
                        c_accent("·"),
                        e.step_index,
                        c_accent(&e.instruction),
                        e.scenario,
                        c_muted(&lam),
                        c_muted(dat)
                    )
                    .ok();
                    if let Some(after) = &e.after_decoded {
                        let s = serde_json::to_string(after).unwrap_or_default();
                        let preview = if s.len() > 200 {
                            format!("{}…", &s[..200])
                        } else {
                            s
                        };
                        writeln!(out, "        {}", c_muted(&preview)).ok();
                    }
                }
            }
        }
        SolanaCommandResult::IxInfo { ix, admin_gated } => {
            render_ix_info(&mut out, ix, *admin_gated);
        }
        SolanaCommandResult::CouplingList { pairs } => {
            render_coupling_list(&mut out, pairs);
        }
        SolanaCommandResult::AccountTypes { accounts } => {
            render_account_types(&mut out, accounts);
        }
        SolanaCommandResult::Coverage { overlay } => {
            render_coverage_overlay(&mut out, overlay);
        }
        SolanaCommandResult::Error { message } => {
            writeln!(out, "  {} {}", c_danger("✗"), message).ok();
        }
    }
    out
}

fn write_decoded_fields(out: &mut String, value: &serde_json::Value, indent: &str) {
    if let serde_json::Value::Object(map) = value {
        let max = map.keys().map(|k| k.chars().count()).max().unwrap_or(0);
        for (k, v) in map {
            let val = match v {
                serde_json::Value::String(s) => s.clone(),
                _ => serde_json::to_string(v).unwrap_or_default(),
            };
            writeln!(
                out,
                "{}{} {}",
                indent,
                c_muted(&format!("{:width$}", k, width = max)),
                val,
            )
            .ok();
        }
    }
}

fn write_decoded_diff(
    out: &mut String,
    before: &serde_json::Value,
    after: &serde_json::Value,
    indent: &str,
) {
    let (a, b) = match (before, after) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => (a, b),
        _ => {
            writeln!(
                out,
                "{}{}",
                indent,
                c_muted("decoded shape changed (not an object diff)"),
            )
            .ok();
            return;
        }
    };
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    let max = keys.iter().map(|k| k.chars().count()).max().unwrap_or(0);
    let mut any = false;
    for k in keys {
        let lhs = a.get(k);
        let rhs = b.get(k);
        if lhs == rhs {
            continue;
        }
        any = true;
        let s_lhs = lhs
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                _ => serde_json::to_string(v).unwrap_or_default(),
            })
            .unwrap_or_else(|| "<absent>".into());
        let s_rhs = rhs
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                _ => serde_json::to_string(v).unwrap_or_default(),
            })
            .unwrap_or_else(|| "<absent>".into());
        writeln!(
            out,
            "{}{}  {} → {}",
            indent,
            c_muted(&format!("{:width$}", k, width = max)),
            c_muted(&s_lhs),
            s_rhs,
        )
        .ok();
    }
    if !any {
        writeln!(out, "{}{}", indent, c_muted("(no decoded field changed)")).ok();
    }
}

fn render_ix_info(out: &mut String, ix: &IxView, admin_gated: bool) {
    writeln!(out).ok();
    writeln!(out, "  {} {}", c_accent("instruction"), ix.name).ok();
    if !ix.discriminator_hex.is_empty() {
        writeln!(out, "  {} {}", c_muted("discriminator"), ix.discriminator_hex).ok();
    }
    writeln!(out).ok();
    writeln!(out, "  {} ({})", c_accent("args"), ix.args.len()).ok();
    if ix.args.is_empty() {
        writeln!(out, "    {}", c_muted("(none)")).ok();
    } else {
        let max = ix
            .args
            .iter()
            .map(|a| a.name.chars().count())
            .max()
            .unwrap_or(0);
        for a in &ix.args {
            writeln!(
                out,
                "    {} {} {}",
                c_accent("·"),
                pad_right(&a.name, max),
                c_muted(&a.ty)
            )
            .ok();
        }
    }
    writeln!(out).ok();
    writeln!(out, "  {} ({})", c_accent("accounts"), ix.accounts.len()).ok();
    if ix.accounts.is_empty() {
        writeln!(out, "    {}", c_muted("(none)")).ok();
    } else {
        let max = ix
            .accounts
            .iter()
            .map(|a| a.name.chars().count())
            .max()
            .unwrap_or(0);
        for a in &ix.accounts {
            let mut flags: Vec<&str> = Vec::new();
            if a.signer {
                flags.push("signer");
            }
            if a.writable {
                flags.push("writable");
            }
            if a.optional {
                flags.push("optional");
            }
            let kind_label = match a.kind {
                AccountKind::Program => "program",
                AccountKind::System => "system",
                AccountKind::Sysvar => "sysvar",
                AccountKind::Pda => "pda",
                AccountKind::Other => "other",
            };
            let suffix = if let Some(addr) = a.address.as_deref() {
                format!("{kind_label}  const {addr}")
            } else if flags.is_empty() {
                kind_label.to_string()
            } else {
                format!("{kind_label}  {}", flags.join(" "))
            };
            writeln!(
                out,
                "    {} {} {}",
                c_accent("·"),
                pad_right(&a.name, max),
                c_muted(&suffix)
            )
            .ok();
        }
    }
    let pdas: Vec<&IxAccountView> = ix.accounts.iter().filter(|a| a.pda.is_some()).collect();
    if !pdas.is_empty() {
        writeln!(out).ok();
        writeln!(out, "  {} ({})", c_accent("pdas"), pdas.len()).ok();
        for acc in pdas {
            let pda = acc.pda.as_ref().expect("filtered above");
            let seeds: Vec<String> = pda.seeds.iter().map(describe_seed_view).collect();
            let prog = pda.program.clone().unwrap_or_else(|| "self".to_string());
            writeln!(
                out,
                "    {} {} seeds=[{}] program={}",
                c_accent("·"),
                acc.name,
                seeds.join(", "),
                c_muted(&prog)
            )
            .ok();
        }
    }
    writeln!(out).ok();
    let gated_label = if admin_gated {
        c_warn("true (heuristic)").to_string()
    } else {
        c_muted("false").to_string()
    };
    writeln!(out, "  {} {}", c_muted("admin_gated"), gated_label).ok();
}

fn render_coupling_list(out: &mut String, pairs: &[CouplingPair]) {
    if pairs.is_empty() {
        writeln!(
            out,
            "  {}",
            c_muted("no instruction pairs share writable accounts")
        )
        .ok();
        return;
    }
    let max = pairs
        .iter()
        .map(|p| p.a.chars().count() + p.b.chars().count() + 5)
        .max()
        .unwrap_or(0);
    for p in pairs {
        let pair = format!("{}  ↔  {}", p.a, p.b);
        writeln!(
            out,
            "  {} {} {}",
            c_accent("·"),
            pad_right(&pair, max),
            c_muted(&format!("[{}]", p.shared_writable.join(", ")))
        )
        .ok();
    }
}

fn render_account_types(out: &mut String, accounts: &[AccountView]) {
    if accounts.is_empty() {
        writeln!(out, "  {}", c_muted("No account types declared in IDL")).ok();
        return;
    }
    for at in accounts {
        writeln!(
            out,
            "  {} {} {}",
            c_accent("[T]"),
            at.name,
            c_muted(&at.discriminator_hex)
        )
        .ok();
        if at.fields.is_empty() {
            writeln!(out, "    {}", c_muted("(opaque or zero-copy layout)")).ok();
            continue;
        }
        let max = at
            .fields
            .iter()
            .map(|f| f.name.chars().count())
            .max()
            .unwrap_or(0);
        for f in &at.fields {
            writeln!(
                out,
                "    {} {} {}",
                c_accent("·"),
                pad_right(&f.name, max),
                c_muted(&f.ty)
            )
            .ok();
        }
    }
}

fn render_coverage_overlay(out: &mut String, overlay: &RuntimeOverlay) {
    writeln!(out).ok();
    writeln!(
        out,
        "  {} {} {}",
        c_accent("Coverage for program"),
        overlay.program,
        c_muted(&format!("(scenario {})", overlay.scenario)),
    )
    .ok();
    writeln!(out).ok();

    let mut ix_keys: std::collections::BTreeSet<&String> = std::collections::BTreeSet::new();
    ix_keys.extend(overlay.calls_per_ix.keys());
    ix_keys.extend(overlay.failed_per_ix.keys());
    ix_keys.extend(overlay.cu_stats_per_ix.keys());

    if ix_keys.is_empty() {
        writeln!(out, "  {}", c_muted("no calls recorded yet")).ok();
        return;
    }

    let cpi_count_per_ix = |ix: &str| -> u32 {
        overlay
            .cpi_edges
            .iter()
            .filter(|e| e.from_ix == ix)
            .map(|e| e.samples)
            .sum()
    };

    let max_ix = ix_keys
        .iter()
        .map(|k| k.chars().count())
        .max()
        .unwrap_or(0)
        .max(11);
    writeln!(
        out,
        "  {} {} {} {} {} {}",
        pad_right("Instruction", max_ix),
        c_muted("Calls"),
        c_muted("Failed"),
        c_muted("CU avg"),
        c_muted("CU max"),
        c_muted("CPIs"),
    )
    .ok();
    let mut total_calls: u32 = 0;
    let mut total_failed: u32 = 0;
    for ix in &ix_keys {
        let calls = overlay.calls_per_ix.get(*ix).copied().unwrap_or(0);
        let failed = overlay.failed_per_ix.get(*ix).copied().unwrap_or(0);
        total_calls += calls;
        total_failed += failed;
        let (cu_avg, cu_max) = match overlay.cu_stats_per_ix.get(*ix) {
            Some(s) => (s.avg.to_string(), s.max.to_string()),
            None => ("—".to_string(), "—".to_string()),
        };
        let cpi = cpi_count_per_ix(ix);
        writeln!(
            out,
            "  {} {} {} {} {} {}",
            pad_right(ix, max_ix),
            pad_right(&calls.to_string(), 5),
            pad_right(&failed.to_string(), 6),
            pad_right(&cu_avg, 6),
            pad_right(&cu_max, 6),
            cpi,
        )
        .ok();
    }
    if !overlay.cpi_edges.is_empty() {
        writeln!(out).ok();
        writeln!(out, "  {}", c_accent("CPIs detected:")).ok();
        for edge in &overlay.cpi_edges {
            writeln!(
                out,
                "    {} {} {} {} {}",
                c_accent("·"),
                edge.from_ix,
                c_muted("→"),
                edge.to_program,
                c_muted(&format!("(depth {}, {}×)", edge.depth, edge.samples)),
            )
            .ok();
        }
    }
    writeln!(out).ok();
    writeln!(
        out,
        "  {} {} calls, {} failed",
        c_muted("Total:"),
        total_calls,
        total_failed,
    )
    .ok();
}

#[allow(clippy::too_many_arguments)]
fn render_who_list(
    out: &mut String,
    target: &str,
    instructions: &[WhoEntry],
    query_kind: WhoQueryKind,
    field_owner: Option<&str>,
    field_type: Option<&str>,
    owner_fields: Option<&[FieldView]>,
    ix_args: Option<&[ArgView]>,
    ix_discriminator_hex: Option<&str>,
    ix_accounts: Option<&[WhoIxAccount]>,
) {
    match query_kind {
        WhoQueryKind::AccountType => {
            writeln!(
                out,
                "  {} '{}' (account type)",
                c_accent("·"),
                c_accent(target)
            )
            .ok();
            render_field_summary(out, owner_fields, "fields");
            writeln!(out).ok();
            if instructions.is_empty() {
                writeln!(out, "  {}", c_muted("not referenced by any instruction")).ok();
                return;
            }
            writeln!(
                out,
                "  Referenced by {} instruction{}:",
                instructions.len(),
                if instructions.len() == 1 { "" } else { "s" }
            )
            .ok();
            writeln!(out).ok();
            for w in instructions {
                render_who_entry_block(out, w);
            }
        }
        WhoQueryKind::Field => {
            let owner = field_owner.unwrap_or("?");
            let ty = field_type.unwrap_or("?");
            writeln!(
                out,
                "  {} '{}' (field of {}, type {})",
                c_accent("·"),
                c_accent(target),
                c_accent(owner),
                c_muted(ty)
            )
            .ok();
            render_field_summary(out, owner_fields, &format!("{owner} struct"));
            writeln!(out).ok();
            writeln!(
                out,
                "  {}",
                c_warn("Heuristic: the following instructions write the owner account.")
            )
            .ok();
            writeln!(
                out,
                "  {}",
                c_muted("Without source-level analysis we cannot tell which one(s)")
            )
            .ok();
            writeln!(
                out,
                "  {}",
                c_muted("actually mutate this field; cross-check with `step <idx>`.")
            )
            .ok();
            writeln!(out).ok();
            if instructions.is_empty() {
                writeln!(out, "    {}", c_muted("(no writers found)")).ok();
                return;
            }
            for w in instructions {
                render_who_entry_block(out, w);
            }
        }
        WhoQueryKind::Instruction => {
            writeln!(
                out,
                "  {} '{}' (instruction)",
                c_accent("·"),
                c_accent(target)
            )
            .ok();
            match ix_args {
                Some(args) if !args.is_empty() => {
                    let parts: Vec<String> = args
                        .iter()
                        .map(|a| format!("{}: {}", a.name, a.ty))
                        .collect();
                    writeln!(out, "    args: {}", c_muted(&parts.join(", "))).ok();
                }
                _ => {
                    writeln!(out, "    {}", c_muted("args: (none)")).ok();
                }
            }
            if let Some(d) = ix_discriminator_hex {
                writeln!(out, "    {} {}", c_muted("discriminator"), d).ok();
            }
            writeln!(out).ok();
            let accounts = ix_accounts.unwrap_or(&[]);
            if accounts.is_empty() {
                writeln!(out, "  {}", c_muted("touches no accounts")).ok();
                return;
            }
            writeln!(
                out,
                "  Touches {} account{}:",
                accounts.len(),
                if accounts.len() == 1 { "" } else { "s" }
            )
            .ok();
            writeln!(out).ok();
            for acc in accounts {
                render_who_ix_account(out, acc);
            }
        }
        WhoQueryKind::NotFound => {
            writeln!(
                out,
                "  {} no instruction, account type or field references '{}'",
                c_muted("·"),
                target
            )
            .ok();
        }
    }
}

fn render_field_summary(out: &mut String, fields: Option<&[FieldView]>, label: &str) {
    match fields {
        Some(fs) if !fs.is_empty() => {
            let parts: Vec<String> = fs.iter().map(|f| format!("{}: {}", f.name, f.ty)).collect();
            writeln!(out, "    {}: {}", label, c_muted(&parts.join(", "))).ok();
        }
        Some(_) => {
            writeln!(
                out,
                "    {}: {}",
                label,
                c_muted("(opaque or zero-copy layout)")
            )
            .ok();
        }
        None => {}
    }
}

fn render_who_entry_block(out: &mut String, entry: &WhoEntry) {
    let mut flags = Vec::new();
    if entry.signer {
        flags.push("signer");
    }
    if entry.writable {
        flags.push("writable");
    }
    let flags_str = flags.join(" ");
    writeln!(
        out,
        "    {} {} (as {}) {}",
        c_accent("·"),
        c_accent(&entry.instruction),
        entry.account_field,
        c_muted(&flags_str)
    )
    .ok();
    match entry.ix_args.as_ref() {
        Some(args) if !args.is_empty() => {
            let parts: Vec<String> = args.iter().map(|a| format!("{}: {}", a.name, a.ty)).collect();
            writeln!(out, "        args: {}", c_muted(&parts.join(", "))).ok();
        }
        Some(_) => {
            writeln!(out, "        {}", c_muted("args: (none)")).ok();
        }
        None => {}
    }
}

fn render_who_ix_account(out: &mut String, acc: &WhoIxAccount) {
    let type_label = acc
        .account_type
        .as_deref()
        .map(|t| format!("({t})"))
        .unwrap_or_else(|| "(—)".to_string());
    let mut flags = Vec::new();
    if acc.signer {
        flags.push("signer");
    }
    if acc.writable {
        flags.push("writable");
    }
    writeln!(
        out,
        "    {} {} {} {}",
        c_accent("·"),
        c_accent(&acc.name),
        c_muted(&type_label),
        c_muted(&flags.join(" "))
    )
    .ok();
    if let Some(fs) = acc.fields.as_ref()
        && !fs.is_empty()
    {
        let parts: Vec<String> = fs.iter().map(|f| format!("{}: {}", f.name, f.ty)).collect();
        writeln!(out, "        struct: {}", c_muted(&parts.join(", "))).ok();
    }
}
