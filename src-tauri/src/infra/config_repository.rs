use std::fs;
use std::path::PathBuf;

use crate::domain::models::Config;

/// Repository abstraction over config persistence. Services depend on this
/// trait, not on the storage format, so storage can change (and tests can use
/// an in-memory fake) without touching business logic.
pub trait ConfigRepository: Send + Sync {
    fn load(&self) -> Config;
    fn save(&self, config: &Config) -> Result<(), String>;
}

/// JSON-file implementation — same on-disk format and path as before.
pub struct JsonConfigRepository {
    path: PathBuf,
}

impl JsonConfigRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ConfigRepository for JsonConfigRepository {
    fn load(&self) -> Config {
        match fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Config::default(),
        }
    }

    fn save(&self, config: &Config) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
        fs::write(&self.path, text).map_err(|e| e.to_string())
    }
}
