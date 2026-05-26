pub mod error;
pub mod solar_frontend;
pub mod solc_frontend;
pub mod span;

use std::path::PathBuf;

use crate::model::project::Project;
use error::ParseError;

/// Abstraction over the concrete parser implementation.
/// Enables swapping solar-parse for another backend (e.g. solang-parser)
/// without changing any downstream code.
pub trait ProjectParser {
    fn parse(&self, paths: &[PathBuf]) -> Result<Project, ParseError>;
}
