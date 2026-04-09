use std::collections::HashSet;
use std::error::Error;
use std::path::Path;
use std::process::Command;

use rayon::prelude::*;

use crate::config::AgendaConfig;

pub const TASK_STATES: &[&str] = &["TODO", "IN_PROGRESS", "DONE", "NEXT", "WAIT", "LATER"];

pub fn task_state_next(current: &str) -> String {
    if let Some(index) = TASK_STATES.iter().position(|&s| s == current) {
        let next_index = (index + 1) % TASK_STATES.len();
        TASK_STATES[next_index].to_string()
    } else {
        TASK_STATES[0].to_string()
    }
}

pub fn task_state_prev(current: &str) -> String {
    if let Some(index) = TASK_STATES.iter().position(|&s| s == current) {
        let prev_index = if index == 0 {
            TASK_STATES.len() - 1
        } else {
            index - 1
        };
        TASK_STATES[prev_index].to_string()
    } else {
        TASK_STATES[0].to_string()
    }
}

fn pattern_get(excluded: Option<&HashSet<String>>) -> String {
    TASK_STATES
        .iter()
        .filter(|state| {
            if let Some(ex) = excluded {
                !ex.contains(**state)
            } else {
                true
            }
        })
        .map(|tag| "#".to_string() + tag)
        .collect::<Vec<_>>()
        .join("|")
}

fn pattern_get_include(included: &HashSet<String>) -> String {
    TASK_STATES
        .iter()
        .filter(|state| included.contains(**state))
        .map(|tag| "#".to_string() + tag)
        .collect::<Vec<_>>()
        .join("|")
}

