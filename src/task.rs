use crate::config::AgendaConfig;
use crate::fs_utils::{file_line_insert, file_lines_get};
use crate::grep::{tasks_grep, TASK_STATES};
use crate::repository::TaskRepository;
use crate::utils::group_by;
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;

pub const TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"#(\w|-)+").unwrap());
pub const HEADING_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(?m)\s*#+\s").unwrap());
pub const HAS_STATE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"#[A-Z]").unwrap());
pub const IS_ONLY_META_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*((@\w+\(.*\)|#\w+)\s*)+$").unwrap());
pub const PROP_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"@(\w+)\(([^)]*)\)").unwrap());

pub fn is_state_tag(tag: &str) -> bool {
    tag.chars().all(|c| c == '_' || c.is_uppercase())
}

#[derive(Serialize, Clone, Debug)]
pub struct Task {
    pub content: String,
    pub state: String,
    pub nest_level: u8,
    pub tags: Vec<String>,
    pub body: String,
    pub id: String,
    pub parent: Option<String>,
    pub children: Vec<Task>,
    pub properties: IndexMap<String, String>,
}

impl Task {
    pub fn tags_populate(&mut self) {
        TAG_RE.find_iter(&self.body).for_each(|i| {
            let tag = (&(i.as_str())[1..]).to_string();
            if is_state_tag(&tag) {
                self.state = tag;
            } else {
                self.tags.push(tag);
            }
        });

        let mut props = IndexMap::new();
        PROP_RE.find_iter(&self.body).for_each(|m| {
            let captures = PROP_RE.captures(m.as_str()).unwrap();
            let key = captures.get(1).unwrap().as_str().to_string();
            let value = captures.get(2).unwrap().as_str().to_string();
            props.insert(key, value);
        });
        self.properties = props;
    }

    pub fn from_string(line: &str, id: &str) -> Task {
        let line = line_strip(line);
        let mut t = Task {
            tags: vec![],
            nest_level: 0,
            state: "".to_string(),
            content: "".to_string(),
            body: line.clone(), // raw line
            id: id.to_string(),
            parent: None,
            children: vec![],
            properties: IndexMap::new(),
        };
        t.tags_populate();

        // derive content (cleaned from tags, properties, and bullets)
        let mut cleaned = TAG_RE.replace_all(&line, "").to_string();
        cleaned = PROP_RE.replace_all(&cleaned, "").to_string();

        // remove bullet symbols
        let mut content = cleaned.trim_start().to_string();
        if let Some(stripped) = content
            .strip_prefix("* ")
            .or_else(|| content.strip_prefix("- "))
            .or_else(|| content.strip_prefix("+ "))
        {
            content = stripped.to_string();
        }

        t.content = content.trim().to_string();
        t
    }
}

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Task {}

pub fn line_strip(line: &str) -> String {
    HEADING_RE.replace(line, "").to_string()
}

pub fn has_state(line: &str) -> bool {
    HAS_STATE_RE.is_match(line)
}

pub fn is_only_meta(line: &str) -> bool {
    if has_state(line) {
        return false;
    }
    IS_ONLY_META_RE.is_match(line)
}

pub fn task_get_by_id(config: &AgendaConfig, task_id: &str) -> Task {
    let task_ids = tasks_ids_get(config);

    let filename = task_filename_get(&task_id);
    let full_path = config.vault_dir.join(&filename);
    let lines = file_lines_get(full_path.to_string_lossy().as_ref());
    task_get_from_lines(task_id, &lines, &task_ids).expect("HANDLE THIS - RETURN OPTION")
}

pub fn task_parent_get(config: &AgendaConfig, child_id: String) -> Option<Task> {
    let filename = task_filename_get(&child_id);
    let line_nr = task_line_get(&child_id);
    let lines = file_lines_get(&filename);
    let task_ids = tasks_ids_get(config);

    let current_line = &lines[line_nr - 1];
    if !is_line_bullet(&current_line) {
        return None;
    }

    let current_indent = current_line.chars().take_while(|c| *c == ' ').count();

    let mut search_indent = current_indent;

    for i in (0..(line_nr - 1)).rev() {
        let line = &lines[i];

        if is_line_heading(&line) {
            break;
        }

        if !is_line_bullet(&line) {
            continue;
        }

        let indent = line.chars().take_while(|c| *c == ' ').count();

        if indent < search_indent {
            let potential_parent_id = format!("{}:{}", filename, i + 1);

            if task_ids.contains(&potential_parent_id) {
                return Some(task_get_by_id(config, &potential_parent_id));
            } else {
                search_indent = indent;
                continue;
            }
        }
    }

    None
}

pub fn flatten_and_collect<'a>(tasks: &'a [Task]) -> (Vec<&'a Task>, HashSet<&'a str>) {
    let mut flat: Vec<&Task> = Vec::with_capacity(tasks.len());
    let mut child_ids: HashSet<&str> = HashSet::new();
    let mut stack: Vec<&Task> = tasks.iter().collect();

    while let Some(task) = stack.pop() {
        flat.push(task);
        for child in &task.children {
            child_ids.insert(&child.id);
            stack.push(child);
        }
    }

    (flat, child_ids)
}

pub fn tasks_group_date(tasks: &[Task]) -> IndexMap<String, Vec<&Task>> {
    group_by(tasks, |task| {
        task.properties
            .get("scheduled")
            .map(|date| date.split('T').next().unwrap_or(date).to_string())
    })
}

