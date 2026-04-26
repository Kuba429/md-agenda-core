use crate::config::AgendaConfig;
use crate::fs_utils::{file_line_insert, file_lines_get};
use crate::grep::{tasks_grep, TASK_STATES};
use crate::repository::TaskRepository;
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
    pub title: String,
    pub state: String,
    pub tags: Vec<String>,
    pub raw: String,
    pub body: String,
    pub id: String,
    pub parent: Option<String>,
    pub children: Vec<Task>,
    pub properties: IndexMap<String, String>,
}

impl Task {
    pub fn tags_populate(&mut self) {
        TAG_RE.find_iter(&self.raw).for_each(|i| {
            let tag = (&(i.as_str())[1..]).to_string();
            if is_state_tag(&tag) {
                self.state = tag;
            } else {
                self.tags.push(tag);
            }
        });

        let mut props = IndexMap::new();
        PROP_RE.find_iter(&self.raw).for_each(|m| {
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
            state: "".to_string(),
            raw: line.clone(),
            title: "".to_string(),
            body: String::new(),
            id: id.to_string(),
            parent: None,
            children: vec![],
            properties: IndexMap::new(),
        };
        t.tags_populate();

        let mut cleaned = TAG_RE.replace_all(&line, "").to_string();
        cleaned = PROP_RE.replace_all(&cleaned, "").to_string();

        let mut title = cleaned.trim_start().to_string();
        if let Some(stripped) = title
            .strip_prefix("* ")
            .or_else(|| title.strip_prefix("- "))
            .or_else(|| title.strip_prefix("+ "))
        {
            title = stripped.to_string();
        }

        t.title = title.trim().to_string();
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
    resolve_task_tree(config, task_id)
}

pub fn compute_parent(task_id: &str, file_lines: &[String]) -> Option<String> {
    let filename = task_filename_get(task_id);
    let line_nr = task_line_get(task_id);
    if line_nr == 0 || line_nr > file_lines.len() {
        return None;
    }

    let current_line = &file_lines[line_nr - 1];
    if !is_line_bullet(current_line) {
        return None;
    }

    let current_indent = compute_line_indent(current_line);
    let mut search_indent = current_indent;

    for i in (0..(line_nr - 1)).rev() {
        let line = &file_lines[i];

        if is_line_heading(line) {
            break;
        }

        if !is_line_bullet(line) {
            continue;
        }

        let indent = compute_line_indent(line);

        if indent < search_indent {
            if has_state(line) {
                return Some(format!("{}:{}", filename, i + 1));
            } else {
                search_indent = indent;
                continue;
            }
        }
    }

    None
}

pub fn task_parent_get(config: &AgendaConfig, child_id: String) -> Option<Task> {
    let filename = task_filename_get(&child_id);
    let line_nr = task_line_get(&child_id);
    let full_path = config.vault_dir.join(&filename);
    let lines = file_lines_get(full_path.to_string_lossy().as_ref());

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
            if has_state(&line) {
                let potential_parent_id = format!("{}:{}", filename, i + 1);
                return Some(task_get_by_id(config, &potential_parent_id));
            } else {
                search_indent = indent;
                continue;
            }
        }
    }

    None
}

pub fn compute_line_indent(line: &str) -> usize {
    let mut indent = 0;
    for c in line.chars() {
        if c == ' ' {
            indent += 1;
        } else if c == '\t' {
            indent += 2; // TODO: make tab width configurable
        } else {
            break;
        }
    }
    indent
}