pub fn tasks_grep_include(
    config: &AgendaConfig,
    included: &HashSet<String>,
) -> Result<String, Box<dyn Error>> {
    let pattern = pattern_get_include(included);
    let vault_str = config.vault_dir_string();

    let output = Command::new("rg")
        .args(["-H", "-n", "--multiline", &pattern, "."])
        .current_dir(&vault_str)
        .output()?;

    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    let mut lines_by_file: std::collections::HashMap<String, Vec<(usize, String)>> =
        std::collections::HashMap::new();

    for line in lines {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() >= 2 {
            if let Some(filename) = parts.first() {
                let relative_name = if filename.starts_with("./") {
                    &filename[2..]
                } else {
                    filename
                };
                if let Some(line_nr_str) = parts.get(1) {
                    if let Ok(line_nr) = line_nr_str.parse::<usize>() {
                        let content = parts.get(2).map(|s| s.to_string());
                        lines_by_file
                            .entry(relative_name.to_string())
                            .or_default()
                            .push((line_nr, content.unwrap_or_default()));
                    }
                }
            }
        }
    }

    let mut filtered_lines: Vec<(usize, String)> = Vec::new();

    let vault_str = config.vault_dir_string();
    let tasks_by_file: Vec<(String, Vec<(usize, String)>)> = lines_by_file.into_iter().collect();

    let results: Vec<Vec<(usize, String)>> = tasks_by_file
        .par_iter()
        .map(|(filename, tasks)| {
            let full_path = Path::new(&vault_str).join(filename);
            let backlink_start_line = if let Ok(content) = std::fs::read_to_string(&full_path) {
                content
                    .lines()
                    .enumerate()
                    .find(|(_, l)| l.contains("<!-- BACKLINKS:START -->"))
                    .map(|(i, _)| i + 1)
            } else {
                None
            };

            tasks
                .iter()
                .filter_map(|(line_nr, content)| {
                    let include = match backlink_start_line {
                        Some(bl_start) => *line_nr < bl_start,
                        None => true,
                    };
                    if include {
                        Some((*line_nr, format!("{}:{}:{}", filename, line_nr, content)))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();

    for batch in results {
        filtered_lines.extend(batch);
    }

    filtered_lines.sort_by_key(|(line_nr, _)| *line_nr);

    let output = filtered_lines
        .into_iter()
        .map(|(_, s)| s)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(output)
}

pub fn tasks_grep_tag(
    config: &AgendaConfig,
    tag: &str,
    excluded: Option<&HashSet<String>>,
) -> Result<String, Box<dyn Error>> {
    let state_pattern = pattern_get(excluded);
    let tag_pattern = format!("#{}", tag);
    let combined_pattern = format!("{}|{}", state_pattern, tag_pattern);
    let vault_str = config.vault_dir_string();

    let output = Command::new("rg")
        .args(["-H", "-n", "--multiline", &combined_pattern, "."])
        .current_dir(&vault_str)
        .output()?;

    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    let mut lines_by_file: std::collections::HashMap<String, Vec<(usize, String)>> =
        std::collections::HashMap::new();

    for line in lines {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() >= 2 {
            if let Some(filename) = parts.first() {
                let relative_name = if filename.starts_with("./") {
                    &filename[2..]
                } else {
                    filename
                };
                if let Some(line_nr_str) = parts.get(1) {
                    if let Ok(line_nr) = line_nr_str.parse::<usize>() {
                        let content = parts.get(2).map(|s| s.to_string());
                        lines_by_file
                            .entry(relative_name.to_string())
                            .or_default()
                            .push((line_nr, content.unwrap_or_default()));
                    }
                }
            }
        }
    }

    let vault_str = config.vault_dir_string();
    let tasks_by_file: Vec<(String, Vec<(usize, String)>)> = lines_by_file.into_iter().collect();

    let results: Vec<Vec<(usize, String)>> = tasks_by_file
        .par_iter()
        .map(|(filename, tasks)| {
            let full_path = Path::new(&vault_str).join(filename);
            let (backlink_start_line, file_content) =
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    let bl_start = content
                        .lines()
                        .enumerate()
                        .find(|(_, l)| l.contains("<!-- BACKLINKS:START -->"))
                        .map(|(i, _)| i + 1);
                    (bl_start, Some(content))
                } else {
                    (None, None)
                };

            tasks
                .iter()
                .filter_map(|(line_nr, content)| {
                    let include = match backlink_start_line {
                        Some(bl_start) => *line_nr < bl_start,
                        None => true,
                    };
                    if !include {
                        return None;
                    }

                    let line_content = file_content
                        .as_ref()
                        .and_then(|c| c.lines().nth(*line_nr - 1));

                    if let Some(line) = line_content {
                        let has_tag = line.contains(&tag_pattern);

                        let has_valid_state = TASK_STATES
                            .iter()
                            .filter(|s| {
                                if let Some(ex) = excluded {
                                    !ex.contains(**s)
                                } else {
                                    true
                                }
                            })
                            .any(|s| line.contains(&format!("#{}", s)));

                        let has_excluded_state = TASK_STATES
                            .iter()
                            .filter(|s| {
                                if let Some(ex) = excluded {
                                    ex.contains(**s)
                                } else {
                                    false
                                }
                            })
                            .any(|s| line.contains(&format!("#{}", s)));

                        if has_tag && has_valid_state && !has_excluded_state {
                            return Some((
                                *line_nr,
                                format!("{}:{}:{}", filename, line_nr, content),
                            ));
                        }
                    }
                    None
                })
                .collect()
        })
        .collect();

    let mut filtered_lines: Vec<(usize, String)> = Vec::new();
    for batch in results {
        filtered_lines.extend(batch);
    }

    filtered_lines.sort_by_key(|(line_nr, _)| *line_nr);

    let output = filtered_lines
        .into_iter()
        .map(|(_, s)| s)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(output)
}

pub fn tasks_grep(
    config: &AgendaConfig,
    excluded: Option<&HashSet<String>>,
) -> Result<String, Box<dyn Error>> {
    let pattern = pattern_get(excluded);
    let vault_str = config.vault_dir_string();

    let output = Command::new("rg")
        .args(["-H", "-n", "--multiline", &pattern, "."])
        .current_dir(&vault_str)
        .output()?;

    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    let mut lines_by_file: std::collections::HashMap<String, Vec<(usize, String)>> =
        std::collections::HashMap::new();

    for line in lines {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() >= 2 {
            if let Some(filename) = parts.first() {
                let relative_name = if filename.starts_with("./") {
                    &filename[2..]
                } else {
                    filename
                };
                if let Some(line_nr_str) = parts.get(1) {
                    if let Ok(line_nr) = line_nr_str.parse::<usize>() {
                        let content = parts.get(2).map(|s| s.to_string());
                        lines_by_file
                            .entry(relative_name.to_string())
                            .or_default()
                            .push((line_nr, content.unwrap_or_default()));
                    }
                }
            }
        }
    }

    let mut filtered_lines: Vec<(usize, String)> = Vec::new();

    let vault_str = config.vault_dir_string();
    let tasks_by_file: Vec<(String, Vec<(usize, String)>)> = lines_by_file.into_iter().collect();

    let results: Vec<Vec<(usize, String)>> = tasks_by_file
        .par_iter()
        .map(|(filename, tasks)| {
            let full_path = Path::new(&vault_str).join(filename);
            let backlink_start_line = if let Ok(content) = std::fs::read_to_string(&full_path) {
                content
                    .lines()
                    .enumerate()
                    .find(|(_, l)| l.contains("<!-- BACKLINKS:START -->"))
                    .map(|(i, _)| i + 1)
            } else {
                None
            };

            tasks
                .iter()
                .filter_map(|(line_nr, content)| {
                    let include = match backlink_start_line {
                        Some(bl_start) => *line_nr < bl_start,
                        None => true,
                    };
                    if include {
                        Some((*line_nr, format!("{}:{}:{}", filename, line_nr, content)))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .collect();

    for batch in results {
        filtered_lines.extend(batch);
    }

    filtered_lines.sort_by_key(|(line_nr, _)| *line_nr);

    let output = filtered_lines
        .into_iter()
        .map(|(_, s)| s)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(output)
}

pub fn tasks_grep_property(
    config: &AgendaConfig,
    property: &str,
) -> Result<String, Box<dyn Error>> {
    let pattern = if property.contains('=') {
        let parts: Vec<&str> = property.splitn(2, '=').collect();
        let key = parts[0];
        let value = parts[1];
        format!(
            r"@{}\({}(?:T[^)]*)?\)",
            regex::escape(key),
            regex::escape(value)
        )
    } else {
        format!("@{}\\([^)]+\\)", regex::escape(property))
    };
    let vault_str = config.vault_dir_string();

    let output = Command::new("rg")
        .args(["-H", "-n", "--multiline", &pattern, "."])
        .current_dir(&vault_str)
        .output()?;

    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    let mut lines_by_file: std::collections::HashMap<String, Vec<(usize, String)>> =
        std::collections::HashMap::new();

    for line in lines {
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        if parts.len() >= 2 {
            if let Some(filename) = parts.first() {
                let relative_name = if filename.starts_with("./") {
                    &filename[2..]
                } else {
                    filename
                };
                if let Some(line_nr_str) = parts.get(1) {
                    if let Ok(line_nr) = line_nr_str.parse::<usize>() {
                        let content = parts.get(2).map(|s| s.to_string());
                        lines_by_file
                            .entry(relative_name.to_string())
                            .or_default()
                            .push((line_nr, content.unwrap_or_default()));
                    }
                }
            }
        }
    }

    let mut filtered_lines: Vec<(usize, String)> = Vec::new();

    let vault_str = config.vault_dir_string();
    let tasks_by_file: Vec<(String, Vec<(usize, String)>)> = lines_by_file.into_iter().collect();

    let results: Vec<Vec<(usize, String)>> = tasks_by_file
        .par_iter()
        .map(|(filename, tasks)| {
            let full_path = Path::new(&vault_str).join(filename);
            let (backlink_start_line, file_content) =
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    let bl_start = content
                        .lines()
                        .enumerate()
                        .find(|(_, l)| l.contains("<!-- BACKLINKS:START -->"))
                        .map(|(i, _)| i + 1);
                    (bl_start, Some(content))
                } else {
                    (None, None)
                };

            tasks
                .iter()
                .filter_map(|(line_nr, content)| {
                    let include = match backlink_start_line {
                        Some(bl_start) => *line_nr < bl_start,
                        None => true,
                    };
                    if !include {
                        return None;
                    }

                    let line_content = file_content
                        .as_ref()
                        .and_then(|c| c.lines().nth(*line_nr - 1));

                    if let Some(line) = line_content {
                        let has_valid_state = TASK_STATES
                            .iter()
                            .any(|s| line.contains(&format!("#{}", s)));

                        if !has_valid_state {
                            return None;
                        }
                    }

                    Some((*line_nr, format!("{}:{}:{}", filename, line_nr, content)))
                })
                .collect()
        })
        .collect();

    for batch in results {
        filtered_lines.extend(batch);
    }

    filtered_lines.sort_by_key(|(line_nr, _)| *line_nr);

    let output = filtered_lines
        .into_iter()
        .map(|(_, s)| s)
        .collect::<Vec<_>>()
        .join("\n");

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_task_state_next() {
        assert_eq!(task_state_next("TODO"), "IN_PROGRESS");
        assert_eq!(task_state_next("IN_PROGRESS"), "DONE");
        assert_eq!(task_state_next("DONE"), "NEXT");
        assert_eq!(task_state_next("NEXT"), "WAIT");
        assert_eq!(task_state_next("WAIT"), "LATER");
        assert_eq!(task_state_next("LATER"), "TODO");
        assert_eq!(task_state_next("UNKNOWN"), "TODO");
    }

    #[test]
    fn test_task_state_prev() {
        assert_eq!(task_state_prev("TODO"), "LATER");
        assert_eq!(task_state_prev("IN_PROGRESS"), "TODO");
        assert_eq!(task_state_prev("DONE"), "IN_PROGRESS");
        assert_eq!(task_state_prev("NEXT"), "DONE");
        assert_eq!(task_state_prev("WAIT"), "NEXT");
        assert_eq!(task_state_prev("LATER"), "WAIT");
        assert_eq!(task_state_prev("UNKNOWN"), "TODO");
    }

    #[test]
    fn test_pattern_get_all_states() {
        let pattern = pattern_get(None);
        assert!(pattern.contains("TODO"));
        assert!(pattern.contains("IN_PROGRESS"));
        assert!(pattern.contains("DONE"));
    }

    #[test]
    fn test_pattern_get_excluded() {
        let mut excluded = HashSet::new();
        excluded.insert("DONE".to_string());
        excluded.insert("CANCELLED".to_string());

        let pattern = pattern_get(Some(&excluded));
        assert!(pattern.contains("TODO"));
        assert!(!pattern.contains("DONE"));
    }

    #[test]
    fn test_tasks_grep_includes_tasks_before_backlinks() {
        let test_vault = std::env::temp_dir().join("test_vault_tasks");
        std::fs::create_dir_all(&test_vault).unwrap();

        let test_file = test_vault.join("test.md");
        std::fs::write(&test_file, "# Test\n\n- #TODO task before backlinks\n\n<!-- BACKLINKS:START -->\n## Backlinks\n\n- #TODO task in backlinks\n\n<!-- BACKLINKS:END -->").unwrap();

        let config = AgendaConfig::new(test_vault.clone());

        let result = tasks_grep(&config, None).unwrap();

        eprintln!("Result:\n{}", result);

        let has_task_before = result.contains("test.md:3");
        let has_task_in_backlinks = result.contains("test.md:7") || result.contains("test.md:8");

        assert!(has_task_before, "should find task before backlink section");
        assert!(
            !has_task_in_backlinks,
            "should not find task inside backlink section"
        );

        std::fs::remove_dir_all(&test_vault).ok();
    }

    #[test]
    fn test_tasks_grep_counts_file_reads() {
        let test_vault = std::env::temp_dir().join("test_vault_file_reads");
        std::fs::create_dir_all(&test_vault).unwrap();

        for i in 1..=5 {
            let test_file = test_vault.join(format!("file{}.md", i));
            std::fs::write(
                &test_file,
                format!(
                    "# File {}\n\n- Task {} #TODO\n\n<!-- BACKLINKS:START -->\n## Backlinks\n\n- Backlink #TODO\n\n<!-- BACKLINKS:END -->",
                    i, i
                ),
            )
            .unwrap();
        }

        let config = AgendaConfig::new(test_vault.clone());

        let result = tasks_grep(&config, None).unwrap();

        for i in 1..=5 {
            assert!(
                result.contains(&format!("file{}.md:3", i)),
                "should find task in file{} before backlinks",
                i
            );
            assert!(
                !result.contains(&format!("file{}.md:7", i)),
                "should NOT find backlink task in file{}",
                i
            );
        }

        std::fs::remove_dir_all(&test_vault).ok();
    }

    #[test]
    fn test_tasks_grep_property_excludes_backlinks() {
        let test_vault = std::env::temp_dir().join("test_vault_prop");
        std::fs::create_dir_all(&test_vault).unwrap();

        let test_file = test_vault.join("test.md");
        std::fs::write(
            &test_file,
            "# Test\n\n- Task #TODO @scheduled(2024-01-15)\n\n<!-- BACKLINKS:START -->\n## Backlinks\n\n- Backlink @scheduled(2024-01-20)\n\n<!-- BACKLINKS:END -->",
        )
        .unwrap();

        let config = AgendaConfig::new(test_vault.clone());

        let result = tasks_grep_property(&config, "scheduled").unwrap();

        assert!(result.contains("test.md:3"));
        assert!(!result.contains("test.md:7"));

        std::fs::remove_dir_all(&test_vault).ok();
    }

    #[test]
    fn test_pattern_get_include() {
        let mut included = HashSet::new();
        included.insert("TODO".to_string());
        included.insert("IN_PROGRESS".to_string());

        let pattern = pattern_get_include(&included);
        assert!(pattern.contains("#TODO"));
        assert!(pattern.contains("#IN_PROGRESS"));
        assert!(!pattern.contains("#DONE"));
    }

    #[test]
    fn test_tasks_grep_include_only_todo() {
        let test_vault = std::env::temp_dir().join("test_vault_include");
        std::fs::create_dir_all(&test_vault).unwrap();

        let test_file = test_vault.join("test.md");
        std::fs::write(
            &test_file,
            "# Test\n\n- TODO task #TODO\n- IN_PROGRESS task #IN_PROGRESS\n- DONE task #DONE\n",
        )
        .unwrap();

        let config = AgendaConfig::new(test_vault.clone());

        let mut included = HashSet::new();
        included.insert("TODO".to_string());
        let result = tasks_grep_include(&config, &included).unwrap();

        assert!(result.contains("TODO task"));
        assert!(!result.contains("IN_PROGRESS task"));
        assert!(!result.contains("DONE task"));

        std::fs::remove_dir_all(&test_vault).ok();
    }

    #[test]
    fn test_tasks_grep_tag_filter() {
        let test_vault = std::env::temp_dir().join("test_vault_tag");
        std::fs::create_dir_all(&test_vault).unwrap();

        let test_file = test_vault.join("test.md");
        std::fs::write(
            &test_file,
            "# Test\n\n- Bug task #TODO #bug\n- Feature task #TODO #feature\n- Bug done #DONE #bug\n",
        )
        .unwrap();

        let config = AgendaConfig::new(test_vault.clone());

        let mut excluded = HashSet::new();
        excluded.insert("DONE".to_string());
        let result = tasks_grep_tag(&config, "bug", Some(&excluded)).unwrap();

        eprintln!("Result:\n{}", result);

        assert!(result.contains("Bug task"));
        assert!(!result.contains("Feature task"));
        assert!(!result.contains("Bug done"));

        std::fs::remove_dir_all(&test_vault).ok();
    }

    #[test]
    fn test_tasks_grep_tag_filter_excludes_non_task_bullets() {
        let test_vault = std::env::temp_dir().join("test_vault_tag_non_task");
        std::fs::create_dir_all(&test_vault).unwrap();

        let test_file = test_vault.join("test.md");
        std::fs::write(
            &test_file,
            "# Test\n\n* bullet with #foo but no state\n- bullet with #bug but no state\n- real task #TODO #bug\n- another task #IN_PROGRESS\n",
        )
        .unwrap();

        let config = AgendaConfig::new(test_vault.clone());

        let mut excluded = HashSet::new();
        excluded.insert("DONE".to_string());
        let result = tasks_grep_tag(&config, "bug", Some(&excluded)).unwrap();

        assert!(result.contains("real task"));
        assert!(!result.contains("bullet with #bug"));

        std::fs::remove_dir_all(&test_vault).ok();
    }

    #[test]
    fn test_tasks_grep_property_with_value() {
        let test_vault = std::env::temp_dir().join("test_vault_prop_value");
        std::fs::create_dir_all(&test_vault).unwrap();

        let test_file = test_vault.join("test.md");
        std::fs::write(
            &test_file,
            "# Test\n\n- Task A #TODO @scheduled(2024-01-01)\n- Task B #TODO @scheduled(2024-01-02)\n- Task C #TODO @scheduled(2024-01-01)\n",
        )
        .unwrap();

        let config = AgendaConfig::new(test_vault.clone());

        let result = tasks_grep_property(&config, "scheduled=2024-01-01").unwrap();

        assert!(result.contains("Task A"));
        assert!(result.contains("Task C"));
        assert!(!result.contains("Task B"));

        std::fs::remove_dir_all(&test_vault).ok();
    }

    #[test]
    fn test_tasks_grep_property_different_key() {
        let test_vault = std::env::temp_dir().join("test_vault_prop_key");
        std::fs::create_dir_all(&test_vault).unwrap();

        let test_file = test_vault.join("test.md");
        std::fs::write(
            &test_file,
            "# Test\n\n- High task #TODO @priority(high)\n- Low task #TODO @priority(low)\n- High again #TODO @priority(high)\n",
        )
        .unwrap();

        let config = AgendaConfig::new(test_vault.clone());

        let result = tasks_grep_property(&config, "priority=high").unwrap();

        assert!(result.contains("High task"));
        assert!(result.contains("High again"));
        assert!(!result.contains("Low task"));

        std::fs::remove_dir_all(&test_vault).ok();
    }
}
