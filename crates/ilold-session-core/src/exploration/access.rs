use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessLevel {
    Public,
    Restricted { role: String },
    Internal,
    Special { kind: String },
}

impl fmt::Display for AccessLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AccessLevel::Public => write!(f, "Public"),
            AccessLevel::Restricted { role } => write!(f, "Restricted({})", role),
            AccessLevel::Internal => write!(f, "Internal"),
            AccessLevel::Special { kind } => write!(f, "Special({})", kind),
        }
    }
}

impl AccessLevel {
    pub fn short_label(&self) -> &str {
        match self {
            AccessLevel::Public => "P",
            AccessLevel::Restricted { .. } => "R",
            AccessLevel::Internal => "I",
            AccessLevel::Special { .. } => "S",
        }
    }

    pub fn is_unrestricted(&self) -> bool {
        matches!(self, AccessLevel::Public)
    }
}
