use crate::fs_utils::{file_line_insert, file_lines_get};
use crate::grep::{TASK_STATES, tasks_grep};
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use std::cmp::PartialEq;
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

pub fn task_get_by_id(task_id: &str) -> Task {
    let task_ids = tasks_ids_get();

    let filename = task_filename_get(&task_id);
    let lines = file_lines_get(&filename);
    task_get_from_lines(task_id, &lines, &task_ids).expect("HANDLE THIS - RETURN OPTION") // TODO:
}

pub fn task_parent_get(child_id: String) -> Option<Task> {
    let filename = task_filename_get(&child_id);
    let line_nr = task_line_get(&child_id);
    let lines = file_lines_get(&filename);
    let task_ids = tasks_ids_get();

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
                return Some(task_get_by_id(&potential_parent_id));
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

fn flatten_tasks(tasks: &[Task]) -> Vec<&Task> {
    let mut flat = Vec::new();
    let mut stack: Vec<&Task> = tasks.iter().collect();

    while let Some(task) = stack.pop() {
        flat.push(task);
        for child in task.children.iter().rev() {
            stack.push(child);
        }
    }

    flat
}

fn tasks_refs_to_owned(refs: Vec<&Task>) -> Vec<Task> {
    refs.into_iter().map(|t| (*t).clone()).collect()
}

pub fn tasks_group_date(tasks: &[Task]) -> IndexMap<String, Vec<&Task>> {
    fn collect_scheduled<'a>(
        task: &'a Task,
        grouped: &mut IndexMap<String, Vec<&'a Task>>,
        seen: &mut IndexMap<String, HashSet<&'a str>>,
    ) {
        if let Some(date) = task.properties.get("scheduled") {
            let key = date.split('T').next().unwrap_or(date).to_string();
            let seen_set = seen.entry(key.clone()).or_insert_with(HashSet::new);

            // Only add the task if neither it nor any ancestor is already included
            if !seen_set.contains(task.id.as_str()) {
                grouped
                    .entry(key.clone())
                    .or_insert_with(Vec::new)
                    .push(task);

                // Mark all descendants as seen for this date
                let mut stack = vec![task];
                while let Some(t) = stack.pop() {
                    seen_set.insert(t.id.as_str());
                    for c in &t.children {
                        stack.push(c);
                    }
                }
            }
        }

        // Recurse into children
        for child in &task.children {
            collect_scheduled(child, grouped, seen);
        }
    }

    let mut grouped: IndexMap<String, Vec<&Task>> = IndexMap::new();
    let mut seen: IndexMap<String, HashSet<&str>> = IndexMap::new();

    for task in tasks {
        collect_scheduled(task, &mut grouped, &mut seen);
    }

    grouped
}
pub fn tasks_group_tag(tasks: &[Task]) -> IndexMap<String, Vec<&Task>> {
    fn collect_tags<'a>(
        task: &'a Task,
        grouped: &mut IndexMap<String, Vec<&'a Task>>,
        seen: &mut IndexMap<String, HashSet<&'a str>>,
    ) {
        let tags: Vec<&str> = if task.tags.is_empty() {
            vec!["NO TAG"]
        } else {
            task.tags.iter().map(|s| s.as_str()).collect()
        };

        for tag in tags {
            let seen_set = seen.entry(tag.to_string()).or_insert_with(HashSet::new);

            // Only add if task (or ancestor) hasn't been added for this tag
            if !seen_set.contains(task.id.as_str()) {
                grouped
                    .entry(tag.to_string())
                    .or_insert_with(Vec::new)
                    .push(task);

                // Mark all descendants as seen for this tag
                let mut stack = vec![task];
                while let Some(t) = stack.pop() {
                    seen_set.insert(t.id.as_str());
                    for c in &t.children {
                        stack.push(c);
                    }
                }
            }
        }

        // Recurse into children
        for child in &task.children {
            collect_tags(child, grouped, seen);
        }
    }

    let mut grouped: IndexMap<String, Vec<&Task>> = IndexMap::new();
    let mut seen: IndexMap<String, HashSet<&str>> = IndexMap::new();

    for task in tasks {
        collect_tags(task, &mut grouped, &mut seen);
    }

    grouped
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
    line.trim_start().starts_with("* ")
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

