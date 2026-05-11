use std::fmt::Write;

use serde::{Deserialize, Serialize};

use super::types::*;

/// Optional auditor metadata threaded through the export. Pass-through, not
/// stored in the journal — see SDD 02-audit-deliverable-export/design.md
/// rationale (avoids JSON-format bumps and keeps PII out of saved sessions).
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AuditMetadata {
    #[serde(default)]
    pub auditor: Option<String>,
    #[serde(default)]
    pub project_version: Option<String>,
    #[serde(default)]
    pub audit_date: Option<String>,
}

/// Solana-specific program facts the renderer prints next to the metadata.
/// Solidity callers pass `None` to `export_markdown_multi`; the body simply
/// omits the program block.
#[derive(Debug, Clone)]
pub struct ProgramSection {
    pub name: String,
    pub program_id: String,
    pub instructions: usize,
    pub account_types: usize,
}

pub fn export_markdown(journal: &AuditJournal, total_functions: usize) -> String {
    let mut md = String::new();
    let (done, total) = journal.progress(total_functions);
    let pct = if total > 0 { done * 100 / total } else { 0 };

    writeln!(md, "# Audit: {}", journal.contract).unwrap();
    writeln!(md, "**Project**: {} | **Started**: {} | **Progress**: {}/{} functions ({}%)",
        journal.project, journal.started_at, done, total, pct).unwrap();

    writeln!(md).unwrap();
    render_findings_block(&mut md, &journal.findings);

    writeln!(md).unwrap();
    render_coverage(&mut md, journal);

    if journal.entries.iter().any(|e| matches!(e,
        JournalEntry::SequenceExplored { .. } | JournalEntry::BranchCreated { .. }
    )) {
        writeln!(md).unwrap();
        render_exploration_log(&mut md, journal);
    }

    md
}

// ── Shared private helpers ──────────────────────────────────────────────────

pub(crate) fn render_findings_block(md: &mut String, findings: &[Finding]) {
    if findings.is_empty() {
        writeln!(md, "## Findings\n\nNo findings recorded.").unwrap();
        return;
    }
    writeln!(md, "## Findings\n").unwrap();
    writeln!(md, "| ID | Severity | Title | Function |").unwrap();
    writeln!(md, "|----|----------|-------|----------|").unwrap();
    for f in findings {
        writeln!(md, "| {} | {} | {} | {} |", f.id, f.severity, f.title, f.affected_function).unwrap();
    }
    for f in findings {
        writeln!(md).unwrap();
        render_finding_detail(md, f, None);
    }
}

pub(crate) fn render_finding_detail(md: &mut String, f: &Finding, scenario: Option<&str>) {
    writeln!(md, "### {}: {}\n", f.id, f.title).unwrap();
    let mut header = format!(
        "**Severity**: {} | **Function**: {}",
        f.severity, f.affected_function,
    );
    if let Some(idx) = f.affected_step_index {
        header.push_str(&format!(" | **Step**: #{idx}"));
    }
    if let Some(s) = scenario {
        header.push_str(&format!(" | **Scenario**: `{s}`"));
    }
    writeln!(md, "{header}").unwrap();
    if let Some(seq) = &f.affected_sequence {
        writeln!(md, "**Sequence**: {}", seq.join(" → ")).unwrap();
    }
    writeln!(md, "\n{}", f.description).unwrap();
    if let Some(rec) = &f.recommendation {
        writeln!(md, "\n**Recommendation**\n\n{rec}").unwrap();
    }
    for note in &f.notes {
        writeln!(md, "\n> {note}").unwrap();
    }
}

pub(crate) fn render_coverage(md: &mut String, journal: &AuditJournal) {
    writeln!(md, "## Coverage\n").unwrap();
    if journal.function_status.is_empty() {
        writeln!(md, "No functions reviewed yet.").unwrap();
        return;
    }
    let mut funcs: Vec<_> = journal.function_status.iter().collect();
    funcs.sort_by_key(|(name, _)| (*name).clone());
    for (name, status) in funcs {
        writeln!(md, "- {} {}", status.badge(), name).unwrap();
    }
}

