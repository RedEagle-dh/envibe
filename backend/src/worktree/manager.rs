use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::fs;
use uuid::Uuid;

use crate::error::{Error, Result};

/// A git worktree snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub name: String,
    pub branch: String,
    pub path: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

/// Result of a merge operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    pub success: bool,
    pub message: String,
    #[serde(rename = "hasConflicts")]
    pub has_conflicts: bool,
    #[serde(rename = "conflictFiles")]
    pub conflict_files: Vec<String>,
    #[serde(rename = "commitHash")]
    pub commit_hash: Option<String>,
}

/// Options for merge operation
#[derive(Debug, Clone, Deserialize)]
pub struct MergeOptions {
    #[serde(rename = "deleteAfterMerge")]
    pub delete_after_merge: bool,
    #[serde(rename = "commitMessage")]
    pub commit_message: Option<String>,
}

/// Manages git worktrees for project snapshots
#[derive(Debug, Default)]
pub struct WorktreeManager {
    /// Maps project name -> snapshot id -> Snapshot
    snapshots: HashMap<String, HashMap<String, Snapshot>>,
}

impl WorktreeManager {
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
        }
    }

    /// Load snapshots from disk
    pub async fn load(data_dir: &PathBuf) -> Result<Self> {
        let path = data_dir.join("snapshots.json");
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(&path).await?;
        let snapshots: HashMap<String, HashMap<String, Snapshot>> = serde_json::from_str(&content)?;
        Ok(Self { snapshots })
    }

    /// Save snapshots to disk
    pub async fn save(&self, data_dir: &PathBuf) -> Result<()> {
        let path = data_dir.join("snapshots.json");
        let content = serde_json::to_string_pretty(&self.snapshots)?;
        fs::write(&path, content).await?;
        Ok(())
    }

    /// Get all snapshots for a project
    pub fn get_snapshots(&self, project_name: &str) -> Vec<Snapshot> {
        self.snapshots
            .get(project_name)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get a specific snapshot
    pub fn get_snapshot(&self, project_name: &str, snapshot_id: &str) -> Option<&Snapshot> {
        self.snapshots
            .get(project_name)
            .and_then(|m| m.get(snapshot_id))
    }

    /// Create a new worktree snapshot from the current HEAD
    pub fn create_snapshot(
        &mut self,
        project_name: &str,
        project_path: &PathBuf,
        snapshot_name: &str,
    ) -> Result<Snapshot> {
        // Validate that the project path is a git repository
        if !project_path.join(".git").exists() {
            return Err(Error::Git(format!(
                "Project '{}' is not a git repository",
                project_name
            )));
        }

        // Sanitize the snapshot name for use as branch name
        let safe_name = snapshot_name
            .replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "-")
            .to_lowercase();

        // Generate unique ID
        let id = Uuid::new_v4().to_string();

        // Create worktree path in .worktrees directory
        let worktrees_dir = project_path.join(".worktrees");
        let worktree_path = worktrees_dir.join(&safe_name);

        // Create a unique branch name
        let branch_name = format!("snapshot/{}", safe_name);

        // Check if worktree already exists
        if worktree_path.exists() {
            return Err(Error::Git(format!(
                "Worktree '{}' already exists",
                safe_name
            )));
        }

        // Ensure .worktrees directory exists
        std::fs::create_dir_all(&worktrees_dir)?;

        // Get current branch/HEAD
        let current_ref = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(project_path)
            .output()?;

        if !current_ref.status.success() {
            return Err(Error::Git("Failed to get current HEAD".to_string()));
        }

        let head_ref = String::from_utf8_lossy(&current_ref.stdout).trim().to_string();

        // Create new branch from HEAD if it doesn't exist
        let branch_exists = Command::new("git")
            .args(["rev-parse", "--verify", &branch_name])
            .current_dir(project_path)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !branch_exists {
            let create_branch = Command::new("git")
                .args(["branch", &branch_name, &head_ref])
                .current_dir(project_path)
                .output()?;

            if !create_branch.status.success() {
                let err = String::from_utf8_lossy(&create_branch.stderr);
                return Err(Error::Git(format!("Failed to create branch: {}", err)));
            }
        }

        // Create the worktree
        let output = Command::new("git")
            .args(["worktree", "add", worktree_path.to_str().unwrap(), &branch_name])
            .current_dir(project_path)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Git(format!("Failed to create worktree: {}", err)));
        }

        let snapshot = Snapshot {
            id: id.clone(),
            name: snapshot_name.to_string(),
            branch: branch_name,
            path: worktree_path.to_string_lossy().to_string(),
            created_at: Utc::now().to_rfc3339(),
        };

        // Store the snapshot
        self.snapshots
            .entry(project_name.to_string())
            .or_default()
            .insert(id, snapshot.clone());

        tracing::info!(
            "Created worktree snapshot '{}' at {}",
            snapshot_name,
            worktree_path.display()
        );

        Ok(snapshot)
    }

    /// Delete a worktree snapshot
    pub fn delete_snapshot(
        &mut self,
        project_name: &str,
        project_path: &PathBuf,
        snapshot_id: &str,
    ) -> Result<()> {
        // Get the snapshot
        let snapshot = self
            .get_snapshot(project_name, snapshot_id)
            .ok_or_else(|| Error::NotFound(format!("Snapshot {} not found", snapshot_id)))?
            .clone();

        let worktree_path = PathBuf::from(&snapshot.path);

        // Remove the worktree using git
        let output = Command::new("git")
            .args(["worktree", "remove", "--force", worktree_path.to_str().unwrap()])
            .current_dir(project_path)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            // If worktree doesn't exist, that's fine - just remove from our registry
            if !err.contains("is not a working tree") {
                tracing::warn!("Failed to remove worktree: {}", err);
            }
        }

        // Optionally delete the branch (only if it was created by us)
        if snapshot.branch.starts_with("snapshot/") {
            let _ = Command::new("git")
                .args(["branch", "-D", &snapshot.branch])
                .current_dir(project_path)
                .output();
        }

        // Remove from our registry
        if let Some(project_snapshots) = self.snapshots.get_mut(project_name) {
            project_snapshots.remove(snapshot_id);
        }

        tracing::info!("Deleted worktree snapshot '{}'", snapshot.name);

        Ok(())
    }

    /// Prune orphaned worktree references
    pub fn prune_worktrees(&self, project_path: &PathBuf) -> Result<()> {
        let output = Command::new("git")
            .args(["worktree", "prune"])
            .current_dir(project_path)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Failed to prune worktrees: {}", err);
        }

        Ok(())
    }

    /// Check if a worktree has uncommitted changes
    fn has_uncommitted_changes(path: &PathBuf) -> Result<bool> {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(path)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Git(format!("Failed to check git status: {}", err)));
        }

        let status = String::from_utf8_lossy(&output.stdout);
        Ok(!status.trim().is_empty())
    }

    /// Get the current branch name for a worktree
    fn get_current_branch(path: &PathBuf) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .current_dir(path)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Git(format!("Failed to get current branch: {}", err)));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Get files with merge conflicts
    fn get_conflict_files(path: &PathBuf) -> Vec<String> {
        let output = Command::new("git")
            .args(["diff", "--name-only", "--diff-filter=U"])
            .current_dir(path)
            .output();

        match output {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Abort an in-progress merge
    fn abort_merge(path: &PathBuf) -> Result<()> {
        let output = Command::new("git")
            .args(["merge", "--abort"])
            .current_dir(path)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("Failed to abort merge: {}", err);
        }

        Ok(())
    }

    /// Get the commit hash of HEAD
    fn get_head_commit(path: &PathBuf) -> Option<String> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(path)
            .output()
            .ok()?;

        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            None
        }
    }

    /// Merge a snapshot branch into the main project branch
    pub fn merge_snapshot(
        &mut self,
        project_name: &str,
        project_path: &PathBuf,
        snapshot_id: &str,
        options: &MergeOptions,
    ) -> Result<MergeResult> {
        // Get the snapshot
        let snapshot = self
            .get_snapshot(project_name, snapshot_id)
            .ok_or_else(|| Error::NotFound(format!("Snapshot {} not found", snapshot_id)))?
            .clone();

        let snapshot_path = PathBuf::from(&snapshot.path);

        // Check for uncommitted changes in main worktree
        if Self::has_uncommitted_changes(project_path)? {
            return Ok(MergeResult {
                success: false,
                message: "Main worktree has uncommitted changes. Please commit or stash them first.".to_string(),
                has_conflicts: false,
                conflict_files: Vec::new(),
                commit_hash: None,
            });
        }

        // Check for uncommitted changes in snapshot worktree
        if Self::has_uncommitted_changes(&snapshot_path)? {
            return Ok(MergeResult {
                success: false,
                message: "Snapshot has uncommitted changes. Please commit or stash them first.".to_string(),
                has_conflicts: false,
                conflict_files: Vec::new(),
                commit_hash: None,
            });
        }

        // Build the merge command
        let commit_message = options.commit_message.clone().unwrap_or_else(|| {
            format!("Merge snapshot '{}' into main branch", snapshot.name)
        });

        tracing::info!(
            "Merging snapshot '{}' (branch: {}) into main project at {}",
            snapshot.name,
            snapshot.branch,
            project_path.display()
        );

        // Perform the merge
        let output = Command::new("git")
            .args(["merge", &snapshot.branch, "-m", &commit_message])
            .current_dir(project_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Check if this is a merge conflict
            if stderr.contains("CONFLICT") || stderr.contains("Automatic merge failed") {
                let conflict_files = Self::get_conflict_files(project_path);

                // Abort the merge to leave the working directory clean
                Self::abort_merge(project_path)?;

                return Ok(MergeResult {
                    success: false,
                    message: "Merge conflicts detected. Merge has been aborted.".to_string(),
                    has_conflicts: true,
                    conflict_files,
                    commit_hash: None,
                });
            }

            // Other merge error
            return Ok(MergeResult {
                success: false,
                message: format!("Merge failed: {}", stderr.trim()),
                has_conflicts: false,
                conflict_files: Vec::new(),
                commit_hash: None,
            });
        }

        // Get the new commit hash
        let commit_hash = Self::get_head_commit(project_path);

        tracing::info!(
            "Successfully merged snapshot '{}' into main branch (commit: {:?})",
            snapshot.name,
            commit_hash
        );

        // Optionally delete the snapshot after successful merge
        if options.delete_after_merge {
            if let Err(e) = self.delete_snapshot(project_name, project_path, snapshot_id) {
                tracing::warn!("Failed to delete snapshot after merge: {}", e);
                return Ok(MergeResult {
                    success: true,
                    message: format!(
                        "Merge successful, but failed to delete snapshot: {}",
                        e
                    ),
                    has_conflicts: false,
                    conflict_files: Vec::new(),
                    commit_hash,
                });
            }
        }

        Ok(MergeResult {
            success: true,
            message: if options.delete_after_merge {
                format!("Successfully merged and deleted snapshot '{}'", snapshot.name)
            } else {
                format!("Successfully merged snapshot '{}'", snapshot.name)
            },
            has_conflicts: false,
            conflict_files: Vec::new(),
            commit_hash,
        })
    }
}
