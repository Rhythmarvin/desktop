// Wired into the run engine composition in the endpoints stage; until then only tests reference it.
#![allow(dead_code)]

use crate::task::resolve_task_cwd;
use ora_application::{
    AgentDefinitionRepository, ExecutionContext, FilesystemSkillStorage, NodeType, SkillStorage,
    StartPrerequisitesError, WorkflowGraph, WorkflowRunStartPrerequisites,
};
use ora_db::{RepositoryPool, SqliteAgentDefinitionRepository};
use ora_skill_package::{parse_manifest, rewrite_manifest};
use std::path::{Path, PathBuf};

/// Upper bound for a SKILL.md manifest read during materialization.
const MAX_SKILL_MANIFEST_BYTES: u64 = 1024 * 1024;

/// Validates start-time skill and role prerequisites and materializes skills into the worktree.
///
/// Skills and roles are launch hard-dependencies: every enabled skill must exist in the catalog
/// and every agent's role must resolve in the agents catalog. Enabled skills are copied into
/// `<worktree>/.claude/skills/<normalized>/`, where CLI tooling auto-discovers them.
#[derive(Clone)]
pub struct SkillRoleMaterializer {
    skills_root: PathBuf,
    pool: RepositoryPool,
}

impl SkillRoleMaterializer {
    /// Builds a materializer from the skill catalog root and the shared repository pool.
    pub fn new(skills_root: PathBuf, pool: RepositoryPool) -> Self {
        Self { skills_root, pool }
    }
}

impl WorkflowRunStartPrerequisites for SkillRoleMaterializer {
    fn validate_and_materialize(
        &self,
        context: &ExecutionContext,
        graph: &WorkflowGraph,
    ) -> Result<(), StartPrerequisitesError> {
        let (skills, roles) = collect_requirements(graph);

        let agent_repository = SqliteAgentDefinitionRepository::new(self.pool.clone());
        for role_id in &roles {
            if agent_repository
                .find_agent_definition_by_name(role_id)?
                .is_none()
            {
                return Err(StartPrerequisitesError::WorkflowRoleNotFound {
                    role_id: role_id.clone(),
                });
            }
        }

        if !skills.is_empty() {
            let worktree_root =
                resolve_task_cwd(&self.pool, &context.task.id).map_err(|error| {
                    StartPrerequisitesError::SkillMaterializationError {
                        message: format!("failed to resolve run worktree: {error}"),
                    }
                })?;
            let storage = FilesystemSkillStorage::new(self.skills_root.clone());
            for skill_id in &skills {
                materialize_skill(&storage, &worktree_root, skill_id)?;
            }
        }
        Ok(())
    }
}

/// Collects the distinct enabled skill ids and role ids declared across all agent nodes.
fn collect_requirements(graph: &WorkflowGraph) -> (Vec<String>, Vec<String>) {
    let mut skills = Vec::new();
    let mut roles = Vec::new();
    for node in graph.nodes() {
        if node.node_type != NodeType::Agent {
            continue;
        }
        let Some(config) = &node.agent_config else {
            continue;
        };
        for skill in &config.skills {
            if skill.enabled && !skills.contains(&skill.skill_id) {
                skills.push(skill.skill_id.clone());
            }
        }
        if let Some(role_id) = &config.role_id
            && !roles.contains(role_id)
        {
            roles.push(role_id.clone());
        }
    }
    (skills, roles)
}

/// Resolves one enabled skill against the catalog and copies it into the worktree.
fn materialize_skill(
    storage: &FilesystemSkillStorage,
    worktree_root: &Path,
    skill_id: &str,
) -> Result<(), StartPrerequisitesError> {
    // A namespaced id like `cdase:sfmea_review` resolves by the suffix after the colon.
    let catalog_name = skill_id.rsplit(':').next().unwrap_or(skill_id);
    if !storage.formal_exists(catalog_name) {
        return Err(StartPrerequisitesError::WorkflowSkillNotFound {
            skill_id: skill_id.to_string(),
        });
    }
    let dir_name = normalize_skill_name(catalog_name);
    let target = worktree_root.join(".claude").join("skills").join(&dir_name);
    storage
        .copy_package_to(catalog_name, &target)
        .map_err(|error| StartPrerequisitesError::SkillMaterializationError {
            message: error.to_string(),
        })?;
    rewrite_manifest_name(&target, &dir_name)
        .map_err(|message| StartPrerequisitesError::SkillMaterializationError { message })?;
    Ok(())
}

