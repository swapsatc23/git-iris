use crate::change_analyzer::{AnalyzedChange, ChangeAnalyzer};
use crate::config::Config;
use crate::context::{ChangeType, CommitContext, ProjectMetadata, RecentCommit, StagedFile};
use crate::file_analyzers;
use anyhow::{anyhow, Result};
use git2::{DiffOptions, Repository, StatusOptions};
use regex::Regex;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn get_git_info(repo_path: &Path, _config: &Config) -> Result<CommitContext> {
    let repo = Repository::open(repo_path)?;

    let branch = get_current_branch(&repo)?;
    let recent_commits = get_recent_commits(&repo, 5)?;
    let (staged_files, unstaged_files) = get_file_statuses(&repo)?;
    let project_metadata = get_project_metadata(repo_path)?;

    let context = CommitContext::new(
        branch,
        recent_commits,
        staged_files,
        unstaged_files,
        project_metadata,
    );

    Ok(context)
}

fn get_current_branch(repo: &Repository) -> Result<String> {
    let head = repo.head()?;
    Ok(head.shorthand().unwrap_or("HEAD detached").to_string())
}

fn get_recent_commits(repo: &Repository, count: usize) -> Result<Vec<RecentCommit>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    let commits = revwalk
        .take(count)
        .map(|oid| {
            let oid = oid?;
            let commit = repo.find_commit(oid)?;
            let author = commit.author();
            Ok(RecentCommit {
                hash: oid.to_string(),
                message: commit.message().unwrap_or_default().to_string(),
                author: author.name().unwrap_or_default().to_string(),
                timestamp: commit.time().seconds().to_string(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(commits)
}

pub fn get_commits_between(repo_path: &Path, from: &str, to: &str) -> Result<Vec<AnalyzedChange>> {
    let repo = Repository::open(repo_path)?;
    let analyzer = ChangeAnalyzer::new(&repo);

    let from_commit = repo.revparse_single(from)?.peel_to_commit()?;
    let to_commit = repo.revparse_single(to)?.peel_to_commit()?;

    let mut revwalk = repo.revwalk()?;
    revwalk.push(to_commit.id())?;
    revwalk.hide(from_commit.id())?;

    let analyzed_commits = revwalk
        .filter_map(|id| id.ok())
        .filter_map(|id| repo.find_commit(id).ok())
        .filter_map(|commit| analyzer.analyze_commit(&commit).ok())
        .collect();

    Ok(analyzed_commits)
}

fn should_exclude_file(path: &str) -> bool {
    let exclude_patterns = vec![
        String::from(r"\.git"),
        String::from(r"\.svn"),
        String::from(r"\.hg"),
        String::from(r"\.DS_Store"),
        String::from(r"node_modules"),
        String::from(r"target"),
        String::from(r"build"),
        String::from(r"dist"),
        String::from(r"\.vscode"),
        String::from(r"\.idea"),
        String::from(r"\.vs"),
        String::from(r"package-lock\.json"),
        String::from(r"\.lock"),
        String::from(r"\.log"),
        String::from(r"\.tmp"),
        String::from(r"\.temp"),
        String::from(r"\.swp"),
        String::from(r"\.min\.js"),
        // Add more patterns as needed
    ];

    for pattern in exclude_patterns {
        let re = Regex::new(&pattern).unwrap();
        if re.is_match(path) {
            return true;
        }
    }
    false
}

fn get_file_statuses(repo: &Repository) -> Result<(Vec<StagedFile>, Vec<String>)> {
    let mut staged_files = Vec::new();
    let mut unstaged_files = Vec::new();

    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.statuses(Some(&mut opts))?;

    for (_index, entry) in statuses.iter().enumerate() {
        let path = entry.path().unwrap();
        let status = entry.status();

        if status.is_index_new() || status.is_index_modified() || status.is_index_deleted() {
            let change_type = if status.is_index_new() {
                ChangeType::Added
            } else if status.is_index_modified() {
                ChangeType::Modified
            } else {
                ChangeType::Deleted
            };

            let should_exclude = should_exclude_file(path);
            let diff = if should_exclude {
                String::from("[Content excluded]")
            } else {
                get_diff_for_file(repo, path, true)?
            };

            let analyzer = file_analyzers::get_analyzer(path);
            let staged_file = StagedFile {
                path: path.to_string(),
                change_type: change_type.clone(),
                diff: diff.clone(),
                analysis: Vec::new(),
                content_excluded: should_exclude,
            };
            let analysis = if should_exclude {
                vec!["[Analysis excluded]".to_string()]
            } else {
                analyzer.analyze(path, &staged_file)
            };

            staged_files.push(StagedFile {
                path: path.to_string(),
                change_type,
                diff,
                analysis,
                content_excluded: should_exclude,
            });
        } else if status.is_wt_modified() || status.is_wt_new() || status.is_wt_deleted() {
            unstaged_files.push(path.to_string());
        }
    }

    Ok((staged_files, unstaged_files))
}

fn get_diff_for_file(repo: &Repository, path: &str, staged: bool) -> Result<String> {
    let mut diff_options = DiffOptions::new();
    diff_options.pathspec(path);

    let tree = if staged {
        Some(repo.head()?.peel_to_tree()?)
    } else {
        None
    };

    let diff = repo.diff_tree_to_workdir_with_index(tree.as_ref(), Some(&mut diff_options))?;

    let mut diff_string = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = match line.origin() {
            '+' | '-' | ' ' => line.origin(),
            _ => ' ',
        };
        diff_string.push(origin);
        diff_string.push_str(&String::from_utf8_lossy(line.content()));
        true
    })?;

    if is_binary_diff(&diff_string) {
        Ok("[Binary file changed]".to_string())
    } else {
        Ok(diff_string)
    }
}

fn is_binary_diff(diff: &str) -> bool {
    diff.contains("Binary files") || diff.contains("GIT binary patch")
}

fn get_project_metadata(repo_path: &Path) -> Result<ProjectMetadata> {
    let mut combined_metadata = ProjectMetadata::default();

    for entry in WalkDir::new(repo_path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let file_path = entry.path();
            let file_name = file_path.file_name().unwrap().to_str().unwrap();
            let analyzer = file_analyzers::get_analyzer(file_name);

            if let Ok(content) = std::fs::read_to_string(file_path) {
                let metadata = analyzer.extract_metadata(file_name, &content);
                merge_metadata(&mut combined_metadata, metadata);
            }
        }
    }

    Ok(combined_metadata)
}

fn merge_metadata(combined: &mut ProjectMetadata, new: ProjectMetadata) {
    if combined.language.is_none() {
        combined.language = new.language;
    }
    if combined.framework.is_none() {
        combined.framework = new.framework;
    }
    if combined.version.is_none() {
        combined.version = new.version;
    }
    if combined.build_system.is_none() {
        combined.build_system = new.build_system;
    }
    if combined.test_framework.is_none() {
        combined.test_framework = new.test_framework;
    }
    combined.dependencies.extend(new.dependencies);
    combined.dependencies.sort();
    combined.dependencies.dedup();
}

pub fn check_environment() -> Result<()> {
    if std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_err()
    {
        return Err(anyhow!("Git is not installed or not in the PATH"));
    }

    Ok(())
}

pub fn is_inside_work_tree() -> Result<bool> {
    match Repository::discover(".") {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

pub fn commit(repo_path: &Path, message: &str) -> Result<()> {
    let repo = Repository::open(repo_path)?;
    let signature = repo.signature()?;
    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let head = repo.head()?;
    let parent_commit = head.peel_to_commit()?;

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &[&parent_commit],
    )?;
    Ok(())
}

pub fn find_and_read_readme(repo_path: &Path) -> Result<Option<String>> {
    let readme_patterns = ["README.md", "README.txt", "README", "Readme.md"];

    for pattern in readme_patterns.iter() {
        let readme_path = repo_path.join(pattern);
        if readme_path.exists() {
            let content = fs::read_to_string(readme_path)?;
            return Ok(Some(content));
        }
    }

    Ok(None)
}