pub fn tasks_group_tag(tasks: &[Task]) -> IndexMap<String, Vec<&Task>> {
    use crate::utils::group_by_multi;
    group_by_multi(tasks, |task| {
        if task.tags.is_empty() {
            vec!["NO TAG".to_string()]
        } else {
            task.tags.clone()
        }
    })
}

pub fn tasks_group_by_date_and_tag(
    tasks: &[Task],
) -> (IndexMap<String, Vec<&Task>>, IndexMap<String, Vec<&Task>>) {
    let grouped_date = tasks_group_date(tasks);
    let grouped_tag = tasks_group_tag(tasks);
    (grouped_date, grouped_tag)
}

pub fn tasks_group_by_property<'a>(
    tasks: &'a [Task],
    property: &str,
) -> IndexMap<String, Vec<&'a Task>> {
    group_by(tasks, |task| {
        task.properties
            .get(property)
            .map(|v| v.split('T').next().unwrap_or(v.as_str()).to_string())
    })
}

pub fn task_filename_get(task_id: &str) -> String {
    task_id.split(':').next().unwrap_or("").to_string()
}

pub fn task_line_get(task_id: &str) -> usize {
    task_id.split(':').nth(1).unwrap().parse().unwrap()
}

pub fn tasks_group_by_filename(task_ids: &HashSet<String>) -> IndexMap<String, Vec<&str>> {
    let mut acc: IndexMap<String, Vec<&str>> = IndexMap::new();

    for task_id in task_ids {
        acc.entry(task_filename_get(&task_id))
            .or_insert_with(Vec::new)
            .push(&task_id);
    }

    acc
}

fn is_line_bullet(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("* ") || trimmed.starts_with("- ") || trimmed.starts_with("+ ")
}

fn is_line_heading(line: &str) -> bool {
    line.trim_start().starts_with("# ")
}

// fn is_task_line(line: &str) -> bool {
//     TAG_RE.is_match(line)
// }

pub fn task_get_from_lines(
    task_id: &str,
    file_lines: &Vec<String>,
    task_ids: &HashSet<String>,
) -> Option<Task> {
    fn aux(
        task_id: &str,
        file_lines: &Vec<String>,
        task_ids: &HashSet<String>,
        skip_set: &mut HashSet<String>,
    ) -> Option<Task> {
        if skip_set.contains(task_id) {
            return None;
        }

        skip_set.insert(task_id.to_string());

        let line_nr = task_line_get(task_id);
        let line = file_lines[line_nr - 1].clone();
        let mut root = Task::from_string(&line, task_id);

        let can_have_children = is_line_bullet(&line);
        if can_have_children {
            let current_indent = line.chars().take_while(|c| *c == ' ').count();
            let mut i = line_nr;

            while i < file_lines.len() {
                let next_line = &file_lines[i];

                if is_line_heading(&next_line) {
                    break;
                }

                if !is_line_bullet(&next_line) {
                    i += 1;
                    continue;
                }

                let next_indent = next_line.chars().take_while(|c| *c == ' ').count();
                if next_indent <= current_indent {
                    break;
                }

                let potential_child_id = format!("{}:{}", task_filename_get(task_id), i + 1);
                if task_ids.contains(&potential_child_id) {
                    if let Some(mut child) =
                        aux(&potential_child_id, file_lines, task_ids, skip_set)
                    {
                        child.parent = Some(root.id.clone());
                        root.children.push(child);
                    }
                }

                i += 1;
            }
        }

        Some(root)
    }

    let mut skip_set = HashSet::new();
    let mut task_ids_set = HashSet::new();
    task_ids.iter().for_each(|id| {
        task_ids_set.insert(id.clone());
    });
    aux(task_id, file_lines, &task_ids_set, &mut skip_set)
}

pub fn tasks_get(config: &AgendaConfig) -> Vec<Task> {
    let repo = crate::repository::MarkdownTaskRepository::new(config.clone());
    repo.load_tasks().unwrap_or_default()
}

pub fn tasks_sort_by(tasks: &mut Vec<Task>, criteria: &[String]) {
    if criteria.is_empty() {
        return tasks_sort(tasks);
    }

    tasks.sort_by(|a, b| {
        for crit in criteria {
            let ord = if crit == "state" {
                a.state.cmp(&b.state)
            } else {
                let a_val = a.properties.get(crit);
                let b_val = b.properties.get(crit);
                match (a_val, b_val) {
                    (Some(av), Some(bv)) => {
                        let a_cmp = if av.contains('-') && !av.contains('T') {
                            format!("{}TZZ:ZZ", av)
                        } else {
                            av.clone()
                        };
                        let b_cmp = if bv.contains('-') && !bv.contains('T') {
                            format!("{}TZZ:ZZ", bv)
                        } else {
                            bv.clone()
                        };
                        a_cmp.cmp(&b_cmp)
                    }
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                }
            };

            if ord != std::cmp::Ordering::Equal {
                return ord;
            }
        }
        std::cmp::Ordering::Equal
    });
}

pub fn tasks_sort(tasks: &mut Vec<Task>) {
    tasks_sort_by(tasks, &["scheduled".to_string(), "priority".to_string()]);
}

pub fn tasks_filter_by_tag(tasks: &[Task], tag: &str) -> Vec<Task> {
    tasks
        .iter()
        .filter(|t| t.tags.contains(&tag.to_string()))
        .cloned()
        .collect()
}

