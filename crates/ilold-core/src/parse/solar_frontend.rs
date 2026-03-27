use std::path::{Path, PathBuf};

use solar::ast::{self, ExprKind, ItemKind, LitKind, StmtKind};
use solar::interface::{ColorChoice, Session};
use solar::parse::Parser;

use crate::model::common::*;
use crate::model::contract::*;
use crate::model::expression::*;
use crate::model::function::*;
use crate::model::modifier::*;
use crate::model::project::*;
use crate::model::statement::*;

use super::error::ParseError;
use super::ProjectParser;

pub struct SolarParser;

impl ProjectParser for SolarParser {
    fn parse(&self, paths: &[PathBuf]) -> Result<Project, ParseError> {
        let mut all_contracts = Vec::new();
        let mut source_files = Vec::new();

        for path in paths {
            if !path.exists() {
                return Err(ParseError::FileNotFound {
                    path: path.display().to_string(),
                });
            }

            let src = std::fs::read_to_string(path).map_err(|e| ParseError::Internal {
                message: format!("Failed to read {}: {e}", path.display()),
            })?;

            let file_index = source_files.len();
            source_files.push(SourceFile {
                path: path.display().to_string(),
                content: src.clone(),
            });

            let contracts = parse_single_file(path, &src, file_index)?;
            all_contracts.extend(contracts);
        }

        let mut project = Project {
            source_files,
            contracts: all_contracts,
            contract_index: Default::default(),
        };
        project.rebuild_index();
        Ok(project)
    }
}

fn parse_single_file(
    path: &Path,
    src: &str,
    file_index: usize,
) -> Result<Vec<ContractDef>, ParseError> {
    let sess = Session::builder()
        .with_buffer_emitter(ColorChoice::Never)
        .build();

    let mut contracts = Vec::new();

    let result = sess.enter(|| -> Result<(), ParseError> {
        let arena = ast::Arena::new();
        let mut parser = Parser::from_source_code(
            &sess,
            &arena,
            solar::interface::source_map::FileName::Real(path.to_path_buf()),
            src.to_string(),
        )
        .map_err(|_| ParseError::SyntaxError {
            path: path.display().to_string(),
            message: "Failed to initialize parser".into(),
            span: None,
        })?;

        let source_unit = parser.parse_file().map_err(|e| {
            e.emit();
            ParseError::SyntaxError {
                path: path.display().to_string(),
                message: "Syntax error".into(),
                span: None,
            }
        })?;

        for item in source_unit.items.iter() {
            if let ItemKind::Contract(ref contract) = item.kind {
                contracts.push(convert_contract(contract, file_index));
            }
        }

        Ok(())
    });

    result?;
    Ok(contracts)
}

// ============================================================================
// Contract
// ============================================================================

fn convert_contract(contract: &ast::ItemContract<'_>, file_index: usize) -> ContractDef {
    let mut functions = Vec::new();
    let mut modifiers = Vec::new();
    let mut state_vars = Vec::new();
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut events = Vec::new();
    let mut errors = Vec::new();

    for item in contract.body.iter() {
        match &item.kind {
            ItemKind::Function(f) => {
                if f.kind == ast::FunctionKind::Modifier {
                    modifiers.push(convert_modifier_def(f, file_index));
                } else {
                    functions.push(convert_function(f, file_index));
                }
            }
            ItemKind::Variable(v) => {
                state_vars.push(convert_state_var(v, file_index));
            }
            ItemKind::Struct(s) => {
                structs.push(convert_struct(s, file_index));
            }
            ItemKind::Enum(e) => {
                enums.push(convert_enum(e, file_index));
            }
            ItemKind::Event(ev) => {
                events.push(convert_event(ev, file_index));
            }
            ItemKind::Error(er) => {
                errors.push(convert_error(er, file_index));
            }
            _ => {}
        }
    }

    let inherits: Vec<String> = contract
        .bases
        .iter()
        .map(|base| path_to_string(&base.name))
        .collect();

    ContractDef {
        name: contract.name.as_str().to_string(),
        kind: convert_contract_kind(contract.kind),
        functions,
        modifiers,
        state_vars,
        structs,
        enums,
        events,
        errors,
        inherits,
        span: make_span(file_index),
    }
}

