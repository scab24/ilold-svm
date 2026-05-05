use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssignOperator {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    ShlAssign,
    ShrAssign,
}

impl AssignOperator {
    pub fn as_str(self) -> &'static str {
        match self {
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
}
