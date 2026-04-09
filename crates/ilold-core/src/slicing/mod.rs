// Intraprocedural backward + forward dataflow slicing over the model AST.
//
// Operates on `FunctionDef.body: Vec<Statement>` directly, not on the CFG.
// The CFG stringifies expressions and discards structure; we need the
// structured AST to extract variable use/def sets.
//
// Known extraction limitations (v1, documented so callers can reason
// about false negatives in the slice):
//
//   * Only `Assignment` expressions produce DEFs. Solidity mutations that
//     are not modelled as Assignment — `x++`, `--x`, `delete x`,
//     `arr.push(v)`, `arr.pop()` — are captured as USEs of `x`/`arr` but
//     NOT as DEFs. A slice on a variable mutated only through `.push()`
//     will miss the mutating call as a def.
//   * Tuple destructuring assignments `(a, b) = foo()` may not hit the
//     top-level `Assignment` arm depending on how the frontend lowers
//     them; in that case sub-expressions are treated as USEs only.
//   * Function-call side effects on state are not tracked — this is an
//     intraprocedural slice, not a whole-program one.
//
// These are acceptable for the audit use case (the 80% that relies on
// plain `x = …` assignments works correctly). Lifting them requires an
// effects database, which is out of scope for Phase 2b.

pub mod backward;
pub mod extract;
pub mod flatten;
pub mod forward;
pub mod types;

use crate::model::contract::ContractDef;
use crate::model::function::FunctionDef;
use crate::model::project::Project;

pub use types::{SliceDirection, SliceEntry, SliceResult, StatementOrigin, StatementPath};

/// Compute a dataflow slice for `variable` in `function`.
///
/// The slicer walks both the function body AND the bodies of every
/// applied modifier (resolved through the inheritance chain), so writes
/// hidden inside `nonReentrant` / `updateReward` / etc. show up in the
/// slice. Each entry carries an `origin` tag distinguishing
/// function-body statements from modifier statements.
///
/// `Backward`: statements whose values feed into reads of `variable`.
/// `Forward`:  statements whose values derive from writes of `variable`.
/// `Both`:     union of backward and forward, annotated per entry.
pub fn build_slice_result(
    project: &Project,
    contract: &ContractDef,
    function: &FunctionDef,
    variable: &str,
    direction: SliceDirection,
) -> SliceResult {
    let flat = flatten::flatten_function(project, contract, function);

    if flat.is_empty() {
        return SliceResult {
            function: function.name.clone(),
            variable: variable.to_string(),
            direction,
            backward: Vec::new(),
            forward: Vec::new(),
        };
    }

    let backward = if matches!(direction, SliceDirection::Backward | SliceDirection::Both) {
        backward::backward_slice(&flat, variable)
    } else {
        Vec::new()
    };

    let forward = if matches!(direction, SliceDirection::Forward | SliceDirection::Both) {
        forward::forward_slice(&flat, variable)
    } else {
        Vec::new()
    };

    SliceResult {
        function: function.name.clone(),
        variable: variable.to_string(),
        direction,
        backward: backward.into_iter().map(|p| entry_for(&flat, p)).collect(),
        forward: forward.into_iter().map(|p| entry_for(&flat, p)).collect(),
    }
}

fn entry_for(flat: &[flatten::FlatStatement<'_>], path: StatementPath) -> SliceEntry {
    let entry = flat.iter().find(|f| f.path == path);
    match entry {
        Some(f) => SliceEntry {
            path,
            span: Some(f.statement.span),
            text: extract::statement_text(f.statement),
            origin: f.origin.clone(),
        },
        None => SliceEntry {
            path,
            span: None,
            text: String::new(),
            origin: StatementOrigin::FunctionBody,
        },
    }
}