fn convert_contract_kind(kind: ast::ContractKind) -> ContractKind {
    match kind {
        ast::ContractKind::Contract => ContractKind::Contract,
        ast::ContractKind::Interface => ContractKind::Interface,
        ast::ContractKind::Library => ContractKind::Library,
        ast::ContractKind::AbstractContract => ContractKind::Abstract,
    }
}

// ============================================================================
// Functions and modifiers
// ============================================================================

fn convert_function(f: &ast::ItemFunction<'_>, file_index: usize) -> FunctionDef {
    let name = f
        .header
        .name
        .as_ref()
        .map(|n| n.as_str().to_string())
        .unwrap_or_default();

    let kind = match f.kind {
        ast::FunctionKind::Constructor => FunctionKind::Constructor,
        ast::FunctionKind::Fallback => FunctionKind::Fallback,
        ast::FunctionKind::Receive => FunctionKind::Receive,
        _ => FunctionKind::Function,
    };

    // FunctionHeader has helper methods that extract from Spanned
    let visibility = f
        .header
        .visibility()
        .map(convert_visibility)
        .unwrap_or(Visibility::Internal);

    let mutability = convert_mutability(f.header.state_mutability());

    let modifiers: Vec<ModifierRef> = f
        .header
        .modifiers
        .iter()
        .map(|m| convert_modifier_ref(m, file_index))
        .collect();

    let params = convert_param_list(&f.header.parameters);
    let returns = convert_returns(&f.header);
    let body = f
        .body
        .as_ref()
        .map(|block| convert_block_stmts(block, file_index));

    FunctionDef {
        name,
        kind,
        visibility,
        mutability,
        modifiers,
        params,
        returns,
        body,
        is_virtual: f.header.virtual_(),
        is_override: f.header.override_.is_some(),
        span: make_span(file_index),
    }
}

fn convert_modifier_def(f: &ast::ItemFunction<'_>, file_index: usize) -> ModifierDef {
    let name = f
        .header
        .name
        .as_ref()
        .map(|n| n.as_str().to_string())
        .unwrap_or_default();

    let params = convert_param_list(&f.header.parameters);
    let body = f
        .body
        .as_ref()
        .map(|block| convert_block_stmts(block, file_index))
        .unwrap_or_default();

    ModifierDef {
        name,
        params,
        body,
        span: make_span(file_index),
    }
}

fn convert_modifier_ref(m: &ast::Modifier<'_>, file_index: usize) -> ModifierRef {
    let name = path_to_string(&m.name);
    let arguments: Vec<Expression> = m.arguments.exprs().map(|e| convert_expression(e, file_index)).collect();
    ModifierRef { name, arguments }
}

fn convert_visibility(v: ast::Visibility) -> Visibility {
    match v {
        ast::Visibility::Public => Visibility::Public,
        ast::Visibility::External => Visibility::External,
        ast::Visibility::Internal => Visibility::Internal,
        ast::Visibility::Private => Visibility::Private,
    }
}

fn convert_mutability(m: ast::StateMutability) -> Mutability {
    match m {
        ast::StateMutability::Pure => Mutability::Pure,
        ast::StateMutability::View => Mutability::View,
        ast::StateMutability::Payable => Mutability::Payable,
        ast::StateMutability::NonPayable => Mutability::NonPayable,
    }
}

fn convert_param_list(params: &ast::ParameterList<'_>) -> Vec<Param> {
    params
        .iter()
        .map(|p| Param {
            name: p
                .name
                .as_ref()
                .map(|n| n.as_str().to_string())
                .unwrap_or_default(),
            type_name: type_to_string(&p.ty),
        })
        .collect()
}

fn convert_returns(header: &ast::FunctionHeader<'_>) -> Vec<Param> {
    header
        .returns()
        .iter()
        .map(|p| Param {
            name: p
                .name
                .as_ref()
                .map(|n| n.as_str().to_string())
                .unwrap_or_default(),
            type_name: type_to_string(&p.ty),
        })
        .collect()
}

// ============================================================================
// State vars, structs, enums, events, errors
// ============================================================================