pub fn parse_task_body(task_id: &str, file_lines: &[String]) -> String {
    let line_nr = task_line_get(task_id);
    if line_nr == 0 || line_nr > file_lines.len() {
        return String::new();
    }

    let task_line = &file_lines[line_nr - 1];
    let task_indent = compute_line_indent(task_line);

    let mut raw_lines: Vec<(usize, &String)> = Vec::new();
    let mut i = line_nr;

    while i < file_lines.len() {
        let line = &file_lines[i];
        let line_indent = compute_line_indent(line);

        if line_indent <= task_indent {
            break;
        }

        raw_lines.push((line_indent, line));
        i += 1;
    }

    if raw_lines.is_empty() {
        return String::new();
    }

    let min_indent = raw_lines.iter().map(|(ind, _)| *ind).min().unwrap_or(0);

    raw_lines
        .into_iter()
        .map(|(_, line)| {
            if min_indent > 0 && line.len() >= min_indent {
                line[min_indent..].to_string()
            } else {
                line.clone()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn resolve_task_tree(config: &AgendaConfig, task_id: &str) -> Task {
    let filename = task_filename_get(task_id);
    let full_path = config.vault_dir.join(&filename);
    let lines = file_lines_get(full_path.to_string_lossy().as_ref());
    let line_nr = task_line_get(task_id);

    if line_nr == 0 || line_nr > lines.len() {
        return Task::from_string("", task_id);
    }

    let task_line = &lines[line_nr - 1];
    let mut task = Task::from_string(task_line, task_id);
    task.body = parse_task_body(task_id, &lines);
    task.parent = compute_parent(task_id, &lines);

    let task_indent = compute_line_indent(task_line);
    let mut children = Vec::new();
    let mut i = line_nr; // 0-based, starts after task line

    while i < lines.len() {
        let line = &lines[i];
        let line_indent = compute_line_indent(line);

        if line_indent <= task_indent {
            break;
        }

        let trimmed = line.trim();
        let is_bullet = trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ");

        if is_bullet && has_state(line) {
            let child_id = format!("{}:{}", filename, i + 1);
            let mut child = resolve_task_tree(config, &child_id);
            child.parent = Some(task.id.clone());
            children.push(child);

            // skip past child's subtree
            let child_indent = line_indent;
            i += 1;
            while i < lines.len() {
                let next_indent = compute_line_indent(&lines[i]);
                if next_indent <= child_indent {
                    break;
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    task.children = children;
    task
}

pub fn task_get_direct_children(config: &AgendaConfig, parent_id: &str) -> Vec<Task> {
    let filename = task_filename_get(parent_id);
    let full_path = config.vault_dir.join(&filename);
    let line_nr = task_line_get(parent_id);
    let lines = file_lines_get(full_path.to_string_lossy().as_ref());

    if line_nr == 0 || line_nr > lines.len() {
        return vec![];
    }

    let parent_line = &lines[line_nr - 1];
    let parent_indent = compute_line_indent(parent_line);

    let mut children = Vec::new();
    let mut i = line_nr;

    while i < lines.len() {
        let line = &lines[i];
        let line_indent = compute_line_indent(line);

        if line_indent <= parent_indent {
            break;
        }

        let trimmed = line.trim();
        let is_bullet = trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with("+ ");

        if is_bullet && has_state(line) {
            let child_id = format!("{}:{}", filename, i + 1);
            let mut child = Task::from_string(line, &child_id);
            child.parent = Some(parent_id.to_string());
            children.push(child);

            let child_indent = line_indent;
            i += 1;
            while i < lines.len() {
                let next_indent = compute_line_indent(&lines[i]);
                if next_indent <= child_indent {
                    break;
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    children
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
    _task_ids: &HashSet<String>,
) -> Option<Task> {
    let line_nr = task_line_get(task_id);
    if line_nr == 0 || line_nr > file_lines.len() {
        return None;
    }
    let line = &file_lines[line_nr - 1];
    let task = Task::from_string(line, task_id);
    Some(task)
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

pub fn tasks_filter_by_title(tasks: &[Task], query: &str) -> Vec<Task> {
    let query_lower = query.to_lowercase();
    tasks
        .iter()
        .filter(|t| t.title.to_lowercase().contains(&query_lower))
        .cloned()
        .collect()
}

pub fn tasks_ids_get(config: &AgendaConfig) -> HashSet<String> {
    let exclude_set: HashSet<String> = HashSet::new();

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
    title: Option<&str>,
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

    if let Some(c) = title {
        task.title = c.to_string();
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
        format!("{} #{} {}", bullet, task.state, task.title)
    } else {
        format!("{}{} #{} {}", indent, bullet, task.state, task.title)
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

    task.raw = new_line;
    Ok(task)
}

pub fn task_add(config: &AgendaConfig, title: &str, parent: Option<&str>) {
    if let Some(parent_id) = parent {
        let filename = task_filename_get(parent_id);
        let lines = file_lines_get(&filename);

        let parent_line_nr = task_line_get(parent_id); // 1-based
        let parent_idx = parent_line_nr - 1; // convert to 0-based

        let parent_indent = lines[parent_idx].chars().take_while(|c| *c == ' ').count();
        let subtask_indent = parent_indent + 2;

        let line_to_insert = format!("{}* #TODO {}", " ".repeat(subtask_indent), title);

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
        let line_to_insert = format!("* #TODO {}", title);
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
    task_title: &str,
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

    let mut task_line = format!("* #{} {}", resolved_state, task_title);

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
    fn test_compute_line_indent_spaces() {
        assert_eq!(compute_line_indent("  hello"), 2);
        assert_eq!(compute_line_indent("    hello"), 4);
        assert_eq!(compute_line_indent("hello"), 0);
    }

    #[test]
    fn test_compute_line_indent_tabs() {
        assert_eq!(compute_line_indent("\thello"), 2);
        assert_eq!(compute_line_indent("\t\thello"), 4);
    }

    #[test]
    fn test_compute_line_indent_mixed() {
        assert_eq!(compute_line_indent(" \thello"), 3);
    }

    #[test]
    fn test_parse_task_body_single_child() {
        let lines = vec![
            "- Parent #TODO".to_string(),
            "  - Child #DONE".to_string(),
            "- Sibling #TODO".to_string(),
        ];
        let body = parse_task_body("test.md:1", &lines);
        assert_eq!(body, "- Child #DONE");
    }

    #[test]
    fn test_parse_task_body_multiline_body() {
        let lines = vec![
            "- Parent #TODO".to_string(),
            "  - Child #DONE".to_string(),
            "  Some note text".to_string(),
            "  > A quote".to_string(),
            "- Sibling #TODO".to_string(),
        ];
        let body = parse_task_body("test.md:1", &lines);
        assert_eq!(body, "- Child #DONE\nSome note text\n> A quote");
    }

    #[test]
    fn test_parse_task_body_stops_at_same_indent() {
        let lines = vec![
            "- Parent #TODO".to_string(),
            "  - Child #DONE".to_string(),
            "- Sibling #TODO".to_string(),
        ];
        let body = parse_task_body("test.md:1", &lines);
        assert_eq!(body, "- Child #DONE");
    }

    #[test]
    fn test_parse_task_body_stops_at_less_indent() {
        let lines = vec![
            "  - Indented parent #TODO".to_string(),
            "    - Child #DONE".to_string(),
            "- Root task #TODO".to_string(),
        ];
        let body = parse_task_body("test.md:1", &lines);
        assert_eq!(body, "- Child #DONE");
    }

    #[test]
    fn test_parse_task_body_no_body() {
        let lines = vec![
            "- Parent #TODO".to_string(),
            "- Sibling #TODO".to_string(),
        ];
        let body = parse_task_body("test.md:1", &lines);
        assert_eq!(body, "");
    }

    #[test]
    fn test_parse_task_body_end_of_file() {
        let lines = vec![
            "- Parent #TODO".to_string(),
            "  - Child #DONE".to_string(),
        ];
        let body = parse_task_body("test.md:1", &lines);
        assert_eq!(body, "- Child #DONE");
    }

    #[test]
    fn test_parse_task_body_preserves_nested_indent() {
        let lines = vec![
            "- Parent #TODO".to_string(),
            "  - Child #DONE".to_string(),
            "    - Grandchild #TODO".to_string(),
            "- Sibling #TODO".to_string(),
        ];
        let body = parse_task_body("test.md:1", &lines);
        assert_eq!(body, "- Child #DONE\n  - Grandchild #TODO");
    }

    #[test]
    fn test_parse_task_body_indented_task() {
        let lines = vec![
            "  - Parent #TODO".to_string(),
            "    - Child #DONE".to_string(),
            "    Some note".to_string(),
            "- Root #TODO".to_string(),
        ];
        let body = parse_task_body("test.md:1", &lines);
        assert_eq!(body, "- Child #DONE\nSome note");
    }

    #[test]
    fn test_task_from_string_basic() {
        let task = Task::from_string("* Buy milk #TODO @priority(1)", "test.md:1");
        assert_eq!(task.title, "Buy milk");
        assert_eq!(task.state, "TODO");
        assert_eq!(task.id, "test.md:1");
        assert!(task.tags.is_empty());
        assert_eq!(task.properties.get("priority"), Some(&"1".to_string()));
    }

    #[test]
    fn test_task_from_string_with_tags() {
        let task = Task::from_string("* Fix bug #bug #high-priority", "test.md:2");
        assert_eq!(task.title, "Fix bug");
        assert!(task.state.is_empty());
        assert!(task.tags.contains(&"bug".to_string()));
        assert!(task.tags.contains(&"high-priority".to_string()));
    }

    #[test]
    fn test_task_from_string_strips_bullet() {
        let task = Task::from_string("- Task with dash", "test.md:1");
        assert_eq!(task.title, "Task with dash");

        let task2 = Task::from_string("+ Task with plus", "test.md:2");
        assert_eq!(task2.title, "Task with plus");
    }

    #[test]
    fn test_task_from_string_strips_heading() {
        let task = Task::from_string("# Heading line #TODO", "test.md:1");
        assert_eq!(task.title, "Heading line");
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
    fn test_tasks_filter_by_tag() {
        let tasks = vec![
            Task {
                title: "Task 1".to_string(),
                state: "TODO".to_string(),
                tags: vec!["bug".to_string(), "urgent".to_string()],
                raw: "* Task 1 #TODO #bug #urgent".to_string(),
                body: String::new(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                title: "Task 2".to_string(),
                state: "TODO".to_string(),
                tags: vec!["feature".to_string()],
                raw: "* Task 2 #TODO #feature".to_string(),
                body: String::new(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                title: "Task 3".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task 3 #TODO".to_string(),
                body: String::new(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let filtered = tasks_filter_by_tag(&tasks, "bug");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Task 1");

        let filtered_feature = tasks_filter_by_tag(&tasks, "feature");
        assert_eq!(filtered_feature.len(), 1);
        assert_eq!(filtered_feature[0].title, "Task 2");

        let filtered_no_tag = tasks_filter_by_tag(&tasks, "nonexistent");
        assert!(filtered_no_tag.is_empty());
    }

    #[test]
    fn test_tasks_filter_by_property() {
        let tasks = vec![
            Task {
                title: "Task 1".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task 1 #TODO @scheduled(2024-01-15)".to_string(),
                body: String::new(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2024-01-15".to_string())]),
            },
            Task {
                title: "Task 2".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task 2 #TODO @priority(1)".to_string(),
                body: String::new(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "1".to_string())]),
            },
            Task {
                title: "Task 3 with datetime".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task 3 #TODO @scheduled(2026-04-09T14:00)".to_string(),
                body: String::new(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([(
                    "scheduled".to_string(),
                    "2026-04-09T14:00".to_string(),
                )]),
            },
            Task {
                title: "Task 4 with date only".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task 4 #TODO @scheduled(2026-04-09)".to_string(),
                body: String::new(),
                id: "test.md:4".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2026-04-09".to_string())]),
            },
        ];

        let filtered = tasks_filter_by_property(&tasks, "scheduled", None);
        assert_eq!(filtered.len(), 3);
        assert!(filtered.iter().any(|t| t.title == "Task 1"));
        assert!(filtered.iter().any(|t| t.title == "Task 3 with datetime"));
        assert!(filtered
            .iter()
            .any(|t| t.title == "Task 4 with date only"));

        let filtered_with_value = tasks_filter_by_property(&tasks, "scheduled", Some("2024-01-15"));
        assert_eq!(filtered_with_value.len(), 1);
        assert_eq!(filtered_with_value[0].title, "Task 1");

        let filtered_wrong_value =
            tasks_filter_by_property(&tasks, "scheduled", Some("2025-01-01"));
        assert!(filtered_wrong_value.is_empty());

        let filtered_with_datetime_value =
            tasks_filter_by_property(&tasks, "scheduled", Some("2026-04-09"));
        assert_eq!(filtered_with_datetime_value.len(), 2);
        assert!(filtered_with_datetime_value
            .iter()
            .any(|t| t.title == "Task 3 with datetime"));
        assert!(filtered_with_datetime_value
            .iter()
            .any(|t| t.title == "Task 4 with date only"));

        let filtered_priority = tasks_filter_by_property(&tasks, "priority", None);
        assert_eq!(filtered_priority.len(), 1);
        assert_eq!(filtered_priority[0].title, "Task 2");

        let filtered_none = tasks_filter_by_property(&tasks, "nonexistent", None);
        assert!(filtered_none.is_empty());
    }

    #[test]
    fn test_tasks_filter_by_states() {
        let tasks = vec![
            Task {
                title: "Task 1".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task 1 #TODO".to_string(),
                body: String::new(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                title: "Task 2".to_string(),
                state: "DONE".to_string(),
                tags: vec![],
                raw: "* Task 2 #DONE".to_string(),
                body: String::new(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                title: "Task 3".to_string(),
                state: "IN_PROGRESS".to_string(),
                tags: vec![],
                raw: "* Task 3 #IN_PROGRESS".to_string(),
                body: String::new(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                title: "Task 4".to_string(),
                state: "CANCELLED".to_string(),
                tags: vec![],
                raw: "* Task 4 #CANCELLED".to_string(),
                body: String::new(),
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
                title: "Bug task".to_string(),
                state: "TODO".to_string(),
                tags: vec!["bug".to_string()],
                raw: "* Bug task #TODO #bug @priority(1)".to_string(),
                body: String::new(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "1".to_string())]),
            },
            Task {
                title: "Feature task".to_string(),
                state: "DONE".to_string(),
                tags: vec!["feature".to_string()],
                raw: "* Feature task #DONE #feature @priority(2)".to_string(),
                body: String::new(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "2".to_string())]),
            },
            Task {
                title: "Another bug".to_string(),
                state: "TODO".to_string(),
                tags: vec!["bug".to_string()],
                raw: "* Another bug #TODO #bug".to_string(),
                body: String::new(),
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
                title: "Task A".to_string(),
                state: "DONE".to_string(),
                tags: vec![],
                raw: "* Task A #DONE".to_string(),
                body: String::new(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                title: "Task B".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task B #TODO".to_string(),
                body: String::new(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                title: "Task C".to_string(),
                state: "IN_PROGRESS".to_string(),
                tags: vec![],
                raw: "* Task C #IN_PROGRESS".to_string(),
                body: String::new(),
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
                title: "Task A".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task A #TODO @priority(2)".to_string(),
                body: String::new(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "2".to_string())]),
            },
            Task {
                title: "Task B".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task B #TODO @priority(1)".to_string(),
                body: String::new(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "1".to_string())]),
            },
            Task {
                title: "Task C".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task C #TODO @priority(1)".to_string(),
                body: String::new(),
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
                title: "Task A".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task A #TODO @scheduled(2024-02-01)".to_string(),
                body: String::new(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2024-02-01".to_string())]),
            },
            Task {
                title: "Task B".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task B #TODO @scheduled(2024-01-15)".to_string(),
                body: String::new(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("scheduled".to_string(), "2024-01-15".to_string())]),
            },
        ];

        tasks_sort_by(&mut tasks, &["scheduled".to_string()]);

        assert_eq!(tasks[0].title, "Task B");
        assert_eq!(tasks[1].title, "Task A");
    }

    #[test]
    fn test_tasks_filter_by_title() {
        let tasks = vec![
            Task {
                title: "Buy groceries".to_string(),
                state: "TODO".to_string(),
                tags: vec!["shopping".to_string()],
                raw: "* Buy groceries #TODO #shopping".to_string(),
                body: String::new(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                title: "Call mom".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Call mom #TODO".to_string(),
                body: String::new(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                title: "Buy milk".to_string(),
                state: "TODO".to_string(),
                tags: vec!["shopping".to_string()],
                raw: "* Buy milk #TODO #shopping".to_string(),
                body: String::new(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
        ];

        let filtered = tasks_filter_by_title(&tasks, "buy");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|t| t.title.to_lowercase().contains("buy")));

        let filtered_case = tasks_filter_by_title(&tasks, "BUY");
        assert_eq!(filtered_case.len(), 2);

        let filtered_none = tasks_filter_by_title(&tasks, "xyz");
        assert!(filtered_none.is_empty());
    }

    #[test]
    fn test_tasks_filter_multiple_criteria_and_logic() {
        let tasks = vec![
            Task {
                title: "Task with bug and priority".to_string(),
                state: "TODO".to_string(),
                tags: vec!["bug".to_string()],
                raw: "* Task with bug and priority #TODO #bug @priority(high)".to_string(),
                body: String::new(),
                id: "test.md:1".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "high".to_string())]),
            },
            Task {
                title: "Task with bug only".to_string(),
                state: "TODO".to_string(),
                tags: vec!["bug".to_string()],
                raw: "* Task with bug only #TODO #bug".to_string(),
                body: String::new(),
                id: "test.md:2".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::new(),
            },
            Task {
                title: "Task with priority only".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Task with priority only #TODO @priority(high)".to_string(),
                body: String::new(),
                id: "test.md:3".to_string(),
                parent: None,
                children: vec![],
                properties: IndexMap::from([("priority".to_string(), "high".to_string())]),
            },
            Task {
                title: "Regular task".to_string(),
                state: "TODO".to_string(),
                tags: vec![],
                raw: "* Regular task #TODO".to_string(),
                body: String::new(),
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
        assert_eq!(filtered_and[0].title, "Task with bug and priority");
    }
}
