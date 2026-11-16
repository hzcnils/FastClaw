//! Local knowledge sources: inspect a drop batch and list readable files without
//! copying files, following directory links, or blocking the async runtime.
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Debug, Serialize)]
pub struct SourceSelection {
    path: String,
    name: String,
    #[serde(rename = "type")]
    entry_type: String,
}

#[derive(Debug, Serialize)]
pub struct PathIssue {
    path: String,
    message: String,
}

#[derive(Debug, Default, Serialize)]
pub struct DropInspection {
    selections: Vec<SourceSelection>,
    errors: Vec<PathIssue>,
}

fn display_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    if let Some(unc) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{}", unc);
    }
    raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string()
}

fn inspect(paths: Vec<String>) -> DropInspection {
    let mut result = DropInspection::default();
    let mut seen = HashSet::new();
    for raw in paths {
        let resolved = std::fs::canonicalize(&raw).and_then(|path| {
            let metadata = std::fs::metadata(&path)?;
            if !metadata.is_file() && !metadata.is_dir() {
                return Err(std::io::Error::other("Not a regular file or directory"));
            }
            Ok((path, metadata.is_dir()))
        });
        match resolved {
            Ok((path, directory)) => {
                if !seen.insert(path.clone()) {
                    continue;
                }
                result.selections.push(SourceSelection {
                    name: path
                        .file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                        .into_owned(),
                    path: display_path(&path),
                    entry_type: if directory { "directory" } else { "file" }.into(),
                });
            }
            Err(error) => result.errors.push(PathIssue {
                path: raw,
                message: error.to_string(),
            }),
        }
    }
    result
}

#[tauri::command]
pub async fn inspect_drop_paths(paths: Vec<String>) -> Result<DropInspection, String> {
    tokio::task::spawn_blocking(move || inspect(paths))
        .await
        .map_err(|e| e.to_string())
}

#[derive(Clone, Deserialize)]
pub struct LocalSource {
    path: String,
    #[serde(default)]
    recursive: bool,
}

#[derive(Default, Serialize)]
pub struct SourceFiles {
    files: Vec<String>,
    errors: Vec<PathIssue>,
    truncated: bool,
}

fn readable(path: &Path) -> bool {
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "txt"
            | "md"
            | "py"
            | "js"
            | "ts"
            | "vue"
            | "json"
            | "yaml"
            | "yml"
            | "xml"
            | "html"
            | "css"
            | "scss"
            | "rs"
            | "toml"
            | "cfg"
            | "ini"
            | "env"
            | "sh"
            | "bat"
            | "ps1"
            | "sql"
            | "java"
            | "go"
            | "c"
            | "h"
            | "cpp"
            | "hpp"
            | "cs"
            | "rb"
            | "php"
            | "swift"
            | "kt"
            | "r"
            | "lua"
            | "conf"
            | "log"
            | "tsx"
            | "jsx"
            | "less"
            | "sass"
            | "graphql"
            | "prisma"
            | "proto"
            | "cmake"
            | "svg"
            | "docx"
            | "xlsx"
    )
}

fn scan(sources: Vec<LocalSource>) -> SourceFiles {
    let mut result = SourceFiles::default();
    let mut seen = HashSet::new();
    let started = Instant::now();
    let mut visited = 0;
    for source in sources {
        let root = PathBuf::from(&source.path);
        if !root.exists() {
            result.errors.push(PathIssue {
                path: source.path,
                message: "Path is unavailable".into(),
            });
            continue;
        }
        if root.is_file() && !readable(&root) {
            result.errors.push(PathIssue {
                path: source.path,
                message: "Unsupported file format".into(),
            });
            continue;
        }
        let max_depth = if root.is_file() {
            0
        } else if source.recursive {
            32
        } else {
            1
        };
        for entry in walkdir::WalkDir::new(&root)
            .follow_links(false)
            .max_depth(max_depth)
        {
            visited += 1;
            if visited > 10_000 || started.elapsed() > Duration::from_secs(5) {
                result.truncated = true;
                return result;
            }
            match entry {
                Ok(entry) => {
                    if source.recursive && entry.depth() == 32 && entry.file_type().is_dir() {
                        result.truncated = true;
                    }
                    if entry.file_type().is_file() && readable(entry.path()) {
                        if let Ok(path) = std::fs::canonicalize(entry.path()) {
                            if seen.insert(path.clone()) {
                                result.files.push(display_path(&path));
                            }
                        }
                    }
                }
                Err(error) => result.errors.push(PathIssue {
                    path: error.path().unwrap_or(&root).to_string_lossy().into_owned(),
                    message: error.to_string(),
                }),
            }
        }
    }
    result.files.sort();
    result
}

#[tauri::command]
pub async fn list_local_source_files(sources: Vec<LocalSource>) -> Result<SourceFiles, String> {
    tokio::task::spawn_blocking(move || scan(sources))
        .await
        .map_err(|e| e.to_string())
}

/// Unlike open_directory, this operation never creates a missing directory.
#[tauri::command]
pub async fn open_local_source_folder(path: String) -> Result<(), String> {
    let resolved = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let directory = if resolved.is_dir() {
        resolved.as_path()
    } else {
        resolved.parent().ok_or("No parent directory")?
    };
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(directory)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_drop_is_one_batch_with_unique_sources_and_errors() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("资料.txt");
        std::fs::write(&file, "content").unwrap();
        let result = inspect(vec![
            file.display().to_string(),
            dir.path().display().to_string(),
            file.display().to_string(),
            dir.path().join("missing").display().to_string(),
        ]);
        assert_eq!(result.selections.len(), 2);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.selections[0].entry_type, "file");
        assert_eq!(result.selections[1].entry_type, "directory");
    }

    #[test]
    fn scan_respects_recursion_and_deduplicates_overlapping_sources() {
        let dir = tempfile::tempdir().unwrap();
        let child = dir.path().join("child");
        std::fs::create_dir(&child).unwrap();
        let file = dir.path().join("a.docx");
        std::fs::write(&file, "test").unwrap();
        std::fs::write(child.join("b.txt"), "test").unwrap();
        std::fs::write(child.join("c.exe"), "test").unwrap();
        let source = LocalSource {
            path: dir.path().display().to_string(),
            recursive: false,
        };
        assert_eq!(scan(vec![source.clone()]).files.len(), 1);
        let recursive = LocalSource {
            recursive: true,
            ..source
        };
        let result = scan(vec![
            recursive,
            LocalSource {
                path: file.display().to_string(),
                recursive: false,
            },
        ]);
        assert_eq!(result.files.len(), 2);
        assert!(result.errors.is_empty());
    }
}