fn convert_state_var(v: &ast::VariableDefinition<'_>, file_index: usize) -> StateVar {
    let visibility = v
        .visibility
        .map(convert_visibility)
        .unwrap_or(Visibility::Internal);

    let is_constant = v
        .mutability
        .map(|m| m == ast::VarMut::Constant)
        .unwrap_or(false);

    let is_immutable = v
        .mutability
        .map(|m| m == ast::VarMut::Immutable)
        .unwrap_or(false);

    let initial_value = v
        .initializer
        .as_ref()
        .map(|expr| convert_expression(expr, file_index));

    StateVar {
        name: v
            .name
            .as_ref()
            .map(|n| n.as_str().to_string())
            .unwrap_or_default(),
        type_name: type_to_string(&v.ty),
        visibility,
        is_constant,
        is_immutable,
        initial_value,
        span: make_span(file_index),
    }
}

fn convert_struct(s: &ast::ItemStruct<'_>, file_index: usize) -> StructDef {
    let fields = s
        .fields
        .iter()
        .map(|f| Param {
            name: f
                .name
                .as_ref()
                .map(|n| n.as_str().to_string())
                .unwrap_or_default(),
            type_name: type_to_string(&f.ty),
        })
        .collect();

    StructDef {
        name: s.name.as_str().to_string(),
        fields,
        span: make_span(file_index),
    }
}

fn convert_enum(e: &ast::ItemEnum<'_>, file_index: usize) -> EnumDef {
    EnumDef {
        name: e.name.as_str().to_string(),
        variants: e.variants.iter().map(|v| v.as_str().to_string()).collect(),
        span: make_span(file_index),
    }
}

fn convert_event(ev: &ast::ItemEvent<'_>, file_index: usize) -> EventDef {
    let params = ev
        .parameters
        .iter()
        .map(|p| Param {
            name: p
                .name
                .as_ref()
                .map(|n| n.as_str().to_string())
                .unwrap_or_default(),
            type_name: type_to_string(&p.ty),
        })
        .collect();

    EventDef {
        name: ev.name.as_str().to_string(),
        params,
        span: make_span(file_index),
    }
}

fn convert_error(er: &ast::ItemError<'_>, file_index: usize) -> ErrorDef {
    let params = er
        .parameters
        .iter()
        .map(|p| Param {
            name: p
                .name
                .as_ref()
                .map(|n| n.as_str().to_string())
                .unwrap_or_default(),
            type_name: type_to_string(&p.ty),
        })
        .collect();

    ErrorDef {
        name: er.name.as_str().to_string(),
        params,
        span: make_span(file_index),
    }
}

// ============================================================================
// Statements
// ============================================================================

fn convert_block_stmts(block: &ast::Block<'_>, file_index: usize) -> Vec<Statement> {
    block
        .stmts
        .iter()
        .map(|s| convert_statement(s, file_index))
        .collect()
}

fn convert_statement(stmt: &ast::Stmt<'_>, file_index: usize) -> Statement {
    let kind = match &stmt.kind {
        StmtKind::If(condition, then_stmt, else_stmt) => StatementKind::If {
            condition: convert_expression(condition, file_index),
            then_body: wrap_stmt_as_body(then_stmt, file_index),
            else_body: else_stmt
                .as_ref()
                .map(|s| wrap_stmt_as_body(s, file_index)),
        },

        StmtKind::For { init, cond, next, body } => StatementKind::For {
            init: init
                .as_ref()
                .map(|s| Box::new(convert_statement(s, file_index))),
            condition: cond
                .as_ref()
                .map(|e| convert_expression(e, file_index)),
            increment: next
                .as_ref()
                .map(|e| convert_expression(e, file_index)),
            body: wrap_stmt_as_body(body, file_index),
        },

        StmtKind::While(condition, body) => StatementKind::While {
            condition: convert_expression(condition, file_index),
            body: wrap_stmt_as_body(body, file_index),
        },

        StmtKind::DoWhile(body, condition) => StatementKind::DoWhile {
            body: wrap_stmt_as_body(body, file_index),
            condition: convert_expression(condition, file_index),
        },

        StmtKind::Block(block) => StatementKind::Block {
            statements: convert_block_stmts(block, file_index),
        },

        StmtKind::UncheckedBlock(block) => StatementKind::UncheckedBlock {
            statements: convert_block_stmts(block, file_index),
        },

        StmtKind::Return(value) => StatementKind::Return {
            value: value.as_ref().map(|e| convert_expression(e, file_index)),
        },

        StmtKind::Emit(path, args) => StatementKind::Emit {
            event_name: path_to_string(path),
            arguments: args.exprs().map(|e| convert_expression(e, file_index)).collect(),
        },

        StmtKind::Revert(path, args) => StatementKind::Revert {
            error_name: Some(path_to_string(path)),
            arguments: args.exprs().map(|e| convert_expression(e, file_index)).collect(),
        },

        StmtKind::Expr(expr) => StatementKind::ExpressionStmt {
            expression: convert_expression(expr, file_index),
        },

        StmtKind::DeclSingle(var) => StatementKind::VariableDeclaration {
            name: var
                .name
                .as_ref()
                .map(|n| n.as_str().to_string())
                .unwrap_or_default(),
            type_name: type_to_string(&var.ty),
            initial_value: var
                .initializer
                .as_ref()
                .map(|e| convert_expression(e, file_index)),
        },

        StmtKind::Assembly(..) => StatementKind::Assembly {
            span: make_span(file_index),
        },

        StmtKind::Break => StatementKind::Break,
        StmtKind::Continue => StatementKind::Continue,
        StmtKind::Placeholder => StatementKind::Placeholder,

        _ => StatementKind::ExpressionStmt {
            expression: Expression {
                kind: ExpressionKind::Identifier {
                    name: "/* unsupported stmt */".into(),
                },
                span: make_span(file_index),
            },
        },
    };

    Statement {
        kind,
        span: make_span(file_index),
    }
}

