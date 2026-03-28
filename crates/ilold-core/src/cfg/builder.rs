use petgraph::stable_graph::NodeIndex;

use crate::model::contract::ContractDef;
use crate::model::expression::{BinaryOperator, Expression, ExpressionKind, UnaryOperator};
use crate::model::function::FunctionDef;
use crate::model::statement::{Statement, StatementKind};

use super::error::CfgError;
use super::modifier::inline_modifiers;
use super::types::*;

pub struct CfgBuilder {
    graph: CfgGraph,
    next_block_id: usize,
    current_block: NodeIndex,
}

impl CfgBuilder {
    pub fn build(function: &FunctionDef, contract: &ContractDef) -> Result<CfgGraph, CfgError> {
        let mut builder = CfgBuilder {
            graph: CfgGraph::new(),
            next_block_id: 0,
            current_block: NodeIndex::new(0), // will be replaced
        };

        let entry = builder.add_block(BlockKind::Entry);
        builder.current_block = entry;

        let body = match &function.body {
            Some(body) => body.clone(),
            None => return Ok(builder.graph), // interface/abstract — no body
        };

        // Inline modifiers if any
        let body = if function.modifiers.is_empty() {
            body
        } else {
            let modifier_defs: Vec<&_> = function
                .modifiers
                .iter()
                .filter_map(|mref| {
                    contract.modifiers.iter().find(|m| m.name == mref.name)
                })
                .collect();
            if modifier_defs.len() == function.modifiers.len() {
                inline_modifiers(&body, &modifier_defs)?
            } else {
                // Some modifiers not found (likely inherited). Skip inlining for those.
                body
            }
        };

        for stmt in &body {
            builder.process_statement(stmt);
        }

        // If current block has no outgoing edges and is NOT already terminal, add implicit return
        if !builder.block_is_terminal(builder.current_block)
            && builder
                .graph
                .edges(builder.current_block)
                .next()
                .is_none()
        {
            let ret = builder.add_block(BlockKind::Return);
            builder.add_edge(builder.current_block, ret, BranchEdge::Unconditional);
        }

        Ok(builder.graph)
    }

    fn add_block(&mut self, kind: BlockKind) -> NodeIndex {
        let id = self.next_block_id;
        self.next_block_id += 1;
        self.graph.add_node(BasicBlock {
            id,
            kind,
            statements: Vec::new(),
            span: None,
        })
    }

    fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, edge: BranchEdge) {
        self.graph.add_edge(from, to, edge);
    }

    fn add_stmt_to_current(&mut self, stmt: CfgStatement) {
        if let Some(block) = self.graph.node_weight_mut(self.current_block) {
            block.statements.push(stmt);
        }
    }

    fn process_statement(&mut self, stmt: &Statement) {
        match &stmt.kind {
            StatementKind::If { condition, then_body, else_body } => {
                self.process_if(condition, then_body, else_body.as_deref());
            }
            StatementKind::For { init, condition, increment, body } => {
                self.process_for(init.as_deref(), condition.as_ref(), increment.as_ref(), body);
            }
            StatementKind::While { condition, body } => {
                self.process_while(condition, body);
            }
            StatementKind::DoWhile { body, condition } => {
                self.process_do_while(body, condition);
            }
            StatementKind::Return { value } => {
                self.process_return(value.as_ref());
            }
            StatementKind::Revert { .. } => {
                let revert = self.add_block(BlockKind::Revert);
                self.add_edge(self.current_block, revert, BranchEdge::Unconditional);
            }
            StatementKind::Emit { event_name, .. } => {
                self.add_stmt_to_current(CfgStatement::EmitEvent {
                    event: event_name.clone(),
                    span: None,
                });
            }
            StatementKind::Block { statements } => {
                for s in statements {
                    self.process_statement(s);
                }
            }
            StatementKind::UncheckedBlock { statements } => {
                for s in statements {
                    self.process_statement(s);
                }
            }
            StatementKind::ExpressionStmt { expression } => {
                self.process_expression_stmt(expression);
            }
            StatementKind::VariableDeclaration { name, initial_value, .. } => {
                if let Some(val) = initial_value {
                    self.add_stmt_to_current(CfgStatement::Assignment {
                        target: name.clone(),
                        value: format!("{:?}", val.kind),
                        span: None,
                    });
                }
            }
            StatementKind::TryCatch { expression, clauses } => {
                self.process_try_catch(expression, clauses);
            }
            StatementKind::Assembly { .. } => {
                self.add_stmt_to_current(CfgStatement::AssemblyBlock { span: None });
            }
            StatementKind::Break | StatementKind::Continue | StatementKind::Placeholder => {
                // Break/Continue handled by loop processors
                // Placeholder should have been replaced by modifier inlining
            }
        }
    }

    fn process_if(
        &mut self,
        condition: &Expression,
        then_body: &[Statement],
        else_body: Option<&[Statement]>,
    ) {
        let cond_str = expr_to_string(condition);
        let before = self.current_block;

        // Then branch
        let then_block = self.add_block(BlockKind::Normal);
        self.add_edge(
            before,
            then_block,
            BranchEdge::ConditionalTrue { condition: cond_str.clone() },
        );
        self.current_block = then_block;
        for s in then_body {
            self.process_statement(s);
        }
        let then_end = self.current_block;

        // Else branch
        let else_end = if let Some(else_stmts) = else_body {
            let else_block = self.add_block(BlockKind::Normal);
            self.add_edge(
                before,
                else_block,
                BranchEdge::ConditionalFalse { condition: cond_str },
            );
            self.current_block = else_block;
            for s in else_stmts {
                self.process_statement(s);
            }
            self.current_block
        } else {
            // No else: false branch goes directly to join
            let join = self.add_block(BlockKind::Normal);
            self.add_edge(
                before,
                join,
                BranchEdge::ConditionalFalse { condition: cond_str },
            );
            join
        };

        // Join block: connect non-terminal branches
        let then_needs_join = !self.block_is_terminal(then_end);
        let else_needs_join = else_body.is_some() && !self.block_is_terminal(else_end);

        if then_needs_join || else_needs_join || else_body.is_none() {
            let join = if else_body.is_none() {
                else_end // already created as the join
            } else {
                self.add_block(BlockKind::Normal)
            };

            if then_needs_join {
                self.add_edge(then_end, join, BranchEdge::Unconditional);
            }
            if else_needs_join {
                self.add_edge(else_end, join, BranchEdge::Unconditional);
            }
            self.current_block = join;
        }
    }

    fn process_for(
        &mut self,
        init: Option<&Statement>,
        condition: Option<&Expression>,
        increment: Option<&Expression>,
        body: &[Statement],
    ) {
        // Init
        if let Some(init_stmt) = init {
            self.process_statement(init_stmt);
        }

        // Condition check
        let cond_block = self.add_block(BlockKind::LoopCondition);
        self.add_edge(self.current_block, cond_block, BranchEdge::Unconditional);

        let exit_block = self.add_block(BlockKind::Normal);
        let body_block = self.add_block(BlockKind::Normal);

        if let Some(cond) = condition {
            let cond_str = expr_to_string(cond);
            self.add_edge(
                cond_block,
                body_block,
                BranchEdge::ConditionalTrue { condition: cond_str.clone() },
            );
            self.add_edge(
                cond_block,
                exit_block,
                BranchEdge::ConditionalFalse { condition: cond_str },
            );
        } else {
            // for(;;) — always enters body
            self.add_edge(cond_block, body_block, BranchEdge::Unconditional);
        }

        // Body
        self.current_block = body_block;
        for s in body {
            self.process_statement(s);
        }

        // Increment + loop back
        if let Some(_incr) = increment {
            // increment is an expression, not a separate block for simplicity
        }
        if !self.block_is_terminal(self.current_block) {
            self.add_edge(self.current_block, cond_block, BranchEdge::LoopBack);
        }

        self.current_block = exit_block;
    }

    fn process_while(&mut self, condition: &Expression, body: &[Statement]) {
        let cond_str = expr_to_string(condition);

        let cond_block = self.add_block(BlockKind::LoopCondition);
        self.add_edge(self.current_block, cond_block, BranchEdge::Unconditional);

        let body_block = self.add_block(BlockKind::Normal);
        let exit_block = self.add_block(BlockKind::Normal);

        self.add_edge(
            cond_block,
            body_block,
            BranchEdge::ConditionalTrue { condition: cond_str.clone() },
        );
        self.add_edge(
            cond_block,
            exit_block,
            BranchEdge::ConditionalFalse { condition: cond_str },
        );

        self.current_block = body_block;
        for s in body {
            self.process_statement(s);
        }
        if !self.block_is_terminal(self.current_block) {
            self.add_edge(self.current_block, cond_block, BranchEdge::LoopBack);
        }

        self.current_block = exit_block;
    }

    fn process_do_while(&mut self, body: &[Statement], condition: &Expression) {
        let cond_str = expr_to_string(condition);

        let body_block = self.add_block(BlockKind::Normal);
        self.add_edge(self.current_block, body_block, BranchEdge::Unconditional);

        self.current_block = body_block;
        for s in body {
            self.process_statement(s);
        }

        let cond_block = self.add_block(BlockKind::LoopCondition);
        if !self.block_is_terminal(self.current_block) {
            self.add_edge(self.current_block, cond_block, BranchEdge::Unconditional);
        }

        let exit_block = self.add_block(BlockKind::Normal);
        self.add_edge(
            cond_block,
            body_block,
            BranchEdge::ConditionalTrue { condition: cond_str.clone() },
        );
        self.add_edge(
            cond_block,
            exit_block,
            BranchEdge::ConditionalFalse { condition: cond_str },
        );

        self.current_block = exit_block;
    }

    fn process_return(&mut self, value: Option<&Expression>) {
        if let Some(expr) = value {
            for s in classify_expression(expr) {
                self.add_stmt_to_current(s);
            }
        }
        let ret = self.add_block(BlockKind::Return);
        self.add_edge(self.current_block, ret, BranchEdge::Unconditional);
        self.current_block = ret;
    }

    fn process_try_catch(
        &mut self,
        expression: &Expression,
        clauses: &[crate::model::statement::CatchClause],
    ) {
        let before = self.current_block;
        for s in classify_expression(expression) {
            self.add_stmt_to_current(s);
        }

        let join = self.add_block(BlockKind::Normal);

        for (i, clause) in clauses.iter().enumerate() {
            let clause_block = self.add_block(BlockKind::Normal);
            let edge = if i == 0 {
                // First clause is the success case (returns)
                BranchEdge::ExternalCallSuccess
            } else {
                BranchEdge::CatchClause {
                    kind: clause.name.clone().unwrap_or("default".into()),
                }
            };
            self.add_edge(before, clause_block, edge);

            self.current_block = clause_block;
            for s in &clause.body {
                self.process_statement(s);
            }
            if !self.block_is_terminal(self.current_block) {
                self.add_edge(self.current_block, join, BranchEdge::Unconditional);
            }
        }

        self.current_block = join;
    }

    fn process_expression_stmt(&mut self, expr: &Expression) {
        // Check if this is a require/assert call
        if let ExpressionKind::FunctionCall { callee, arguments } = &expr.kind {
            if let ExpressionKind::Identifier { name } = &callee.kind {
                match name.as_str() {
                    "require" => {
                        self.process_require(arguments);
                        return;
                    }
                    "assert" => {
                        self.process_assert(arguments);
                        return;
                    }
                    _ => {}
                }
            }
        }

        // Not require/assert — classify and add to current block
        for s in classify_expression(expr) {
            self.add_stmt_to_current(s);
        }
    }

    fn process_require(&mut self, arguments: &[Expression]) {
        let condition = arguments.first().map(expr_to_string).unwrap_or_default();
        let message = arguments.get(1).map(expr_to_string);

        let cond_str = condition.clone();
        self.add_stmt_to_current(CfgStatement::RequireCheck {
            condition,
            message,
            span: None,
        });

        let before = self.current_block;

        // True branch: continues
        let next = self.add_block(BlockKind::Normal);
        self.add_edge(
            before,
            next,
            BranchEdge::ConditionalTrue { condition: cond_str.clone() },
        );

        // False branch: reverts
        let revert = self.add_block(BlockKind::Revert);
        self.add_edge(
            before,
            revert,
            BranchEdge::ConditionalFalse { condition: cond_str },
        );

        self.current_block = next;
    }

    fn process_assert(&mut self, arguments: &[Expression]) {
        let condition = arguments.first().map(expr_to_string).unwrap_or_default();
        let cond_str = condition.clone();

        self.add_stmt_to_current(CfgStatement::AssertCheck {
            condition,
            span: None,
        });

        let before = self.current_block;

        let next = self.add_block(BlockKind::Normal);
        self.add_edge(
            before,
            next,
            BranchEdge::ConditionalTrue { condition: cond_str.clone() },
        );

        let revert = self.add_block(BlockKind::Revert);
        self.add_edge(
            before,
            revert,
            BranchEdge::ConditionalFalse { condition: cond_str },
        );

        self.current_block = next;
    }

    fn block_is_terminal(&self, idx: NodeIndex) -> bool {
        let kind = self.graph[idx].kind;
        kind == BlockKind::Return || kind == BlockKind::Revert
    }
}