pub fn tasks_filter_by_property(tasks: &[Task], property: &str, value: Option<&str>) -> Vec<Task> {
    tasks
        .iter()
        .filter(|t| {
            if let Some(v) = value {
                t.properties
                    .get(property)
                    .map(|pv| pv.split('T').next().unwrap_or(pv.as_str()) == v)
                    .unwrap_or(false)
            } else {
                t.properties.contains_key(property)
            }
        })
        .cloned()
        .collect()
}

pub fn tasks_filter_by_states(tasks: &[Task], exclude_states: &[String]) -> Vec<Task> {
    tasks
        .iter()
        .filter(|t| !exclude_states.contains(&t.state))
        .cloned()
        .collect()
}

pub fn tasks_filter_by_states_included(tasks: &[Task], include_states: &[String]) -> Vec<Task> {
    tasks
        .iter()
        .filter(|t| include_states.contains(&t.state))
        .cloned()
        .collect()
}

pub fn tasks_filter_by_content(tasks: &[Task], query: &str) -> Vec<Task> {
    let query_lower = query.to_lowercase();
    tasks
        .iter()
        .filter(|t| t.content.to_lowercase().contains(&query_lower))
        .cloned()
        .collect()
}

pub fn tasks_ids_get(config: &AgendaConfig) -> HashSet<String> {
    let mut exclude_set = HashSet::new();
    exclude_set.insert("DONE".to_string());
    exclude_set.insert("CANCELLED".to_string());

    let rg_result = tasks_grep(config, Some(&exclude_set));
    let rg_result = match rg_result {
        Ok(output) => output,
        Err(_) => "".to_string(),
    };

    rg_result
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, ':');
            let filename = parts.next()?;
            let line_nr = parts.next()?;
            Some(format!("{}:{}", filename, line_nr))
        })
        .collect()
}

pub fn task_line_set(config: &AgendaConfig, id: &str, line: &str) -> String {
    let filename = task_filename_get(id);
    let full_path = config.vault_dir.join(&filename);
    let line_nr = task_line_get(id);
    let mut lines = file_lines_get(full_path.to_string_lossy().as_ref());

    lines[line_nr - 1] = line.to_string();

    let mut file = File::create(&full_path).unwrap();
    for l in &lines {
        writeln!(file, "{}", l).unwrap();
    }

    lines[line_nr - 1].clone()
}

pub fn task_set_fields(
    config: &AgendaConfig,
    task_id: &str,
    content: Option<&str>,
    state: Option<&str>,
    properties: Option<&std::collections::HashMap<String, String>>,
    tags: Option<&[String]>,
    clear_tags: bool,
    clear_properties: bool,
) -> Result<Task, String> {
    if state.is_some() && !TASK_STATES.contains(&state.unwrap()) {
        return Err(format!("Invalid state: {}", state.unwrap()));
    }

    let filename = task_filename_get(task_id);
    let full_path = config.vault_dir.join(&filename);
    let line_nr = task_line_get(task_id);
    let mut lines = file_lines_get(full_path.to_string_lossy().as_ref());

    if line_nr == 0 || line_nr > lines.len() {
        return Err("Invalid line number".to_string());
    }

    let original_line = &lines[line_nr - 1];

    let indent: String = original_line
        .chars()
        .take_while(|c| *c == ' ')
        .collect();

    let trimmed = original_line.trim_start();
    let bullet = if trimmed.starts_with("- ") {
        "-"
    } else if trimmed.starts_with("+ ") {
        "+"
    } else {
        "*"
    };

    let mut task = Task::from_string(original_line, task_id);

    if let Some(c) = content {
        task.content = c.to_string();
    }

    if let Some(s) = state {
        task.state = s.to_string();
    }

    if clear_properties {
        task.properties.clear();
    } else if let Some(props) = properties {
        for (key, value) in props {
            task.properties.insert(key.clone(), value.clone());
        }
    }

    if clear_tags {
        task.tags.clear();
    } else if let Some(t) = tags {
        task.tags = t.to_vec();
    }

    let mut new_line = if indent.is_empty() {
        format!("{} #{} {}", bullet, task.state, task.content)
    } else {
        format!("{}{} #{} {}", indent, bullet, task.state, task.content)
    };

    let mut sorted_keys: Vec<&String> = task.properties.keys().collect();
    sorted_keys.sort();
    for key in sorted_keys {
        new_line.push_str(&format!(
            " @{}({})",
            key,
            task.properties.get(key).unwrap()
        ));
    }

    for tag in &task.tags {
        new_line.push_str(&format!(" #{}", tag));
    }

    lines[line_nr - 1] = new_line.clone();

    let mut file = File::create(&full_path).map_err(|e| e.to_string())?;
    for l in &lines {
        writeln!(file, "{}", l).map_err(|e| e.to_string())?;
    }

    task.body = new_line;
    Ok(task)
}

pub fn task_add(config: &AgendaConfig, content: &str, parent: Option<&str>) {
    if let Some(parent_id) = parent {
        let filename = task_filename_get(parent_id);
        let lines = file_lines_get(&filename);

        let parent_line_nr = task_line_get(parent_id); // 1-based
        let parent_idx = parent_line_nr - 1; // convert to 0-based

        let parent_indent = lines[parent_idx].chars().take_while(|c| *c == ' ').count();
        let subtask_indent = parent_indent + 2;

        let line_to_insert = format!("{}* #TODO {}", " ".repeat(subtask_indent), content);

        let mut insert_idx = lines.len();
        for i in (parent_idx + 1)..lines.len() {
            let line = &lines[i];

            if line.trim_start().starts_with('#') {
                insert_idx = i;
                break;
            }

            if (line.trim_start().starts_with("* ")
                || line.trim_start().starts_with("- ")
                || line.trim_start().starts_with("+ "))
                && line.chars().take_while(|c| *c == ' ').count() <= parent_indent
            {
                insert_idx = i;
                break;
            }
        }

        file_line_insert(&filename, Some(insert_idx + 1), &line_to_insert)
            .expect("failed to add subtask");
    } else {
        let line_to_insert = format!("* #TODO {}", content);
        let agenda_file = config.default_file_string();
        file_line_insert(&agenda_file, None, &line_to_insert).expect("failed to add task");
    }
}

