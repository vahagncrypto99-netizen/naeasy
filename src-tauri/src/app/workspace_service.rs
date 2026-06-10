use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::domain::models::{
    default_shortcut, gen_id, now_secs, AppData, Config, Ide, Recent, Workspace,
};
use crate::domain::tree::build_app_data;
use crate::infra::config_repository::ConfigRepository;
use crate::infra::ide_detector::{ide_name_from_path, IdeDetector};
use crate::infra::project_launcher::ProjectLauncher;

/// Facade over all workspace/IDE use-cases. Owns the in-memory config cache;
/// every mutation is persisted through the injected `ConfigRepository`.
pub struct WorkspaceService {
    config: Mutex<Config>,
    repo: Arc<dyn ConfigRepository>,
    detector: Arc<dyn IdeDetector>,
    launcher: Arc<dyn ProjectLauncher>,
}

impl WorkspaceService {
    pub fn new(
        repo: Arc<dyn ConfigRepository>,
        detector: Arc<dyn IdeDetector>,
        launcher: Arc<dyn ProjectLauncher>,
    ) -> Self {
        let config = repo.load();
        Self {
            config: Mutex::new(config),
            repo,
            detector,
            launcher,
        }
    }

    pub fn app_data(&self) -> AppData {
        let config = self.config.lock().unwrap();
        build_app_data(&config)
    }

    pub fn shortcut(&self) -> String {
        let config = self.config.lock().unwrap();
        config.shortcut.clone().unwrap_or_else(default_shortcut)
    }

