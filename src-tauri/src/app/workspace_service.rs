use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::domain::git_url::web_url_from_remote;
use crate::domain::models::{
    default_shortcut, gen_id, now_secs, AppData, Config, Ide, Recent, Workspace,
};
use crate::domain::tree::build_app_data;
use crate::infra::config_repository::ConfigRepository;
use crate::infra::git::GitClient;
use crate::infra::ide_detector::{ide_name_from_path, IdeDetector};
use crate::infra::project_launcher::ProjectLauncher;

/// Facade over all workspace/IDE use-cases. Owns the in-memory config cache;
/// every mutation is persisted through the injected `ConfigRepository`.
pub struct WorkspaceService {
    config: Mutex<Config>,
    repo: Arc<dyn ConfigRepository>,
    detector: Arc<dyn IdeDetector>,
    launcher: Arc<dyn ProjectLauncher>,
    git: Arc<dyn GitClient>,
}

impl WorkspaceService {
    pub fn new(
        repo: Arc<dyn ConfigRepository>,
        detector: Arc<dyn IdeDetector>,
        launcher: Arc<dyn ProjectLauncher>,
        git: Arc<dyn GitClient>,
    ) -> Self {
        let config = repo.load();
        Self {
            config: Mutex::new(config),
            repo,
            detector,
            launcher,
            git,
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

    /// Browsable https URL of the project's `origin` remote.
    pub fn repo_web_url(&self, project_path: &str) -> Result<String, String> {
        let remote = self
            .git
            .remote_url(project_path)
            .ok_or("No git remote (origin) found")?;
        let aliases = self.git.ssh_aliases();
        web_url_from_remote(&remote, &aliases)
            .ok_or_else(|| format!("Can't build a web URL from remote: {remote}"))
    }

    /// Fast-start: create (or reuse) a branch from the chosen base and open
    /// the project in its IDE. The base resolution order is: explicit `base`
    /// → the project's remembered base → the global default. An explicit
    /// choice is remembered for the project.
    pub fn fast_start(
        &self,
        project_path: String,
        branch: String,
        base: Option<String>,
    ) -> Result<(), String> {
        let branch = branch.trim().to_string();
        if branch.is_empty() {
            return Err("Branch name is empty".into());
        }
        if !Path::new(&project_path).exists() {
            return Err(format!("Project not found: {project_path}"));
        }

        let base = {
            let mut config = self.config.lock().unwrap();
            let resolved = base
                .filter(|b| !b.trim().is_empty())
                .unwrap_or_else(|| {
                    config
                        .project_base_branches
                        .get(&project_path)
                        .cloned()
                        .unwrap_or_else(|| config.default_base_branch.clone())
                });
            // Remember the per-project choice; drop the override when it
            // matches the global default again.
            let changed = config.project_base_branches.get(&project_path)
                != Some(&resolved);
            if changed {
                if resolved == config.default_base_branch {
                    config.project_base_branches.remove(&project_path);
                } else {
                    config
                        .project_base_branches
                        .insert(project_path.clone(), resolved.clone());
                }
                let _ = self.repo.save(&config);
            }
            resolved
        };

        if self.git.branch_exists(&project_path, &branch) {
            // Idempotent: re-running fast-start just returns to the task.
            self.git.checkout(&project_path, &branch)?;
        } else {
            self.git.create_branch_from(&project_path, &branch, &base)?;
        }

        self.open_project(project_path, None)
    }

    pub fn add_base_branch(&self, name: String) -> Result<AppData, String> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err("Branch name is empty".into());
        }
        let mut config = self.config.lock().unwrap();
        if !config.base_branches.contains(&name) {
            config.base_branches.push(name);
        }
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    pub fn remove_base_branch(&self, name: &str) -> Result<AppData, String> {
        let mut config = self.config.lock().unwrap();
        if config.base_branches.len() <= 1 {
            return Err("At least one base branch is required".into());
        }
        config.base_branches.retain(|b| b != name);
        if config.default_base_branch == name {
            config.default_base_branch =
                config.base_branches.first().cloned().unwrap_or_default();
        }
        config.project_base_branches.retain(|_, b| b != name);
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    pub fn set_default_base_branch(&self, name: String) -> Result<AppData, String> {
        let mut config = self.config.lock().unwrap();
        if !config.base_branches.contains(&name) {
            return Err("Unknown base branch".into());
        }
        config.default_base_branch = name;
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    /// MR/PR list URL for the project's current branch ("open last MR").
    pub fn last_mr_url(&self, project_path: &str) -> Result<String, String> {
        let web = self.repo_web_url(project_path)?;
        let branch = self.git.current_branch(project_path);
        Ok(crate::domain::git_url::mr_list_url(&web, branch.as_deref()))
    }

    /// Pin/unpin a project. Pinning past the section limit is rejected.
    pub fn toggle_pin(&self, project_path: String) -> Result<AppData, String> {
        let mut config = self.config.lock().unwrap();
        if let Some(i) = config.pinned.iter().position(|p| p == &project_path) {
            config.pinned.remove(i);
        } else {
            if config.pinned.len() >= config.max_pinned as usize {
                return Err(format!("Pinned limit reached ({})", config.max_pinned));
            }
            config.pinned.push(project_path);
        }
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    /// Recent/Pinned section preferences (None = leave unchanged).
    pub fn set_section_prefs(
        &self,
        show_recent: Option<bool>,
        max_recent: Option<u32>,
        show_pinned: Option<bool>,
        max_pinned: Option<u32>,
    ) -> Result<AppData, String> {
        let mut config = self.config.lock().unwrap();
        if let Some(v) = show_recent {
            config.show_recent = v;
        }
        if let Some(v) = max_recent {
            config.max_recent = v.clamp(1, 20);
        }
        if let Some(v) = show_pinned {
            config.show_pinned = v;
        }
        if let Some(v) = max_pinned {
            config.max_pinned = v.clamp(1, 20);
        }
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    /// Window mode preferences (None = leave unchanged). Switching float off
    /// drops the remembered position — pinned mode anchors under the tray.
    pub fn set_window_prefs(
        &self,
        float: Option<bool>,
        fixed: Option<bool>,
        fixed_size: Option<(u32, u32)>,
    ) -> Result<AppData, String> {
        let mut config = self.config.lock().unwrap();
        if let Some(v) = float {
            config.window_float = v;
            if !v {
                config.window_pos = None;
            }
        }
        if let Some(v) = fixed {
            config.window_fixed = v;
        }
        if let Some((w, h)) = fixed_size {
            config.fixed_size = (w.clamp(280, 1200), h.clamp(360, 1400));
        }
        self.repo.save(&config)?;
        Ok(build_app_data(&config))
    }

    /// In-memory notes from window move/resize events; persisted on hide.
    pub fn remember_window_pos(&self, x: i32, y: i32) {
        let mut config = self.config.lock().unwrap();
        if config.window_float {
            config.window_pos = Some((x, y));
        }
    }

    pub fn remember_window_size(&self, w: u32, h: u32) {
        let mut config = self.config.lock().unwrap();
        if !config.window_fixed {
            config.window_size = Some((w, h));
        }
    }

    /// Persist the current in-memory config (used on window hide so frequent
    /// move/resize events don't hammer the disk).
    pub fn persist(&self) {
        let config = self.config.lock().unwrap();
        let _ = self.repo.save(&config);
    }

    /// Window geometry for the show path: (float, saved_pos, fixed, size).
    pub fn window_prefs(&self) -> (bool, Option<(i32, i32)>, bool, (u32, u32)) {
        let config = self.config.lock().unwrap();
        let size = if config.window_fixed {
            config.fixed_size
        } else {
            config.window_size.unwrap_or(crate::domain::models::default_fixed_size())
        };
        (config.window_float, config.window_pos, config.window_fixed, size)
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

    #[derive(Default)]
    struct FakeGitClient {
        remote: Option<String>,
        existing_branches: Vec<String>,
        ops: Mutex<Vec<String>>,
    }

    impl crate::infra::git::GitClient for FakeGitClient {
        fn remote_url(&self, _p: &str) -> Option<String> {
            self.remote.clone()
        }
        fn ssh_aliases(&self) -> HashMap<String, String> {
            HashMap::from([("gitlab-crypto".to_string(), "gitlab.com".to_string())])
        }
        fn current_branch(&self, _p: &str) -> Option<String> {
            Some("HP-432".to_string())
        }
        fn branch_exists(&self, _p: &str, branch: &str) -> bool {
            self.existing_branches.iter().any(|b| b == branch)
        }
        fn checkout(&self, _p: &str, branch: &str) -> Result<(), String> {
            self.ops.lock().unwrap().push(format!("checkout {branch}"));
            Ok(())
        }
        fn create_branch_from(&self, _p: &str, branch: &str, base: &str) -> Result<(), String> {
            self.ops
                .lock()
                .unwrap()
                .push(format!("create {branch} from {base}"));
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
            Arc::new(FakeGitClient::default()),
        );
        (service, repo)
    }

    fn service_with_git(
        detected: Vec<Ide>,
        git: FakeGitClient,
    ) -> (WorkspaceService, Arc<FakeConfigRepository>, Arc<FakeGitClient>) {
        let repo = Arc::new(FakeConfigRepository::new());
        let git = Arc::new(git);
        let service = WorkspaceService::new(
            repo.clone(),
            Arc::new(FakeDetector { ides: detected }),
            Arc::new(FakeLauncher::new()),
            git.clone(),
        );
        (service, repo, git)
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
            Arc::new(FakeGitClient::default()),
        );
        (service, repo, launcher)
    }

    #[test]
    fn fast_start_creates_branch_from_default_base() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().to_string_lossy().to_string();
        let (service, _, git) =
            service_with_git(vec![ide("a", "PhpStorm", "/apps/PhpStorm.app")], FakeGitClient::default());
        service.detect_ides().unwrap();

        service.fast_start(project, "HP-432".into(), None).unwrap();
        assert_eq!(
            git.ops.lock().unwrap().as_slice(),
            ["create HP-432 from master"]
        );
    }

    #[test]
    fn fast_start_reuses_existing_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().to_string_lossy().to_string();
        let git = FakeGitClient {
            existing_branches: vec!["HP-432".into()],
            ..Default::default()
        };
        let (service, _, git) =
            service_with_git(vec![ide("a", "PhpStorm", "/apps/PhpStorm.app")], git);
        service.detect_ides().unwrap();

        service.fast_start(project, "HP-432".into(), None).unwrap();
        assert_eq!(git.ops.lock().unwrap().as_slice(), ["checkout HP-432"]);
    }

    #[test]
    fn fast_start_remembers_project_base() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().to_string_lossy().to_string();
        let (service, repo, git) =
            service_with_git(vec![ide("a", "PhpStorm", "/apps/PhpStorm.app")], FakeGitClient::default());
        service.detect_ides().unwrap();
        service.add_base_branch("develop".into()).unwrap();

        service
            .fast_start(project.clone(), "HP-1".into(), Some("develop".into()))
            .unwrap();
        assert_eq!(
            repo.load().project_base_branches.get(&project).map(String::as_str),
            Some("develop")
        );

        // Next fast-start without an explicit base uses the remembered one.
        service.fast_start(project, "HP-2".into(), None).unwrap();
        assert_eq!(
            git.ops.lock().unwrap().last().map(String::as_str),
            Some("create HP-2 from develop")
        );
    }

    #[test]
    fn fast_start_rejects_empty_branch() {
        let (service, _) = service_with(vec![]);
        assert!(service.fast_start("/p".into(), "  ".into(), None).is_err());
    }

    #[test]
    fn base_branch_settings_flow() {
        let (service, _) = service_with(vec![]);
        let data = service.add_base_branch("develop".into()).unwrap();
        assert_eq!(data.base_branches, ["master", "develop"]);

        let data = service.set_default_base_branch("develop".into()).unwrap();
        assert_eq!(data.default_base_branch, "develop");

        let data = service.remove_base_branch("develop").unwrap();
        assert_eq!(data.base_branches, ["master"]);
        assert_eq!(data.default_base_branch, "master");

        assert!(service.remove_base_branch("master").is_err(), "last one stays");
        assert!(service.set_default_base_branch("nope".into()).is_err());
    }

    #[test]
    fn repo_web_url_resolves_alias() {
        let git = FakeGitClient {
            remote: Some("git@gitlab-crypto:group/repo.git".into()),
            ..Default::default()
        };
        let (service, _, _) = service_with_git(vec![], git);
        assert_eq!(
            service.repo_web_url("/p").unwrap(),
            "https://gitlab.com/group/repo"
        );
    }

    #[test]
    fn repo_web_url_errors_without_remote() {
        let (service, _) = service_with(vec![]);
        assert!(service.repo_web_url("/p").is_err());
    }

    #[test]
    fn last_mr_url_uses_current_branch() {
        let git = FakeGitClient {
            remote: Some("git@gitlab-crypto:group/repo.git".into()),
            ..Default::default()
        };
        let (service, _, _) = service_with_git(vec![], git);
        assert_eq!(
            service.last_mr_url("/p").unwrap(),
            "https://gitlab.com/group/repo/-/merge_requests?scope=all&state=all&source_branch=HP-432"
        );
    }

    #[test]
    fn pin_toggle_respects_limit() {
        let (service, repo) = service_with(vec![]);
        service.toggle_pin("/a".into()).unwrap();
        service.toggle_pin("/b".into()).unwrap();
        service.toggle_pin("/c".into()).unwrap();
        assert!(service.toggle_pin("/d".into()).is_err(), "default limit is 3");

        // Unpin frees a slot; raising the limit allows more.
        service.toggle_pin("/a".into()).unwrap();
        service.toggle_pin("/d".into()).unwrap();
        service.set_section_prefs(None, None, None, Some(4)).unwrap();
        let data = service.toggle_pin("/e".into()).unwrap();
        assert_eq!(data.pinned, ["/b", "/c", "/d", "/e"]);
        assert_eq!(repo.load().pinned.len(), 4);
    }

    #[test]
    fn window_prefs_flow() {
        let (service, repo) = service_with(vec![]);
        // Defaults: pinned, resizable, base size.
        assert_eq!(service.window_prefs(), (false, None, false, (380, 560)));

        service.set_window_prefs(Some(true), None, None).unwrap();
        service.remember_window_pos(100, 200);
        service.remember_window_size(500, 700);
        service.persist();
        assert_eq!(service.window_prefs(), (true, Some((100, 200)), false, (500, 700)));
        assert_eq!(repo.load().window_pos, Some((100, 200)));

        // Fixed mode uses fixed_size; leaving float drops the position.
        service
            .set_window_prefs(Some(false), Some(true), Some((400, 600)))
            .unwrap();
        assert_eq!(service.window_prefs(), (false, None, true, (400, 600)));
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