pub fn tasks_get() -> Vec<Task> {
    let task_ids = tasks_ids_get();
    let grouped = tasks_group_by_filename(&task_ids);

    let mut res: Vec<Task> = vec![];

    let mut filenames: Vec<_> = grouped.keys().cloned().collect();
    filenames.sort();

    for filename in filenames {
        let tasks = &grouped[&filename];
        let lines = file_lines_get(&filename);

        let mut sorted_tasks = tasks.clone();
        sorted_tasks.sort_by_key(|id| task_line_get(id));

        for t_id in sorted_tasks {
            let t = task_get_from_lines(t_id, &lines, &task_ids);
            if let Some(task) = t {
                res.push(task);
            }
        }
    }

    res
}

// sorts by `scheduled` and `priority` properties currently
// TODO: make generic - should take an array of properties to sort by
pub fn tasks_sort(tasks: &mut Vec<Task>) {
    tasks.sort_by(|a, b| {
        let normalize_date = |s: &str| {
            if s.contains('T') {
                s.to_string()
            } else {
                format!("{}TZZ:ZZ", s)
            }
        };

        let a_date = a.properties.get("scheduled");
        let b_date = b.properties.get("scheduled");

        match (a_date, b_date) {
            (Some(ad), Some(bd)) => {
                let ord = normalize_date(ad).cmp(&normalize_date(bd));
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            (Some(_), None) => return std::cmp::Ordering::Less,
            (None, Some(_)) => return std::cmp::Ordering::Greater,
            _ => {}
        }

        let a_pri = a.properties.get("priority");
        let b_pri = b.properties.get("priority");

        match (a_pri, b_pri) {
            (Some(ap), Some(bp)) => ap.cmp(bp),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        }
    });
}

pub fn tasks_ids_get() -> HashSet<String> {
    let mut exclude_set = HashSet::new();
    exclude_set.insert("DONE".to_string());
    exclude_set.insert("CANCELLED".to_string());

    let rg_result = tasks_grep(Some(&exclude_set));
    let rg_result = match rg_result {
        Ok(output) => output,
        Err(_) => "".to_string(),
    };

    let r = rg_result
        .lines()
        .filter_map(|line| {
            // split into filename, line number, and the rest
            let mut parts = line.splitn(3, ':');
            let filename = parts.next()?;
            let line_nr = parts.next()?;
            Some(format!("{}:{}", filename, line_nr))
        })
        .collect();
    r
}

pub fn task_line_set(id: String, line: String) -> String {
    let filename = task_filename_get(&id);

    let line_nr = task_line_get(&id);
    let mut lines = file_lines_get(&filename);

    lines[line_nr - 1] = line;

    let mut file = File::create(&filename).unwrap();
    for line in lines {
        writeln!(file, "{}", line).unwrap();
    }

    "dobre pomaranczowe".to_string()
}

pub fn task_add(content: &str, parent: Option<&str>) {
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
        let agenda_file = "/home/kuba/Documents/brain-vault/agenda/agenda.md";
        file_line_insert(agenda_file, None, &line_to_insert).expect("failed to add task");
    }
}

pub fn task_change_state(task_id: &str, new_state: &str) -> Result<(), String> {
    let filename = task_filename_get(task_id);
    let line_nr = task_line_get(task_id);
    let mut lines = file_lines_get(&filename);

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
    let mut file = File::create(&filename).map_err(|e| e.to_string())?;
    for l in lines {
        writeln!(file, "{}", l).map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn task_change_property(task_id: &str, key: &str, value: Option<&str>) -> Result<(), String> {
    let filename = task_filename_get(task_id);
    let line_nr = task_line_get(task_id);
    let mut lines = file_lines_get(&filename);

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

    let mut file = File::create(&filename).map_err(|e| e.to_string())?;
    for l in lines {
        writeln!(file, "{}", l).map_err(|e| e.to_string())?;
    }

    Ok(())
}
