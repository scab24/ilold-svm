use std::collections::{BTreeSet, HashSet};

use super::backward::add_ancestors_with_uses;
use super::extract::extract_statement_uses_defs;
use super::flatten::FlatStatement;
use super::types::StatementPath;

/// Intraprocedural forward dataflow slice for `variable`.
///
/// Iterates to a fixed point over the flattened body:
/// 1. Any statement whose use set intersects the `tainted` set is added
///    to the slice and its defs are merged into `tainted` — taint
///    spreads along data dependencies.
/// 2. Every included statement drags in its lexical ancestors so that
///    enclosing control-flow headers appear in the rendered slice. The
///    ancestor's own uses are merged into `tainted` too, which keeps
///    parity with `backward_slice` and handles nested propagation.
/// The loop re-runs until neither set grows.
///
/// Returns the selected paths in program order.
pub fn forward_slice(flat: &[FlatStatement<'_>], variable: &str) -> Vec<StatementPath> {
    let mut tainted: HashSet<String> = HashSet::new();
    tainted.insert(variable.to_string());

    let mut included: BTreeSet<usize> = BTreeSet::new();

    loop {
        let mut changed = false;

        for i in 0..flat.len() {
            if included.contains(&i) {
                continue;
            }
            let (uses, defs) = extract_statement_uses_defs(flat[i].statement);
            if uses.is_disjoint(&tainted) {
                continue;
            }
            included.insert(i);
            changed = true;
            for d in defs {
                if tainted.insert(d) {
                    changed = true;
                }
            }
        }

        if add_ancestors_with_uses(flat, &mut included, &mut tainted) {
            changed = true;
        }

        if !changed {
            break;
        }
    }

    included.into_iter().map(|i| flat[i].path.clone()).collect()
}