// ============================================================================
// Expression classification
// ============================================================================

/// Classify an expression into CfgStatements for the detection engine.
/// Returns multiple statements when an expression contains embedded calls
/// (e.g., `x = foo()` produces both an Assignment and an InternalCall).
fn classify_expression(expr: &Expression) -> Vec<CfgStatement> {
    let mut stmts = Vec::new();
    collect_calls(expr, &mut stmts);

    match &expr.kind {
        ExpressionKind::FunctionCall { .. } => {
            // Already handled by collect_calls
        }
        ExpressionKind::Assignment { target, .. } => {
            stmts.push(CfgStatement::Assignment {
                target: expr_to_string(target),
                value: expr_to_string(expr),
                span: None,
            });
        }
        _ => {
            stmts.push(CfgStatement::Assignment {
                target: String::new(),
                value: expr_to_string(expr),
                span: None,
            });
        }
    }

    stmts
}

/// Recursively scan an expression for function calls and add them as CfgStatements.
fn collect_calls(expr: &Expression, stmts: &mut Vec<CfgStatement>) {
    match &expr.kind {
        ExpressionKind::FunctionCall { callee, arguments } => {
            // Classify this call
            match &callee.kind {
                ExpressionKind::Identifier { name } => {
                    stmts.push(CfgStatement::InternalCall {
                        function: name.clone(),
                        span: None,
                    });
                }
                ExpressionKind::MemberAccess { object, member } => {
                    if let ExpressionKind::Identifier { name } = &object.kind {
                        if name == "this" || name == "super" {
                            stmts.push(CfgStatement::InternalCall {
                                function: member.clone(),
                                span: None,
                            });
                        } else {
                            stmts.push(CfgStatement::ExternalCall {
                                target: name.clone(),
                                function: member.clone(),
                                span: None,
                            });
                        }
                    } else {
                        stmts.push(CfgStatement::ExternalCall {
                            target: expr_to_string(object),
                            function: member.clone(),
                            span: None,
                        });
                    }
                }
                _ => {
                    stmts.push(CfgStatement::InternalCall {
                        function: expr_to_string(callee),
                        span: None,
                    });
                }
            }
            // Also recurse into arguments (they might contain calls too)
            for arg in arguments {
                collect_calls(arg, stmts);
            }
        }
        // Recurse into sub-expressions
        ExpressionKind::Assignment { target, value, .. } => {
            collect_calls(target, stmts);
            collect_calls(value, stmts);
        }
        ExpressionKind::BinaryOp { left, right, .. } => {
            collect_calls(left, stmts);
            collect_calls(right, stmts);
        }
        ExpressionKind::UnaryOp { operand, .. } => {
            collect_calls(operand, stmts);
        }
        ExpressionKind::MemberAccess { object, .. } => {
            collect_calls(object, stmts);
        }
        ExpressionKind::IndexAccess { base, index } => {
            collect_calls(base, stmts);
            if let Some(idx) = index {
                collect_calls(idx, stmts);
            }
        }
        ExpressionKind::Ternary { condition, true_expr, false_expr } => {
            collect_calls(condition, stmts);
            collect_calls(true_expr, stmts);
            collect_calls(false_expr, stmts);
        }
        _ => {}
    }
}