/// Normalizes a catalog name for the `.claude/skills/` directory: lowercase, `_` becomes `-`.
fn normalize_skill_name(name: &str) -> String {
    name.to_lowercase().replace('_', "-")
}

/// Rewrites the copied `SKILL.md` frontmatter `name` when it differs from the target directory.
fn rewrite_manifest_name(target: &Path, dir_name: &str) -> Result<(), String> {
    let manifest_path = target.join("SKILL.md");
    let bytes = std::fs::read(&manifest_path).map_err(|error| error.to_string())?;
    let manifest = parse_manifest(&bytes, MAX_SKILL_MANIFEST_BYTES)
        .map_err(|error| format!("invalid SKILL.md in {}: {error}", manifest_path.display()))?;
    if manifest.name == dir_name {
        return Ok(());
    }
    let rewritten = rewrite_manifest(&bytes, dir_name, &manifest.description)
        .map_err(|error| format!("failed to rewrite SKILL.md name: {error}"))?;
    std::fs::write(&manifest_path, rewritten).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    #[test]
    fn normalizes_skill_names_to_lowercase_dashes() {
        assert_eq!(normalize_skill_name("sfmea_review"), "sfmea-review");
        assert_eq!(normalize_skill_name("OpenSpec_Explore"), "openspec-explore");
    }

    #[test]
    fn materialize_skill_copies_the_package_and_rewrites_the_manifest() {
        let temp = TempDir::new().unwrap();
        let skills_root = temp.path().join("skills");
        let skill_dir = skills_root.join("sfmea_review");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: sfmea_review\ndescription: review\n---\n\nbody\n",
        )
        .unwrap();
        std::fs::write(skill_dir.join("notes.txt"), "payload").unwrap();
        let storage = FilesystemSkillStorage::new(skills_root);
        let worktree = temp.path().join("worktree");
        std::fs::create_dir_all(&worktree).unwrap();

        materialize_skill(&storage, &worktree, "cdase:sfmea_review").unwrap();

        let target = worktree.join(".claude").join("skills").join("sfmea-review");
        assert!(target.join("notes.txt").exists());
        let manifest = parse_manifest(
            &std::fs::read(target.join("SKILL.md")).unwrap(),
            MAX_SKILL_MANIFEST_BYTES,
        )
        .unwrap();
        assert_eq!(manifest.name, "sfmea-review");
        assert_eq!(manifest.description, "review");
    }

    #[test]
    fn materialize_skill_reports_a_missing_skill() {
        let temp = TempDir::new().unwrap();
        let storage = FilesystemSkillStorage::new(temp.path().join("skills"));
        let worktree = temp.path().join("worktree");
        std::fs::create_dir_all(&worktree).unwrap();

        let error = materialize_skill(&storage, &worktree, "missing_skill").unwrap_err();
        assert!(matches!(
            error,
            StartPrerequisitesError::WorkflowSkillNotFound { skill_id }
                if skill_id == "missing_skill"
        ));
    }

    #[test]
    fn materialize_skill_is_idempotent_and_overwrites() {
        let temp = TempDir::new().unwrap();
        let skills_root = temp.path().join("skills");
        let skill_dir = skills_root.join("explore");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: explore\ndescription: explore\n---\n\nbody\n",
        )
        .unwrap();
        let storage = FilesystemSkillStorage::new(skills_root);
        let worktree = temp.path().join("worktree");
        std::fs::create_dir_all(&worktree).unwrap();

        materialize_skill(&storage, &worktree, "explore").unwrap();
        materialize_skill(&storage, &worktree, "explore").unwrap();
        assert!(
            worktree
                .join(".claude")
                .join("skills")
                .join("explore")
                .join("SKILL.md")
                .exists()
        );
    }
}