    pub fn add_workspace(&self, path: String) -> Result<AppData, String> {
        let p = PathBuf::from(&path);
        if !p.is_dir() {
            return Err(format!("Not a directory: {path}"));
        }
        let mut config = self.config.lock().unwrap();
        if config.workspaces.iter().any(|w| w.path == path) {
            return Err("Workspace already added".into());
        }
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        config.workspaces.push(Workspace {
            id: gen_id(),
            name,
            path,
        });
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    pub fn remove_workspace(&self, id: &str) -> Result<AppData, String> {
        let mut config = self.config.lock().unwrap();
        config.workspaces.retain(|w| w.id != id);
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    pub fn detect_ides(&self) -> Result<AppData, String> {
        let detected = self.detector.detect();
        let mut config = self.config.lock().unwrap();
        for ide in detected {
            if !config.ides.iter().any(|i| i.path == ide.path) {
                config.ides.push(ide);
            }
        }
        if config.default_ide_id.is_none() {
            config.default_ide_id = config.ides.first().map(|i| i.id.clone());
        }
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    pub fn add_ide(&self, path: String) -> Result<AppData, String> {
        let p = PathBuf::from(&path);
        if !p.exists() {
            return Err(format!("Path does not exist: {path}"));
        }
        let mut config = self.config.lock().unwrap();
        if config.ides.iter().any(|i| i.path == path) {
            return Err("IDE already added".into());
        }
        let name = ide_name_from_path(&path);
        let ide = Ide {
            id: gen_id(),
            name,
            path,
        };
        let new_id = ide.id.clone();
        config.ides.push(ide);
        if config.default_ide_id.is_none() {
            config.default_ide_id = Some(new_id);
        }
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    pub fn remove_ide(&self, id: &str) -> Result<AppData, String> {
        let mut config = self.config.lock().unwrap();
        config.ides.retain(|i| i.id != id);
        if config.default_ide_id.as_deref() == Some(id) {
            config.default_ide_id = config.ides.first().map(|i| i.id.clone());
        }
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    pub fn set_default_ide(&self, id: String) -> Result<AppData, String> {
        let mut config = self.config.lock().unwrap();
        if !config.ides.iter().any(|i| i.id == id) {
            return Err("Unknown IDE".into());
        }
        config.default_ide_id = Some(id);
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    /// Open a project in the chosen IDE and remember it in "Recents".
    /// IDE resolution order: explicit `ide_id` → the project's remembered IDE
    /// → the default IDE. Focus-or-open is handled by the launcher.
    pub fn open_project(&self, project_path: String, ide_id: Option<String>) -> Result<(), String> {
        let mut config = self.config.lock().unwrap();
        let chosen = ide_id
            .or_else(|| config.project_ides.get(&project_path).cloned())
            .or_else(|| config.default_ide_id.clone());
        let ide = match chosen.and_then(|id| config.ides.iter().find(|i| i.id == id).cloned()) {
            Some(ide) => ide,
            None => return Err("No IDE selected. Add an IDE first.".into()),
        };

        if !Path::new(&project_path).exists() {
            return Err(format!("Project not found: {project_path}"));
        }

        self.launcher
            .open(&ide.path, &project_path, config.open_in_tabs)?;

        // Remember when and with which IDE this project was opened.
        config.recents.insert(
            project_path.clone(),
            Recent {
                last_opened: now_secs(),
                ide: ide.name.clone(),
            },
        );
        let _ = self.repo.save(&config);
        Ok(())
    }

    pub fn reveal(&self, path: &str) -> Result<(), String> {
        self.launcher.reveal(path)
    }

    pub fn scan_open(&self, names: &[String]) -> HashMap<String, String> {
        self.launcher.scan_open(names)
    }

    /// Remember (or clear, with `None`) the preferred IDE for one project.
    pub fn set_project_ide(
        &self,
        project_path: String,
        ide_id: Option<String>,
    ) -> Result<AppData, String> {
        let mut config = self.config.lock().unwrap();
        match ide_id {
            Some(id) => {
                if !config.ides.iter().any(|i| i.id == id) {
                    return Err("Unknown IDE".into());
                }
                config.project_ides.insert(project_path, id);
            }
            None => {
                config.project_ides.remove(&project_path);
            }
        }
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    /// Toggle tabs-vs-windows mode for opening projects.
    pub fn set_open_in_tabs(&self, enabled: bool) -> Result<AppData, String> {
        let mut config = self.config.lock().unwrap();
        config.open_in_tabs = enabled;
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    /// Persist a new shortcut accelerator (registration with the OS is the
    /// UI layer's job — it owns the Tauri app handle).
    pub fn set_shortcut(&self, accel: String) -> Result<AppData, String> {
        let mut config = self.config.lock().unwrap();
        config.shortcut = Some(accel);
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeConfigRepository {
        config: Mutex<Config>,
        fail_save: bool,
    }

    impl FakeConfigRepository {
        fn new() -> Self {
            Self {
                config: Mutex::new(Config::default()),
                fail_save: false,
            }
        }
    }

    impl ConfigRepository for FakeConfigRepository {
        fn load(&self) -> Config {
            self.config.lock().unwrap().clone()
        }
        fn save(&self, config: &Config) -> Result<(), String> {
            if self.fail_save {
                return Err("save failed".into());
            }
            *self.config.lock().unwrap() = config.clone();
            Ok(())
        }
    }

    struct FakeDetector {
        ides: Vec<Ide>,
    }

    impl IdeDetector for FakeDetector {
        fn detect(&self) -> Vec<Ide> {
            self.ides.clone()
        }
    }

    struct FakeLauncher {
        opened: Mutex<Vec<(String, String, bool)>>,
    }

    impl FakeLauncher {
        fn new() -> Self {
            Self {
                opened: Mutex::new(Vec::new()),
            }
        }
    }

    impl ProjectLauncher for FakeLauncher {
        fn open(&self, app_path: &str, project_path: &str, in_tabs: bool) -> Result<(), String> {
            self.opened
                .lock()
                .unwrap()
                .push((app_path.to_string(), project_path.to_string(), in_tabs));
            Ok(())
        }
        fn scan_open(&self, _names: &[String]) -> HashMap<String, String> {
            HashMap::new()
        }
        fn reveal(&self, _path: &str) -> Result<(), String> {
            Ok(())
        }
    }

    fn ide(id: &str, name: &str, path: &str) -> Ide {
        Ide {
            id: id.into(),
            name: name.into(),
            path: path.into(),
        }
    }

    fn service_with(detected: Vec<Ide>) -> (WorkspaceService, Arc<FakeConfigRepository>) {
        let repo = Arc::new(FakeConfigRepository::new());
        let service = WorkspaceService::new(
            repo.clone(),
            Arc::new(FakeDetector { ides: detected }),
            Arc::new(FakeLauncher::new()),
        );
        (service, repo)
    }

    #[test]
    fn add_workspace_persists_and_rejects_duplicates() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_string_lossy().to_string();
        let (service, repo) = service_with(vec![]);

        let data = service.add_workspace(path.clone()).unwrap();
        assert_eq!(data.workspaces.len(), 1);
        assert_eq!(repo.load().workspaces.len(), 1, "must be persisted");

        let err = service.add_workspace(path).unwrap_err();
        assert_eq!(err, "Workspace already added");
    }

    #[test]
    fn add_workspace_rejects_non_directory() {
        let (service, _) = service_with(vec![]);
        let err = service
            .add_workspace("/definitely/not/a/dir".into())
            .unwrap_err();
        assert!(err.starts_with("Not a directory"));
    }

    #[test]
    fn remove_workspace_deletes_by_id() {
        let tmp = tempfile::tempdir().unwrap();
        let (service, repo) = service_with(vec![]);
        let data = service
            .add_workspace(tmp.path().to_string_lossy().to_string())
            .unwrap();
        let id = data.workspaces[0].id.clone();

        let data = service.remove_workspace(&id).unwrap();
        assert!(data.workspaces.is_empty());
        assert!(repo.load().workspaces.is_empty());
    }

    #[test]
    fn detect_ides_merges_without_duplicates_and_sets_default() {
        let detected = vec![
            ide("a", "PhpStorm", "/apps/PhpStorm.app"),
            ide("b", "Zed", "/apps/Zed.app"),
        ];
        let (service, _) = service_with(detected);

        let data = service.detect_ides().unwrap();
        assert_eq!(data.ides.len(), 2);
        assert_eq!(data.default_ide_id.as_deref(), Some("a"));

        // Re-detecting must not duplicate (same paths).
        let data = service.detect_ides().unwrap();
        assert_eq!(data.ides.len(), 2);
    }

    #[test]
    fn remove_ide_reassigns_default() {
        let detected = vec![
            ide("a", "PhpStorm", "/apps/PhpStorm.app"),
            ide("b", "Zed", "/apps/Zed.app"),
        ];
        let (service, _) = service_with(detected);
        service.detect_ides().unwrap();

        let data = service.remove_ide("a").unwrap();
        assert_eq!(data.ides.len(), 1);
        assert_eq!(data.default_ide_id.as_deref(), Some("b"));
    }

    #[test]
    fn set_default_ide_rejects_unknown() {
        let (service, _) = service_with(vec![]);
        assert_eq!(service.set_default_ide("nope".into()).unwrap_err(), "Unknown IDE");
    }

    #[test]
    fn open_project_launches_and_records_recent() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().to_string_lossy().to_string();
        let (service, repo) = service_with(vec![ide("a", "PhpStorm", "/apps/PhpStorm.app")]);
        service.detect_ides().unwrap();

        service.open_project(project.clone(), None).unwrap();

        let config = repo.load();
        let recent = config.recents.get(&project).expect("recent recorded");
        assert_eq!(recent.ide, "PhpStorm");
        assert!(recent.last_opened > 0);
    }

    #[test]
    fn open_project_without_ide_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let (service, _) = service_with(vec![]);
        let err = service
            .open_project(tmp.path().to_string_lossy().to_string(), None)
            .unwrap_err();
        assert_eq!(err, "No IDE selected. Add an IDE first.");
    }

    #[test]
    fn set_shortcut_persists_accel() {
        let (service, repo) = service_with(vec![]);
        let data = service.set_shortcut("Alt+Space".into()).unwrap();
        assert_eq!(data.shortcut, "Alt+Space");
        assert_eq!(repo.load().shortcut.as_deref(), Some("Alt+Space"));
        assert_eq!(service.shortcut(), "Alt+Space");
    }

    fn service_with_launcher(
        detected: Vec<Ide>,
    ) -> (WorkspaceService, Arc<FakeConfigRepository>, Arc<FakeLauncher>) {
        let repo = Arc::new(FakeConfigRepository::new());
        let launcher = Arc::new(FakeLauncher::new());
        let service = WorkspaceService::new(
            repo.clone(),
            Arc::new(FakeDetector { ides: detected }),
            launcher.clone(),
        );
        (service, repo, launcher)
    }

    #[test]
    fn project_ide_overrides_default_and_clears_back() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().to_string_lossy().to_string();
        let (service, _, launcher) = service_with_launcher(vec![
            ide("a", "PhpStorm", "/apps/PhpStorm.app"),
            ide("b", "Zed", "/apps/Zed.app"),
        ]);
        service.detect_ides().unwrap(); // default = "a"

        let data = service
            .set_project_ide(project.clone(), Some("b".into()))
            .unwrap();
        assert_eq!(data.project_ides.get(&project).map(String::as_str), Some("b"));

        service.open_project(project.clone(), None).unwrap();
        assert_eq!(launcher.opened.lock().unwrap().last().unwrap().0, "/apps/Zed.app");

        // Clearing the override falls back to the default IDE.
        service.set_project_ide(project.clone(), None).unwrap();
        service.open_project(project.clone(), None).unwrap();
        assert_eq!(
            launcher.opened.lock().unwrap().last().unwrap().0,
            "/apps/PhpStorm.app"
        );
    }

    #[test]
    fn set_project_ide_rejects_unknown() {
        let (service, _) = service_with(vec![]);
        assert_eq!(
            service.set_project_ide("/p".into(), Some("nope".into())).unwrap_err(),
            "Unknown IDE"
        );
    }

    #[test]
    fn open_in_tabs_defaults_on_and_toggles() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().to_string_lossy().to_string();
        let (service, repo, launcher) =
            service_with_launcher(vec![ide("a", "PhpStorm", "/apps/PhpStorm.app")]);
        service.detect_ides().unwrap();

        service.open_project(project.clone(), None).unwrap();
        assert!(launcher.opened.lock().unwrap().last().unwrap().2, "tabs on by default");

        let data = service.set_open_in_tabs(false).unwrap();
        assert!(!data.open_in_tabs);
        assert!(!repo.load().open_in_tabs);

        service.open_project(project, None).unwrap();
        assert!(!launcher.opened.lock().unwrap().last().unwrap().2);
    }
}