fn expr_to_string(expr: &Expression) -> String {
    match &expr.kind {
        ExpressionKind::Identifier { name } => name.clone(),
        ExpressionKind::Literal { value, .. } => value.clone(),
        ExpressionKind::MemberAccess { object, member } => {
            format!("{}.{}", expr_to_string(object), member)
        }
        ExpressionKind::FunctionCall { callee, arguments } => {
            let args: Vec<String> = arguments.iter().map(expr_to_string).collect();
            format!("{}({})", expr_to_string(callee), args.join(", "))
        }
        ExpressionKind::BinaryOp { left, operator, right } => {
            format!("{} {} {}", expr_to_string(left), binop_to_str(operator), expr_to_string(right))
        }
        ExpressionKind::UnaryOp { operator, operand } => match operator {
            UnaryOperator::Not => format!("!{}", expr_to_string(operand)),
            UnaryOperator::Neg => format!("-{}", expr_to_string(operand)),
            UnaryOperator::BitNot => format!("~{}", expr_to_string(operand)),
            _ => format!("{:?}({})", operator, expr_to_string(operand)),
        },
        ExpressionKind::IndexAccess { base, index } => {
            let idx = index.as_ref().map(|e| expr_to_string(e)).unwrap_or_default();
            format!("{}[{}]", expr_to_string(base), idx)
        }
        ExpressionKind::Assignment { target, value, .. } => {
            format!("{} = {}", expr_to_string(target), expr_to_string(value))
        }
        ExpressionKind::Ternary { condition, true_expr, false_expr } => {
            format!("{} ? {} : {}", expr_to_string(condition), expr_to_string(true_expr), expr_to_string(false_expr))
        }
        ExpressionKind::TypeCast { type_name, expression } => {
            format!("{type_name}({})", expr_to_string(expression))
        }
        ExpressionKind::TypeMeta { type_name } => {
            format!("type({type_name})")
        }
        ExpressionKind::New { type_name, arguments } => {
            let args: Vec<String> = arguments.iter().map(expr_to_string).collect();
            format!("new {type_name}({})", args.join(", "))
        }
    }
}

fn binop_to_str(op: &BinaryOperator) -> &'static str {
    match op {
        BinaryOperator::Add => "+",
        BinaryOperator::Sub => "-",
        BinaryOperator::Mul => "*",
        BinaryOperator::Div => "/",
        BinaryOperator::Mod => "%",
        BinaryOperator::Pow => "**",
        BinaryOperator::Eq => "==",
        BinaryOperator::Neq => "!=",
        BinaryOperator::Lt => "<",
        BinaryOperator::Gt => ">",
        BinaryOperator::Lte => "<=",
        BinaryOperator::Gte => ">=",
        BinaryOperator::And => "&&",
        BinaryOperator::Or => "||",
        BinaryOperator::BitAnd => "&",
        BinaryOperator::BitOr => "|",
        BinaryOperator::BitXor => "^",
        BinaryOperator::Shl => "<<",
        BinaryOperator::Shr => ">>",
    }
}
