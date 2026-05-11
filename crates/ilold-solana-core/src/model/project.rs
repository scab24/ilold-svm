use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::ProgramDef;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SolanaProject {
    pub programs: Vec<ProgramDef>,
    #[serde(skip)]
    pub program_index: HashMap<String, usize>,
}

impl SolanaProject {
    pub fn new(programs: Vec<ProgramDef>) -> Self {
        let mut me = Self {
            programs,
            program_index: HashMap::new(),
        };
        me.rebuild_index();
        me
    }

    pub fn rebuild_index(&mut self) {
        self.program_index.clear();
        for (idx, program) in self.programs.iter().enumerate() {
            self.program_index.insert(program.name.clone(), idx);
        }
    }

    pub fn find_program(&self, name: &str) -> Option<&ProgramDef> {
        self.program_index.get(name).and_then(|i| self.programs.get(*i))
    }
}
