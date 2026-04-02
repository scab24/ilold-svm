use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::cfg::types::CfgGraph;
use crate::classify::entry_points::{classify_all, AccessLevel};
use crate::journal::types::{Finding, ReviewStatus, Severity};
use crate::model::contract::ContractDef;
use crate::narrative::function::build_function_narrative;
use crate::narrative::sequence::build_sequence_narrative;
use crate::narrative::types::{FunctionNarrative, SequenceNarrative};
use crate::pathtree::types::PathTree;
use crate::sequence::analysis::{FunctionBehavior, TransitionInfo};

use super::session::{ExplorationSession, VariableSummary};

pub struct AnalysisData<'a> {
    pub contract: &'a ContractDef,
    pub cfgs: &'a HashMap<(String, String), CfgGraph>,
    pub path_trees: &'a HashMap<(String, String), PathTree>,
    pub behaviors: &'a [FunctionBehavior],
    pub transitions: &'a [TransitionInfo],
    pub classifications: &'a [(String, AccessLevel)],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionCommand {
    Call { func: String },
    Back,
    Clear,
    State,
    Functions,
    Finding { severity: Severity, title: String, description: String },
    Note { text: String },
    Status { func: String, status: ReviewStatus },
    Session,
    Who { variable: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandResult {
    StepAdded {
        step_index: usize,
        function: String,
        access: AccessLevel,
        state_changed: Vec<String>,
    },
    StepRemoved {
        remaining: usize,
    },
    Cleared,
    StateView {
        summary: Vec<VariableSummary>,
    },
    FunctionList {
        functions: Vec<(String, AccessLevel)>,
    },
    FindingAdded {
        id: String,
    },
    NoteAdded,
    StatusUpdated,
    SessionView {
        contract: String,
        steps: Vec<String>,
        findings_count: usize,
    },
    VariableInfo {
        variable: String,
        writers: Vec<(String, AccessLevel)>,
        readers: Vec<(String, AccessLevel)>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CanvasPatch {
    AddNode { function: String, access: AccessLevel, step_index: usize },
    RemoveLastNode,
    ClearAll,
    Highlight { function: String },
}

pub fn canvas_patch_from(result: &CommandResult) -> Option<CanvasPatch> {
    match result {
        CommandResult::StepAdded { function, access, step_index, .. } => {
            Some(CanvasPatch::AddNode {
                function: function.clone(),
                access: access.clone(),
                step_index: *step_index,
            })
        }
        CommandResult::StepRemoved { .. } => Some(CanvasPatch::RemoveLastNode),
        CommandResult::Cleared => Some(CanvasPatch::ClearAll),
        _ => None,
    }
}

pub fn execute_command(
    cmd: SessionCommand,
    session: &mut ExplorationSession,
    data: &AnalysisData,
    timestamp: &str,
) -> CommandResult {
    match cmd {
        SessionCommand::Call { func } => execute_call(session, data, &func, timestamp),
        SessionCommand::Back => execute_back(session),
        SessionCommand::Clear => { session.clear(); CommandResult::Cleared }
        SessionCommand::State => CommandResult::StateView {
            summary: session.variable_summary(),
        },
        SessionCommand::Functions => CommandResult::FunctionList {
            functions: classify_all(data.contract),
        },
        SessionCommand::Finding { severity, title, description } => {
            execute_finding(session, severity, title, description, timestamp)
        }
        SessionCommand::Note { text } => execute_note(session, &text, timestamp),
        SessionCommand::Status { func, status } => {
            execute_status(session, &func, status, timestamp)
        }
        SessionCommand::Session => CommandResult::SessionView {
            contract: session.contract.clone(),
            steps: session.current_sequence().into_iter().map(|s| s.to_string()).collect(),
            findings_count: session.journal.findings.len(),
        },
        SessionCommand::Who { variable } => execute_who(data, &variable),
    }
}

fn execute_call(
    session: &mut ExplorationSession,
    data: &AnalysisData,
    func: &str,
    timestamp: &str,
) -> CommandResult {
    let key = (session.contract.clone(), func.to_string());

    let cfg = match data.cfgs.get(&key) {
        Some(c) => c,
        None => return CommandResult::Error {
            message: format!("Function '{}' not found. Use 'functions' to see available.", func),
        },
    };

    let function_def = match data.contract.functions.iter().find(|f| f.name == func) {
        Some(f) => f,
        None => return CommandResult::Error {
            message: format!("Function '{}' not found in contract", func),
        },
    };

    let access = crate::classify::entry_points::classify_function(function_def, data.contract);

    session.add_step(func, cfg, &data.contract.state_vars, timestamp);

    let state_changed: Vec<String> = session.steps.last()
        .map(|s| {
            let mut vars: Vec<String> = s.mutations.iter().map(|m| m.variable.clone()).collect();
            vars.sort();
            vars.dedup();
            vars
        })
        .unwrap_or_default();

    CommandResult::StepAdded {
        step_index: session.steps.len() - 1,
        function: func.to_string(),
        access,
        state_changed,
    }
}

fn execute_back(session: &mut ExplorationSession) -> CommandResult {
    if session.remove_last_step() {
        CommandResult::StepRemoved { remaining: session.steps.len() }
    } else {
        CommandResult::Error { message: "No steps to undo".into() }
    }
}

fn execute_finding(
    session: &mut ExplorationSession,
    severity: Severity,
    title: String,
    description: String,
    timestamp: &str,
) -> CommandResult {
    let affected_sequence = if session.steps.is_empty() {
        None
    } else {
        Some(session.current_sequence().into_iter().map(|s| s.to_string()).collect())
    };

    let finding = Finding {
        id: String::new(),
        severity,
        title,
        affected_function: session.steps.last().map(|s| s.function.clone()).unwrap_or_default(),
        affected_sequence,
        description,
        notes: vec![],
        created_at: String::new(),
    };

    session.journal.add_finding(finding, timestamp);
    let id = session.journal.findings.last().map(|f| f.id.clone()).unwrap_or_default();
    CommandResult::FindingAdded { id }
}

fn execute_note(session: &mut ExplorationSession, text: &str, timestamp: &str) -> CommandResult {
    let anchor = session.current_sequence().join(" → ");
    session.journal.record(crate::journal::types::JournalEntry::NoteAdded {
        anchor,
        content: text.into(),
        timestamp: timestamp.into(),
    });
    CommandResult::NoteAdded
}

fn execute_status(
    session: &mut ExplorationSession,
    func: &str,
    status: ReviewStatus,
    timestamp: &str,
) -> CommandResult {
    session.journal.record(crate::journal::types::JournalEntry::StatusChanged {
        function: func.into(),
        status,
        timestamp: timestamp.into(),
    });
    CommandResult::StatusUpdated
}

fn execute_who(data: &AnalysisData, variable: &str) -> CommandResult {
    let var_lower = variable.to_lowercase();

    let access_for = |func_name: &str| -> AccessLevel {
        data.classifications.iter()
            .find(|(name, _)| name == func_name)
            .map(|(_, access)| access.clone())
            .unwrap_or(AccessLevel::Internal)
    };

    let writers: Vec<(String, AccessLevel)> = data.behaviors.iter()
        .filter(|b| b.state_writes.iter().any(|w| w.to_lowercase() == var_lower))
        .map(|b| (b.name.clone(), access_for(&b.name)))
        .collect();

    let readers: Vec<(String, AccessLevel)> = data.behaviors.iter()
        .filter(|b| b.state_reads.iter().any(|r| r.to_lowercase() == var_lower))
        .filter(|b| !b.state_writes.iter().any(|w| w.to_lowercase() == var_lower))
        .map(|b| (b.name.clone(), access_for(&b.name)))
        .collect();

    if writers.is_empty() && readers.is_empty() {
        return CommandResult::Error {
            message: format!("Variable '{}' not found in any function", variable),
        };
    }

    CommandResult::VariableInfo { variable: variable.to_string(), writers, readers }
}

pub fn get_step_narrative(
    session: &ExplorationSession,
    step_index: usize,
    data: &AnalysisData,
) -> Result<FunctionNarrative, String> {
    let step = session.steps.get(step_index)
        .ok_or_else(|| format!("Step {} not found", step_index))?;

    let key = (session.contract.clone(), step.function.clone());
    let cfg = data.cfgs.get(&key).ok_or("No CFG")?;
    let pt = data.path_trees.get(&key).ok_or("No paths")?;
    let func = data.contract.functions.iter()
        .find(|f| f.name == step.function)
        .ok_or("Function not found")?;

    Ok(build_function_narrative(data.contract, func, pt, cfg, data.behaviors))
}

pub fn get_function_info(
    func_name: &str,
    data: &AnalysisData,
) -> Result<FunctionNarrative, String> {
    let key = (data.contract.name.clone(), func_name.to_string());
    let cfg = data.cfgs.get(&key).ok_or_else(|| format!("Function '{}' not found", func_name))?;
    let pt = data.path_trees.get(&key).ok_or("No paths")?;
    let func = data.contract.functions.iter()
        .find(|f| f.name == func_name)
        .ok_or("Function not found")?;

    Ok(build_function_narrative(data.contract, func, pt, cfg, data.behaviors))
}

pub fn get_sequence_narrative(
    session: &ExplorationSession,
    data: &AnalysisData,
) -> Result<SequenceNarrative, String> {
    if session.steps.len() < 2 {
        return Err("Need at least 2 steps for a sequence narrative".into());
    }

    let names: Vec<&str> = session.current_sequence();
    Ok(build_sequence_narrative(
        &session.contract, &names,
        data.behaviors, data.transitions, data.classifications,
    ))
}

pub fn get_session_state(session: &ExplorationSession) -> Vec<VariableSummary> {
    session.variable_summary()
}