fn wrap_stmt_as_body(stmt: &ast::Stmt<'_>, file_index: usize) -> Vec<Statement> {
    match &stmt.kind {
        StmtKind::Block(block) => convert_block_stmts(block, file_index),
        _ => vec![convert_statement(stmt, file_index)],
    }
}

// ============================================================================
// Expressions
// ============================================================================

fn convert_expression(expr: &ast::Expr<'_>, file_index: usize) -> Expression {
    let kind = match &expr.kind {
        ExprKind::Call(callee, args) => ExpressionKind::FunctionCall {
            callee: Box::new(convert_expression(callee, file_index)),
            arguments: args.exprs().map(|e| convert_expression(e, file_index)).collect(),
        },

        ExprKind::CallOptions(callee, _options) => ExpressionKind::FunctionCall {
            callee: Box::new(convert_expression(callee, file_index)),
            arguments: Vec::new(),
        },

        ExprKind::Member(object, member) => ExpressionKind::MemberAccess {
            object: Box::new(convert_expression(object, file_index)),
            member: member.as_str().to_string(),
        },

        ExprKind::Index(base, index_kind) => ExpressionKind::IndexAccess {
            base: Box::new(convert_expression(base, file_index)),
            index: match index_kind {
                ast::IndexKind::Index(Some(expr)) => {
                    Some(Box::new(convert_expression(expr, file_index)))
                }
                _ => None,
            },
        },

        ExprKind::Binary(left, op, right) => ExpressionKind::BinaryOp {
            left: Box::new(convert_expression(left, file_index)),
            operator: convert_binop_kind(op.kind),
            right: Box::new(convert_expression(right, file_index)),
        },

        ExprKind::Unary(op, operand) => ExpressionKind::UnaryOp {
            operator: convert_unop_kind(op.kind),
            operand: Box::new(convert_expression(operand, file_index)),
        },

        ExprKind::Assign(target, op, value) => ExpressionKind::Assignment {
            target: Box::new(convert_expression(target, file_index)),
            operator: op
                .as_ref()
                .map(|o| convert_assign_op_kind(o.kind))
                .unwrap_or(AssignOperator::Assign),
            value: Box::new(convert_expression(value, file_index)),
        },

        ExprKind::Ternary(cond, true_expr, false_expr) => ExpressionKind::Ternary {
            condition: Box::new(convert_expression(cond, file_index)),
            true_expr: Box::new(convert_expression(true_expr, file_index)),
            false_expr: Box::new(convert_expression(false_expr, file_index)),
        },

        ExprKind::Ident(ident) => ExpressionKind::Identifier {
            name: ident.as_str().to_string(),
        },

        ExprKind::Lit(lit, _sub) => ExpressionKind::Literal {
            value: format!("{lit}"),
            literal_type: convert_literal_type(&lit.kind),
        },

        ExprKind::New(ty) => ExpressionKind::New {
            type_name: type_to_string(ty),
            arguments: Vec::new(),
        },

        ExprKind::Type(ty) => ExpressionKind::TypeCast {
            type_name: type_to_string(ty),
            expression: Box::new(Expression {
                kind: ExpressionKind::Identifier { name: String::new() },
                span: make_span(file_index),
            }),
        },

        _ => ExpressionKind::Identifier {
            name: "/* unsupported expr */".into(),
        },
    };

    Expression {
        kind,
        span: make_span(file_index),
    }
}