pub(crate) fn render_exploration_log(md: &mut String, journal: &AuditJournal) {
    writeln!(md, "## Exploration Log\n").unwrap();
    for entry in &journal.entries {
        match entry {
            JournalEntry::SequenceExplored { steps, timestamp, .. } => {
                writeln!(md, "- **{}** Explored: {}",
                    &timestamp[..std::cmp::min(16, timestamp.len())], steps.join(" → ")).unwrap();
            }
            JournalEntry::BranchCreated { from_function, branch_function, timestamp } => {
                writeln!(md, "- **{}** Branch: {} → {}",
                    &timestamp[..std::cmp::min(16, timestamp.len())], from_function, branch_function).unwrap();
            }
            _ => {}
        }
    }
}

pub(crate) fn render_metadata_header(md: &mut String, m: &AuditMetadata) {
    let mut parts: Vec<String> = Vec::new();
    if let Some(a) = &m.auditor { parts.push(format!("**Auditor**: {a}")); }
    if let Some(v) = &m.project_version { parts.push(format!("**Version**: {v}")); }
    if let Some(d) = &m.audit_date { parts.push(format!("**Date**: {d}")); }
    if !parts.is_empty() {
        writeln!(md, "{}\n", parts.join(" | ")).unwrap();
    }
}

pub(crate) fn render_severity_matrix(md: &mut String, journals: &[&AuditJournal]) {
    let mut counts = [0usize; 5]; // Critical, High, Medium, Low, Informational
    for j in journals {
        for f in &j.findings {
            let i = match f.severity {
                Severity::Critical => 0,
                Severity::High => 1,
                Severity::Medium => 2,
                Severity::Low => 3,
                Severity::Informational => 4,
            };
            counts[i] += 1;
        }
    }
    writeln!(md, "## Severity Matrix\n").unwrap();
    writeln!(md, "| Severity | Count |").unwrap();
    writeln!(md, "|----------|-------|").unwrap();
    let labels = ["Critical", "High", "Medium", "Low", "Informational"];
    for (i, label) in labels.iter().enumerate() {
        writeln!(md, "| {label} | {} |", counts[i]).unwrap();
    }
    let total: usize = counts.iter().sum();
    writeln!(md, "| **Total** | **{total}** |").unwrap();
}

pub(crate) fn render_methodology(md: &mut String, program_name: &str) {
    writeln!(md, "## Methodology\n").unwrap();
    writeln!(md, "This deliverable was produced with Ilold, an interactive REPL for \
auditing smart contracts. The Solana backend executes the program (`{program_name}`) \
inside LiteSVM with the real BPF binary, so every step in the timeline corresponds to \
an actual transaction; compute-unit usage, account diffs and program logs are recorded \
verbatim. Findings are scenario-anchored — each entry references the scenario in which \
the auditor observed the issue, and every scenario carries an independent VM with its \
own state snapshot stack so attacks explored in a branch never leak into the main \
session.\n").unwrap();
}

