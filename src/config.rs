use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AgendaConfig {
    pub vault_dir: PathBuf,
    pub default_file: PathBuf,
}

impl AgendaConfig {
    pub fn new(vault_dir: PathBuf) -> Self {
        let default_file = vault_dir.join("agenda.md");
        AgendaConfig {
            vault_dir,
            default_file,
        }
    }

    pub fn default_file_string(&self) -> String {
        self.default_file.to_string_lossy().to_string()
    }

    pub fn vault_dir_string(&self) -> String {
        self.vault_dir.to_string_lossy().to_string()
    }
}