pub fn task_change_state(
    config: &AgendaConfig,
    task_id: &str,
    new_state: &str,
) -> Result<(), String> {
    let filename = task_filename_get(task_id);
    let full_path = config.vault_dir.join(&filename);
    let line_nr = task_line_get(task_id);
    let mut lines = file_lines_get(full_path.to_string_lossy().as_ref());

    if line_nr == 0 || line_nr > lines.len() {
        return Err("Invalid line number".to_string());
    }

    let line = &lines[line_nr - 1];

    let current_state = TASK_STATES
        .iter()
        .find(|&&state| line.contains(&format!("#{}", state)))
        .map(|s| s.to_string());

    let next_state = new_state.to_string();

    let mut updated_line = line.clone();
    if let Some(current) = current_state {
        let pattern = format!("#{}", current);
        updated_line = updated_line.replace(&pattern, &format!("#{}", next_state));
    } else {
        if let Some(pos) = updated_line.find('#') {
            updated_line.insert_str(pos, &format!("#{} ", next_state));
        } else {
            updated_line.push_str(&format!(" #{}", next_state));
        }
    }

    lines[line_nr - 1] = updated_line;
    let mut file = File::create(&full_path).map_err(|e| e.to_string())?;
    for l in lines {
        writeln!(file, "{}", l).map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn task_change_property(
    config: &AgendaConfig,
    task_id: &str,
    key: &str,
    value: Option<&str>,
) -> Result<(), String> {
    let filename = task_filename_get(task_id);
    let full_path = config.vault_dir.join(&filename);
    let line_nr = task_line_get(task_id);
    let mut lines = file_lines_get(full_path.to_string_lossy().as_ref());

    if line_nr == 0 || line_nr > lines.len() {
        return Err("Invalid line number".to_string());
    }

    let original_line = &lines[line_nr - 1];

    let (indent, content) = original_line
        .chars()
        .enumerate()
        .find(|(_, c)| !c.is_whitespace())
        .map(|(idx, _)| original_line.split_at(idx))
        .unwrap_or(("", original_line));

    let mut new_content = content.to_string();

    let prop_re = regex::Regex::new(&format!(r"@{}\([^)]*\)", regex::escape(key)))
        .map_err(|e| e.to_string())?;

    if let Some(val) = value {
        // if property exists -> replace its value; otherwise append it
        if prop_re.is_match(&new_content) {
            new_content = prop_re
                .replace_all(&new_content, format!("@{}({})", key, val))
                .to_string();
        } else {
            if new_content.ends_with(' ') {
                new_content.push_str(&format!("@{}({})", key, val));
            } else {
                new_content.push_str(&format!(" @{}({})", key, val));
            }
        }
    } else {
        // remove the property if value is None
        new_content = prop_re.replace_all(&new_content, "").to_string();
        new_content = new_content.replace("  ", " ").trim_end().to_string();
    }

    lines[line_nr - 1] = format!("{}{}", indent, new_content);

    let mut file = File::create(&full_path).map_err(|e| e.to_string())?;
    for l in lines {
        writeln!(file, "{}", l).map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn task_capture(
    config: &AgendaConfig,
    task_content: &str,
    target: &str,
    state: Option<&str>,
    properties: &std::collections::HashMap<String, String>,
    tags: &[String],
) -> Result<String, String> {
    use crate::grep::TASK_STATES;
    use std::fs::File;
    use std::io::Write;

    let resolved_state = if let Some(s) = state {
        if !TASK_STATES.contains(&s) {
            return Err(format!("Invalid state: {}", s));
        }
        s.to_string()
    } else {
        "TODO".to_string()
    };

    let mut task_line = format!("* #{} {}", resolved_state, task_content);

    let mut sorted_keys: Vec<&String> = properties.keys().collect();
    sorted_keys.sort();
    for key in sorted_keys {
        task_line.push_str(&format!(" @{}({})", key, properties.get(key).unwrap()));
    }

    for tag in tags {
        task_line.push_str(&format!(" #{}", tag));
    }

    if target.contains(':') {
        let filename = task_filename_get(target);
        let full_path = config.vault_dir.join(&filename);
        let full_path_str = full_path.to_string_lossy().to_string();

        let parent_line_nr = task_line_get(target);
        if parent_line_nr == 0 {
            return Err(format!("Invalid task ID: {}", target));
        }

        let lines = file_lines_get(&full_path_str);
        if parent_line_nr > lines.len() {
            return Err(format!("Parent task not found: {}", target));
        }

        let parent_idx = parent_line_nr - 1;
        let parent_line = &lines[parent_idx];
        if !has_state(parent_line) {
            return Err(format!("Target is not a task: {}", target));
        }

        let parent_indent = parent_line.chars().take_while(|c| *c == ' ').count();
        let subtask_indent = parent_indent + 2;

        let indented_line = format!("{}{}", " ".repeat(subtask_indent), task_line);

        let mut insert_idx = lines.len();
        for i in (parent_idx + 1)..lines.len() {
            let line = &lines[i];

            if line.trim_start().starts_with('#') {
                insert_idx = i;
                break;
            }

            if (line.trim_start().starts_with("* ")
                || line.trim_start().starts_with("- ")
                || line.trim_start().starts_with("+ "))
                && line.chars().take_while(|c| *c == ' ').count() <= parent_indent
            {
                insert_idx = i;
                break;
            }
        }

        let mut lines = lines;
        lines.insert(insert_idx, indented_line);

        let mut file = File::create(&full_path_str).map_err(|e| e.to_string())?;
        for line in &lines {
            writeln!(file, "{}", line).map_err(|e| e.to_string())?;
        }

        let task_id = format!("{}:{}", filename, insert_idx + 1);
        Ok(task_id)
    } else {
        let file_path = target;

        let full_file_path = std::path::Path::new(&config.vault_dir_string())
            .join(file_path)
            .to_string_lossy()
            .to_string();

        let file_content = if std::path::Path::new(&full_file_path).exists() {
            std::fs::read_to_string(&full_file_path).unwrap_or_default()
        } else {
            String::new()
        };

        let mut lines: Vec<String> = if file_content.is_empty() {
            vec![]
        } else {
            file_content.lines().map(|s| s.to_string()).collect()
        };

        lines.push(task_line);

        let mut file = File::create(&full_file_path).map_err(|e| e.to_string())?;
        for line in &lines {
            writeln!(file, "{}", line).map_err(|e| e.to_string())?;
        }

        let task_id = format!("{}:{}", file_path, lines.len());
        Ok(task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_state_tag() {
        assert!(is_state_tag("TODO"));
        assert!(is_state_tag("DONE"));
        assert!(is_state_tag("IN_PROGRESS"));
        assert!(is_state_tag("ABC"));
        assert!(is_state_tag("__"));
        assert!(!is_state_tag("todo"));
        assert!(!is_state_tag("InProgress"));
        assert!(!is_state_tag("task"));
    }

    #[test]
    fn test_task_from_string_basic() {
        let task = Task::from_string("* Buy milk #TODO @priority(1)", "test.md:1");
        assert_eq!(task.content, "Buy milk");
        assert_eq!(task.state, "TODO");
        assert_eq!(task.id, "test.md:1");
        assert!(task.tags.is_empty());
        assert_eq!(task.properties.get("priority"), Some(&"1".to_string()));
    }

    #[test]
    fn test_task_from_string_with_tags() {
        let task = Task::from_string("* Fix bug #bug #high-priority", "test.md:2");
        assert_eq!(task.content, "Fix bug");
        assert!(task.state.is_empty());
        assert!(task.tags.contains(&"bug".to_string()));
        assert!(task.tags.contains(&"high-priority".to_string()));
    }

    #[test]
    fn test_task_from_string_strips_bullet() {
        let task = Task::from_string("- Task with dash", "test.md:1");
        assert_eq!(task.content, "Task with dash");

        let task2 = Task::from_string("+ Task with plus", "test.md:2");
        assert_eq!(task2.content, "Task with plus");
    }

    #[test]
    fn test_task_from_string_strips_heading() {
        let task = Task::from_string("# Heading line #TODO", "test.md:1");
        assert_eq!(task.content, "Heading line");
    }

    #[test]
    fn test_task_from_string_multiple_properties() {
        let task = Task::from_string(
            "* Complex task #TODO @priority(2) @scheduled(2024-01-15)",
            "test.md:1",
        );
        assert_eq!(task.state, "TODO");
        assert_eq!(task.properties.get("priority"), Some(&"2".to_string()));
        assert_eq!(
            task.properties.get("scheduled"),
            Some(&"2024-01-15".to_string())
        );
    }

    #[test]
    fn test_line_strip() {
        assert_eq!(line_strip("# Title"), "Title");
        assert_eq!(line_strip("## Subtitle"), "Subtitle");
        assert_eq!(line_strip("No heading"), "No heading");
    }

    #[test]
    fn test_has_state() {
        assert!(has_state("#TODO"));
        assert!(has_state("#DONE #something"));
        assert!(!has_state("#todo"));
        assert!(!has_state("#tag"));
    }

    #[test]
    fn test_is_only_meta() {
        assert!(!is_only_meta("#TODO @priority(1)")); // has state, so false
        assert!(is_only_meta("#tag1 #tag2"));
        assert!(is_only_meta("@scheduled(2024-01-01)"));
        assert!(!is_only_meta("* Actual task #TODO"));
        assert!(!is_only_meta("Just text"));
    }

    #[test]
    fn test_task_filename_get() {
        assert_eq!(task_filename_get("path/to/file.md:42"), "path/to/file.md");
        assert_eq!(task_filename_get("simple.md:1"), "simple.md");
    }

    #[test]
    fn test_task_line_get() {
        assert_eq!(task_line_get("file.md:42"), 42);
        assert_eq!(task_line_get("file.md:1"), 1);
    }

    #[test]
    fn test_task_id_equality() {
        let task1 = Task::from_string("* test", "file.md:1");
        let task2 = Task::from_string("* test", "file.md:1");
        let task3 = Task::from_string("* test", "file.md:2");

        assert_eq!(task1, task2);
        assert_ne!(task1, task3);
    }

    #[test]
    fn test_tasks_group_date_groups_by_date() {
        let tasks = vec![
            Task {
                content: "Task 1".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 1 #TODO @scheduled(2024-01-15)".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2024-01-15".to_string())]),
            },
            Task {
                content: "Task 2".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 2 #TODO @scheduled(2024-01-15)".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([(
                    "scheduled".to_string(),
                    "2024-01-15T10:00".to_string(),
                )]),
            },
            Task {
                content: "Task 3".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 3 #TODO @scheduled(2024-01-20)".to_string(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2024-01-20".to_string())]),
            },
            Task {
                content: "Task no date".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task no date #TODO".to_string(),
                id: "test.md:4".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let grouped = tasks_group_date(&tasks);

        assert!(grouped.contains_key("2024-01-15"));
        assert!(grouped.contains_key("2024-01-20"));
        assert_eq!(grouped["2024-01-15"].len(), 2);
        assert_eq!(grouped["2024-01-20"].len(), 1);
    }

    #[test]
    fn test_tasks_group_date_with_children() {
        let child = Task {
            content: "Child task".to_string(),
            state: "TODO".to_string(),
            nest_level: 1,
            tags: vec![],
            body: "  - Child #TODO @scheduled(2024-02-01)".to_string(),
            id: "test.md:2".to_string(),
            parent: Some("test.md:1".to_string()),
            children: vec![],
            properties: IndexMap::from([("scheduled".to_string(), "2024-02-01".to_string())]),
        };

        let tasks = vec![Task {
            content: "Parent task".to_string(),
            state: "TODO".to_string(),
            nest_level: 0,
            tags: vec![],
            body: "* Parent #TODO @scheduled(2024-01-15)".to_string(),
            id: "test.md:1".to_string(),
            parent: None,
            children: vec![child],
            properties: IndexMap::from([("scheduled".to_string(), "2024-01-15".to_string())]),
        }];

        let grouped = tasks_group_date(&tasks);

        assert!(grouped.contains_key("2024-01-15"));
        assert!(grouped.contains_key("2024-02-01"));
    }

    #[test]
    fn test_tasks_group_tag_groups_by_tag() {
        let tasks = vec![
            Task {
                content: "Task 1".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["bug".to_string(), "urgent".to_string()],
                body: "* Task 1 #TODO #bug #urgent".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Task 2".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["bug".to_string()],
                body: "* Task 2 #TODO #bug".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Task 3".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 3 #TODO".to_string(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let grouped = tasks_group_tag(&tasks);

        assert!(grouped.contains_key("bug"));
        assert!(grouped.contains_key("urgent"));
        assert!(grouped.contains_key("NO TAG"));
        assert_eq!(grouped["bug"].len(), 2);
        assert_eq!(grouped["urgent"].len(), 1);
        assert_eq!(grouped["NO TAG"].len(), 1);
    }

    #[test]
    fn test_tasks_group_tag_with_children() {
        let child = Task {
            content: "Child".to_string(),
            state: "TODO".to_string(),
            nest_level: 1,
            tags: vec!["feature".to_string()],
            body: "  - Child #TODO #feature".to_string(),
            id: "test.md:2".to_string(),
            parent: Some("test.md:1".to_string()),
            children: vec![],
            properties: IndexMap::new(),
        };

        let tasks = vec![Task {
            content: "Parent".to_string(),
            state: "TODO".to_string(),
            nest_level: 0,
            tags: vec!["bug".to_string()],
            body: "* Parent #TODO #bug".to_string(),
            id: "test.md:1".to_string(),
            parent: None,
            children: vec![child],
            properties: IndexMap::new(),
        }];

        let grouped = tasks_group_tag(&tasks);

        assert!(grouped.contains_key("bug"));
        assert!(grouped.contains_key("feature"));
    }

    #[test]
    fn test_tasks_group_by_date_and_tag_combined() {
        let tasks = vec![
            Task {
                content: "Task 1".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["bug".to_string()],
                body: "* Task 1 #TODO #bug @scheduled(2024-01-15)".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2024-01-15".to_string())]),
            },
            Task {
                content: "Task 2".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["feature".to_string()],
                body: "* Task 2 #TODO #feature".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let (grouped_date, grouped_tag) = tasks_group_by_date_and_tag(&tasks);

        assert!(grouped_date.contains_key("2024-01-15"));
        assert_eq!(grouped_date["2024-01-15"].len(), 1);

        assert!(grouped_tag.contains_key("bug"));
        assert!(grouped_tag.contains_key("feature"));
        assert_eq!(grouped_tag["bug"].len(), 1);
        assert_eq!(grouped_tag["feature"].len(), 1);
    }

    #[test]
    fn test_tasks_group_by_property() {
        let tasks = vec![
            Task {
                content: "Task 1".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 1 #TODO @priority(1)".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "1".to_string())]),
            },
            Task {
                content: "Task 2".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 2 #TODO @priority(2)".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "2".to_string())]),
            },
            Task {
                content: "Task 3".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 3 #TODO".to_string(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let grouped = tasks_group_by_property(&tasks, "priority");

        assert!(grouped.contains_key("1"));
        assert!(grouped.contains_key("2"));
        assert!(!grouped.contains_key("3"));
        assert_eq!(grouped["1"].len(), 1);
        assert_eq!(grouped["2"].len(), 1);
        assert_eq!(grouped["1"][0].content, "Task 1");
    }

    #[test]
    fn test_tasks_group_by_property_with_children() {
        let child = Task {
            content: "Child task".to_string(),
            state: "TODO".to_string(),
            nest_level: 1,
            tags: vec![],
            body: "  - Child #TODO @scheduled(2024-02-01)".to_string(),
            id: "test.md:2".to_string(),
            parent: Some("test.md:1".to_string()),
            children: vec![],
            properties: IndexMap::from([("scheduled".to_string(), "2024-02-01".to_string())]),
        };

        let tasks = vec![Task {
            content: "Parent task".to_string(),
            state: "TODO".to_string(),
            nest_level: 0,
            tags: vec![],
            body: "* Parent #TODO @scheduled(2024-01-15)".to_string(),
            id: "test.md:1".to_string(),
            parent: None,
            children: vec![child],
            properties: IndexMap::from([("scheduled".to_string(), "2024-01-15".to_string())]),
        }];

        let grouped = tasks_group_by_property(&tasks, "scheduled");

        assert!(grouped.contains_key("2024-01-15"));
        assert!(grouped.contains_key("2024-02-01"));
    }

    #[test]
    fn test_tasks_group_by_property_skips_missing() {
        let tasks = vec![
            Task {
                content: "Task with scheduled".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task #TODO @scheduled(2024-01-15)".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2024-01-15".to_string())]),
            },
            Task {
                content: "Task without scheduled".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task no prop #TODO".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let grouped = tasks_group_by_property(&tasks, "scheduled");

        assert_eq!(grouped.len(), 1);
        assert!(grouped.contains_key("2024-01-15"));
        assert_eq!(grouped["2024-01-15"].len(), 1);
    }

    #[test]
    fn test_tasks_group_by_property_with_datetime() {
        let tasks = vec![
            Task {
                content: "Task with date only".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task #TODO @scheduled(2024-01-15)".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2024-01-15".to_string())]),
            },
            Task {
                content: "Task with datetime".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task #TODO @scheduled(2024-01-15T09:00)".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([(
                    "scheduled".to_string(),
                    "2024-01-15T09:00".to_string(),
                )]),
            },
        ];

        let grouped = tasks_group_by_property(&tasks, "scheduled");

        assert_eq!(grouped.len(), 1);
        assert!(grouped.contains_key("2024-01-15"));
        assert_eq!(grouped["2024-01-15"].len(), 2);
    }

    #[test]
    fn test_tasks_filter_by_tag() {
        let tasks = vec![
            Task {
                content: "Task 1".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["bug".to_string(), "urgent".to_string()],
                body: "* Task 1 #TODO #bug #urgent".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Task 2".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["feature".to_string()],
                body: "* Task 2 #TODO #feature".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Task 3".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 3 #TODO".to_string(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let filtered = tasks_filter_by_tag(&tasks, "bug");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].content, "Task 1");

        let filtered_feature = tasks_filter_by_tag(&tasks, "feature");
        assert_eq!(filtered_feature.len(), 1);
        assert_eq!(filtered_feature[0].content, "Task 2");

        let filtered_no_tag = tasks_filter_by_tag(&tasks, "nonexistent");
        assert!(filtered_no_tag.is_empty());
    }

    #[test]
    fn test_tasks_filter_by_property() {
        let tasks = vec![
            Task {
                content: "Task 1".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 1 #TODO @scheduled(2024-01-15)".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2024-01-15".to_string())]),
            },
            Task {
                content: "Task 2".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 2 #TODO @priority(1)".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "1".to_string())]),
            },
            Task {
                content: "Task 3 with datetime".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 3 #TODO @scheduled(2026-04-09T14:00)".to_string(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([(
                    "scheduled".to_string(),
                    "2026-04-09T14:00".to_string(),
                )]),
            },
            Task {
                content: "Task 4 with date only".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 4 #TODO @scheduled(2026-04-09)".to_string(),
                id: "test.md:4".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2026-04-09".to_string())]),
            },
        ];

        let filtered = tasks_filter_by_property(&tasks, "scheduled", None);
        assert_eq!(filtered.len(), 3);
        assert!(filtered.iter().any(|t| t.content == "Task 1"));
        assert!(filtered.iter().any(|t| t.content == "Task 3 with datetime"));
        assert!(filtered
            .iter()
            .any(|t| t.content == "Task 4 with date only"));

        let filtered_with_value = tasks_filter_by_property(&tasks, "scheduled", Some("2024-01-15"));
        assert_eq!(filtered_with_value.len(), 1);
        assert_eq!(filtered_with_value[0].content, "Task 1");

        let filtered_wrong_value =
            tasks_filter_by_property(&tasks, "scheduled", Some("2025-01-01"));
        assert!(filtered_wrong_value.is_empty());

        let filtered_with_datetime_value =
            tasks_filter_by_property(&tasks, "scheduled", Some("2026-04-09"));
        assert_eq!(filtered_with_datetime_value.len(), 2);
        assert!(filtered_with_datetime_value
            .iter()
            .any(|t| t.content == "Task 3 with datetime"));
        assert!(filtered_with_datetime_value
            .iter()
            .any(|t| t.content == "Task 4 with date only"));

        let filtered_priority = tasks_filter_by_property(&tasks, "priority", None);
        assert_eq!(filtered_priority.len(), 1);
        assert_eq!(filtered_priority[0].content, "Task 2");

        let filtered_none = tasks_filter_by_property(&tasks, "nonexistent", None);
        assert!(filtered_none.is_empty());
    }

    #[test]
    fn test_tasks_filter_by_states() {
        let tasks = vec![
            Task {
                content: "Task 1".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 1 #TODO".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Task 2".to_string(),
                state: "DONE".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 2 #DONE".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Task 3".to_string(),
                state: "IN_PROGRESS".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 3 #IN_PROGRESS".to_string(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Task 4".to_string(),
                state: "CANCELLED".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task 4 #CANCELLED".to_string(),
                id: "test.md:4".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let exclude_done: Vec<String> = vec!["DONE".to_string()];
        let filtered = tasks_filter_by_states(&tasks, &exclude_done);
        assert_eq!(filtered.len(), 3);
        assert!(filtered.iter().all(|t| t.state != "DONE"));

        let exclude_multiple: Vec<String> = vec!["DONE".to_string(), "CANCELLED".to_string()];
        let filtered2 = tasks_filter_by_states(&tasks, &exclude_multiple);
        assert_eq!(filtered2.len(), 2);
        assert!(filtered2
            .iter()
            .all(|t| t.state != "DONE" && t.state != "CANCELLED"));
    }

    #[test]
    fn test_tasks_filter_combined() {
        let tasks = vec![
            Task {
                content: "Bug task".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["bug".to_string()],
                body: "* Bug task #TODO #bug @priority(1)".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "1".to_string())]),
            },
            Task {
                content: "Feature task".to_string(),
                state: "DONE".to_string(),
                nest_level: 0,
                tags: vec!["feature".to_string()],
                body: "* Feature task #DONE #feature @priority(2)".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "2".to_string())]),
            },
            Task {
                content: "Another bug".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["bug".to_string()],
                body: "* Another bug #TODO #bug".to_string(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let exclude_done: Vec<String> = vec!["DONE".to_string()];
        let filtered = tasks_filter_by_states(&tasks, &exclude_done);
        let filtered = tasks_filter_by_tag(&filtered, "bug");

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|t| t.tags.contains(&"bug".to_string())));
    }

    #[test]
    fn test_tasks_sort_by_state() {
        let mut tasks = vec![
            Task {
                content: "Task A".to_string(),
                state: "DONE".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task A #DONE".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Task B".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task B #TODO".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Task C".to_string(),
                state: "IN_PROGRESS".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task C #IN_PROGRESS".to_string(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        tasks_sort_by(&mut tasks, &["state".to_string()]);

        assert_eq!(tasks[0].state, "DONE");
        assert_eq!(tasks[1].state, "IN_PROGRESS");
        assert_eq!(tasks[2].state, "TODO");
    }

    #[test]
    fn test_tasks_sort_by_priority_then_state() {
        let mut tasks = vec![
            Task {
                content: "Task A".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task A #TODO @priority(2)".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "2".to_string())]),
            },
            Task {
                content: "Task B".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task B #TODO @priority(1)".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "1".to_string())]),
            },
            Task {
                content: "Task C".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task C #TODO @priority(1)".to_string(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "1".to_string())]),
            },
        ];

        tasks_sort_by(&mut tasks, &["priority".to_string(), "state".to_string()]);

        assert_eq!(tasks[0].properties.get("priority"), Some(&"1".to_string()));
        assert_eq!(tasks[1].properties.get("priority"), Some(&"1".to_string()));
        assert_eq!(tasks[2].properties.get("priority"), Some(&"2".to_string()));
    }

    #[test]
    fn test_tasks_sort_by_scheduled() {
        let mut tasks = vec![
            Task {
                content: "Task A".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task A #TODO @scheduled(2024-02-01)".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2024-02-01".to_string())]),
            },
            Task {
                content: "Task B".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task B #TODO @scheduled(2024-01-15)".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2024-01-15".to_string())]),
            },
        ];

        tasks_sort_by(&mut tasks, &["scheduled".to_string()]);

        assert_eq!(tasks[0].content, "Task B");
        assert_eq!(tasks[1].content, "Task A");
    }

    #[test]
    fn test_tasks_filter_by_content() {
        let tasks = vec![
            Task {
                content: "Buy groceries".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["shopping".to_string()],
                body: "* Buy groceries #TODO #shopping".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Call mom".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Call mom #TODO".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Buy milk".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["shopping".to_string()],
                body: "* Buy milk #TODO #shopping".to_string(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let filtered = tasks_filter_by_content(&tasks, "buy");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|t| t.content.to_lowercase().contains("buy")));

        let filtered_case = tasks_filter_by_content(&tasks, "BUY");
        assert_eq!(filtered_case.len(), 2);

        let filtered_none = tasks_filter_by_content(&tasks, "xyz");
        assert!(filtered_none.is_empty());
    }

    #[test]
    fn test_tasks_filter_multiple_criteria_and_logic() {
        let tasks = vec![
            Task {
                content: "Task with bug and priority".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["bug".to_string()],
                body: "* Task with bug and priority #TODO #bug @priority(high)".to_string(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "high".to_string())]),
            },
            Task {
                content: "Task with bug only".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec!["bug".to_string()],
                body: "* Task with bug only #TODO #bug".to_string(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                content: "Task with priority only".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Task with priority only #TODO @priority(high)".to_string(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "high".to_string())]),
            },
            Task {
                content: "Regular task".to_string(),
                state: "TODO".to_string(),
                nest_level: 0,
                tags: vec![],
                body: "* Regular task #TODO".to_string(),
                id: "test.md:4".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let filtered_tag = tasks_filter_by_tag(&tasks, "bug");
        assert_eq!(filtered_tag.len(), 2);

        let filtered_prop = tasks_filter_by_property(&tasks, "priority", Some("high"));
        assert_eq!(filtered_prop.len(), 2);

        let filtered_and = tasks_filter_by_tag(&tasks, "bug");
        let filtered_and = tasks_filter_by_property(&filtered_and, "priority", Some("high"));
        assert_eq!(filtered_and.len(), 1);
        assert_eq!(filtered_and[0].content, "Task with bug and priority");
    }
}