pub fn export_markdown_multi(
    scenarios: &[(&str, &AuditJournal)],
    program: Option<&ProgramSection>,
    metadata: Option<&AuditMetadata>,
    instructions_count: usize,
) -> String {
    let _ = instructions_count; // reserved for future per-instruction coverage
    let mut md = String::new();
    let header_name = program.map(|p| p.name.as_str())
        .or_else(|| scenarios.first().map(|(_, j)| j.contract.as_str()))
        .unwrap_or("(unknown)");
    writeln!(md, "# Audit report — {header_name}\n").unwrap();
    if let Some(m) = metadata {
        render_metadata_header(&mut md, m);
    }
    let total_steps: usize = 0; // step counts live on ExplorationSession,
    let total_findings: usize = scenarios.iter().map(|(_, j)| j.findings.len()).sum();
    writeln!(md, "**Scenarios**: {} · **Total findings**: {}\n",
        scenarios.len(), total_findings).unwrap();
    let _ = total_steps; // step listing is rendered by the Solana caller via ExplorationStep,
                         // which the journal layer does not own — kept here for future use.

    if let Some(p) = program {
        writeln!(md, "## Program\n").unwrap();
        writeln!(md, "- Program ID: `{}`", p.program_id).unwrap();
        writeln!(md, "- Instructions: {}", p.instructions).unwrap();
        writeln!(md, "- Account types: {}\n", p.account_types).unwrap();
    }

    render_methodology(&mut md, header_name);

    let journals: Vec<&AuditJournal> = scenarios.iter().map(|(_, j)| *j).collect();
    render_severity_matrix(&mut md, &journals);
    writeln!(md).unwrap();

    writeln!(md, "## Findings (all scenarios)\n").unwrap();
    let mut any = false;
    for (scn_name, journal) in scenarios {
        for f in &journal.findings {
            any = true;
            render_finding_detail(&mut md, f, Some(scn_name));
            writeln!(md).unwrap();
        }
    }
    if !any {
        writeln!(md, "_(no findings recorded)_\n").unwrap();
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_journal_export() {
        let j = AuditJournal::new("myproject", "Staking", "2026-03-31T10:00:00Z");
        let md = export_markdown(&j, 8);
        assert!(md.contains("# Audit: Staking"));
        assert!(md.contains("0/8 functions (0%)"));
        assert!(md.contains("No findings recorded."));
        assert!(md.contains("No functions reviewed yet."));
    }

    #[test]
    fn export_with_findings_and_status() {
        let mut j = AuditJournal::new("myproject", "Staking", "2026-03-31T10:00:00Z");

        j.add_finding(Finding {
            id: String::new(),
            severity: Severity::High,
            title: "Reentrancy in withdraw".into(),
            affected_function: "withdraw".into(),
            affected_sequence: Some(vec!["deposit".into(), "withdraw".into()]),
            description: "External call before state update".into(),
            notes: vec![],
            created_at: String::new(),
            affected_step_index: None,
            recommendation: None,
        }, "2026-03-31T14:30:00Z");

        j.record(JournalEntry::StatusChanged {
            function: "deposit".into(),
            status: ReviewStatus::Clean,
            timestamp: "2026-03-31T14:00:00Z".into(),
        });
        j.record(JournalEntry::StatusChanged {
            function: "withdraw".into(),
            status: ReviewStatus::Vulnerable,
            timestamp: "2026-03-31T14:30:00Z".into(),
        });

        let md = export_markdown(&j, 8);
        assert!(md.contains("2/8 functions (25%)"));
        assert!(md.contains("| F-01 | High | Reentrancy in withdraw | withdraw |"));
        assert!(md.contains("[ok] deposit"));
        assert!(md.contains("[!] withdraw"));
        assert!(md.contains("deposit → withdraw"));
    }

    #[test]
    fn export_with_exploration_log() {
        let mut j = AuditJournal::new("p", "c", "t");
        j.record(JournalEntry::SequenceExplored {
            steps: vec!["deposit".into(), "withdraw".into()],
            timestamp: "2026-03-31T14:32:00Z".into(),
        });
        j.record(JournalEntry::BranchCreated {
            from_function: "deposit".into(),
            branch_function: "claimRewards".into(),
            timestamp: "2026-03-31T14:45:00Z".into(),
        });

        let md = export_markdown(&j, 5);
        assert!(md.contains("## Exploration Log"));
        assert!(md.contains("Explored: deposit → withdraw"));
        assert!(md.contains("Branch: deposit → claimRewards"));
    }

    #[test]
    fn timestamp_truncation_handles_short_strings() {
        let mut j = AuditJournal::new("p", "c", "t");
        j.record(JournalEntry::SequenceExplored {
            steps: vec!["x".into()],
            timestamp: "short".into(),
        });
        let md = export_markdown(&j, 1);
        assert!(md.contains("**short**"));
    }
}