// ============================================================================
// Operator conversion
// ============================================================================

fn convert_binop_kind(kind: ast::BinOpKind) -> BinaryOperator {
    use ast::BinOpKind::*;
    match kind {
        Add => BinaryOperator::Add,
        Sub => BinaryOperator::Sub,
        Mul => BinaryOperator::Mul,
        Div => BinaryOperator::Div,
        Rem => BinaryOperator::Mod,
        Pow => BinaryOperator::Pow,
        Eq => BinaryOperator::Eq,
        Ne => BinaryOperator::Neq,
        Lt => BinaryOperator::Lt,
        Gt => BinaryOperator::Gt,
        Le => BinaryOperator::Lte,
        Ge => BinaryOperator::Gte,
        And => BinaryOperator::And,
        Or => BinaryOperator::Or,
        BitAnd => BinaryOperator::BitAnd,
        BitOr => BinaryOperator::BitOr,
        BitXor => BinaryOperator::BitXor,
        Shl => BinaryOperator::Shl,
        Shr | Sar => BinaryOperator::Shr,
    }
}

fn convert_unop_kind(kind: ast::UnOpKind) -> UnaryOperator {
    use ast::UnOpKind::*;
    match kind {
        Not => UnaryOperator::Not,
        Neg => UnaryOperator::Neg,
        BitNot => UnaryOperator::BitNot,
        PreInc => UnaryOperator::PreIncrement,
        PreDec => UnaryOperator::PreDecrement,
        PostInc => UnaryOperator::PostIncrement,
        PostDec => UnaryOperator::PostDecrement,
    }
}

fn convert_assign_op_kind(kind: ast::BinOpKind) -> AssignOperator {
    use ast::BinOpKind::*;
    match kind {
        Add => AssignOperator::AddAssign,
        Sub => AssignOperator::SubAssign,
        Mul => AssignOperator::MulAssign,
        Div => AssignOperator::DivAssign,
        Rem => AssignOperator::ModAssign,
        BitAnd => AssignOperator::BitAndAssign,
        BitOr => AssignOperator::BitOrAssign,
        BitXor => AssignOperator::BitXorAssign,
        Shl => AssignOperator::ShlAssign,
        Shr | Sar => AssignOperator::ShrAssign,
        _ => AssignOperator::Assign,
    }
}

fn convert_literal_type(kind: &LitKind<'_>) -> LiteralType {
    match kind {
        LitKind::Number(_) => LiteralType::Number,
        LitKind::Str(..) => LiteralType::String,
        LitKind::Bool(_) => LiteralType::Bool,
        LitKind::Address(_) => LiteralType::Address,
        _ => LiteralType::String,
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Converts a solar Type to a readable string like "uint256", "address",
/// "mapping(address => uint256)", "uint256[]", or the custom type name.
/// Solar's Type doesn't implement Display, so we need our own conversion.
fn type_to_string(ty: &ast::Type<'_>) -> String {
    match &ty.kind {
        ast::TypeKind::Elementary(elem) => format!("{elem}"),
        ast::TypeKind::Custom(path) => path_to_string(path),
        ast::TypeKind::Array(arr) => {
            let elem = type_to_string(&arr.element);
            match &arr.size {
                Some(_size) => format!("{elem}[/*sized*/]"),
                None => format!("{elem}[]"),
            }
        }
        ast::TypeKind::Mapping(m) => {
            let key = type_to_string(&m.key);
            let value = type_to_string(&m.value);
            format!("mapping({key} => {value})")
        }
        ast::TypeKind::Function(_) => "function".into(),
    }
}

fn path_to_string(path: &ast::AstPath<'_>) -> String {
    path.segments()
        .last()
        .map(|s| s.as_str().to_string())
        .unwrap_or_default()
}

/// Placeholder span — real line/column mapping will be added later
/// using solar's SourceMap integration.
fn make_span(file_index: usize) -> SourceSpan {
    SourceSpan {
        file_index,
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 0,
    }
}
