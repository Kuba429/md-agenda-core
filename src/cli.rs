use crate::backlinks::run as run_backlinks;
use crate::config::AgendaConfig;
use crate::fs_utils::file_lines_get;
use crate::grep::task_state_next;
use crate::grep::task_state_prev;
use crate::repository::MarkdownTaskRepository;
use crate::task::task_add;
use crate::task::task_capture;
use crate::task::task_change_property;
use crate::task::task_change_state;
use crate::task::task_parent_get;
use crate::task::tasks_filter_by_property;
use crate::task::tasks_filter_by_states;
use crate::task::tasks_filter_by_states_included;
use crate::task::tasks_filter_by_tag;
use crate::task::tasks_filter_by_content;
use crate::task::{
    task_filename_get, task_get_by_id, task_line_get, task_line_set, task_set_fields, tasks_get,
    tasks_group_by_property, Task,
};
use crate::task::{tasks_sort, tasks_sort_by};
use clap::Parser;
use clap::Subcommand;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "agenda-core")]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long)]
    pub vault_dir: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Get {
        #[arg(short, long)]
        group: Option<String>,
        #[arg(short, long)]
        task_id: Option<String>,
        #[arg(long)]
        next_state: Option<String>,
        #[arg(long)]
        previous_state: Option<String>,
        #[arg(long)]
        parent_of: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long, num_args = 1..)]
        property: Option<Vec<String>>,
        #[arg(long, num_args = 1..)]
        exclude_state: Option<Vec<String>>,
        #[arg(long, num_args = 1..)]
        state: Option<Vec<String>>,
        #[arg(long, num_args = 1..)]
        sort: Option<Vec<String>>,
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        include_children: bool,
        #[arg(long)]
        include_ancestors: bool,
    },
    Set {
        #[arg(short, long)]
        id: Option<String>,
        #[arg(short, long)]
        line: Option<String>,
        #[arg(short, long)]
        content: Option<String>,
        #[arg(short, long)]
        state: Option<String>,
        #[arg(long, num_args = 1..)]
        property: Option<Vec<String>>,
        #[arg(long, num_args = 1..)]
        tag: Option<Vec<String>>,
        #[arg(long)]
        remove_tags: bool,
        #[arg(long)]
        remove_properties: bool,
    },
    Add {
        #[arg(short, long)]
        content: String,
        #[arg(short, long)]
        task_id: Option<String>,
    },
    Change {
        #[arg(short, long)]
        task_id: String,
        #[arg(short, long)]
        state: Option<String>,
        #[arg(long, num_args = 1..=2)]
        property: Option<Vec<String>>,
    },
    Backlinks {
        #[arg(index = 1)]
        target: Option<String>,
    },
    Capture {
        #[arg(short, long)]
        content: String,
        #[arg(short, long)]
        target: Option<String>,
        #[arg(short, long)]
        state: Option<String>,
        #[arg(long, num_args = 1..)]
        property: Option<Vec<String>>,
        #[arg(long, num_args = 1..)]
        tag: Option<Vec<String>>,
    },
}

fn get_vault_dir(args: &Args) -> PathBuf {
    if let Some(ref dir) = args.vault_dir {
        PathBuf::from(dir)
    } else if let Ok(dir) = std::env::var("AGENDA_VAULT_DIR") {
        PathBuf::from(dir)
    } else {
        PathBuf::from(".")
    }
}

fn get_task_children(config: &AgendaConfig, task_id: &str) -> Vec<Task> {
    let filename = task_filename_get(task_id);
    let full_path = config.vault_dir.join(&filename);
    let line_nr = task_line_get(task_id);
    let lines = file_lines_get(full_path.to_string_lossy().as_ref());

    if line_nr == 0 || line_nr > lines.len() {
        return vec![];
    }

    let current_line = &lines[line_nr - 1];
    let current_indent = current_line.chars().take_while(|c| *c == ' ').count();

    let mut children = vec![];
    let mut i = line_nr;

    while i < lines.len() {
        let next_line = &lines[i];
        let trimmed = next_line.trim();

        if trimmed.starts_with('#') {
            break;
        }

        let next_indent = next_line.chars().take_while(|c| *c == ' ').count();
        if next_indent <= current_indent {
            break;
        }

        let is_bullet =
            trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ");
        let has_state = trimmed.contains("#TODO")
            || trimmed.contains("#IN_PROGRESS")
            || trimmed.contains("#DONE")
            || trimmed.contains("#NEXT")
            || trimmed.contains("#WAIT")
            || trimmed.contains("#LATER");

        if is_bullet && has_state {
            let child_id = format!("{}:{}", filename, i + 1);
            let child = Task::from_string(next_line, &child_id);
            let grandchildren = get_task_children(config, &child_id);
            children.push(Task {
                children: grandchildren,
                ..child
            });

            let child_indent = next_indent;
            i += 1;
            while i < lines.len() {
                let line = &lines[i];
                if line.trim().starts_with('#') {
                    break;
                }
                if line.chars().take_while(|c| *c == ' ').count() <= child_indent {
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

pub(crate) fn collect_child_ids(task: &Task, ids: &mut HashSet<String>) {
    for child in &task.children {
        ids.insert(child.id.clone());
        collect_child_ids(child, ids);
    }
}

pub(crate) fn get_task_ancestors(config: &AgendaConfig, task: &Task) -> Task {
    let filename = task_filename_get(&task.id);
    let full_path = config.vault_dir.join(&filename);
    let lines = file_lines_get(full_path.to_string_lossy().as_ref());
    let line_nr = task_line_get(&task.id);

    if line_nr == 0 || line_nr > lines.len() {
        return task.clone();
    }

    let current_line = &lines[line_nr - 1];
    let current_indent = current_line.chars().take_while(|c| *c == ' ').count();

    let mut ancestor_stack: Vec<(usize, String, String)> = Vec::new();

    for i in (0..line_nr - 1).rev() {
        let line = &lines[i];
        let trimmed = line.trim();

        if trimmed.starts_with('#') {
            break;
        }

        if let Some(&(last_indent, _, _)) = ancestor_stack.last() {
            if last_indent <= line.chars().take_while(|c| *c == ' ').count() {
                continue;
            }
        }

        let indent = line.chars().take_while(|c| *c == ' ').count();
        if indent >= current_indent {
            continue;
        }

        let is_bullet =
            trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ");
        let has_state = trimmed.contains("#TODO")
            || trimmed.contains("#IN_PROGRESS")
            || trimmed.contains("#DONE")
            || trimmed.contains("#NEXT")
            || trimmed.contains("#WAIT")
            || trimmed.contains("#LATER");

        if is_bullet && has_state {
            let ancestor_id = format!("{}:{}", filename, i + 1);
            ancestor_stack.push((indent, ancestor_id, line.clone()));
        }
    }

    if ancestor_stack.is_empty() {
        return task.clone();
    }

    let mut result = task.clone();

    for (_, ancestor_id, ancestor_line) in ancestor_stack {
        let ancestor = Task::from_string(&ancestor_line, &ancestor_id);
        let mut wrapped = ancestor;
        wrapped.children = vec![result];
        result = wrapped;
    }

    result
}

fn line_number_from_id(id: &str) -> usize {
    id.rsplitn(2, ':')
        .next()
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(0)
}

pub(crate) fn merge_task_trees(tasks: Vec<Task>) -> Vec<Task> {
    let mut root_map: std::collections::HashMap<String, Task> = std::collections::HashMap::new();
    let mut root_order: Vec<String> = Vec::new();

    for task in tasks {
        if let Some(existing) = root_map.get_mut(&task.id) {
            merge_children(existing, task.children);
        } else {
            root_order.push(task.id.clone());
            root_map.insert(task.id.clone(), task);
        }
    }

    root_order
        .into_iter()
        .filter_map(|id| root_map.remove(&id))
        .collect()
}

fn merge_children(parent: &mut Task, new_children: Vec<Task>) {
    for child in new_children {
        if let Some(existing) = parent.children.iter_mut().find(|c| c.id == child.id) {
            merge_children(existing, child.children);
        } else {
            let insert_pos = parent
                .children
                .iter()
                .position(|c| line_number_from_id(&c.id) > line_number_from_id(&child.id))
                .unwrap_or(parent.children.len());
            parent.children.insert(insert_pos, child);
        }
    }
}

fn handle_get_by_id(config: &AgendaConfig, id: &str) -> Value {
    json!({ "task": task_get_by_id(config, id) })
}

fn handle_get_grouped(config: &AgendaConfig, group: &str, sort: &Option<Vec<String>>) -> Value {
    let mut tasks = tasks_get(config);
    sort_or_default(&mut tasks, sort);
    match group {
        "date" => json!(tasks_group_by_property(&tasks, "scheduled")),
        "tag" => json!(tasks_group_by_property(&tasks, "tag")),
        _ => json!(tasks_group_by_property(&tasks, group)),
    }
}

fn handle_get_next_state(state: &str) -> Value {
    serde_json::Value::String(task_state_next(state))
}

fn handle_get_previous_state(state: &str) -> Value {
    serde_json::Value::String(task_state_prev(state))
}

fn handle_get_parent(config: &AgendaConfig, child_id: &str) -> Value {
    json!({ "task": task_parent_get(config, child_id.to_string()) })
}

fn sort_or_default(tasks: &mut Vec<Task>, sort: &Option<Vec<String>>) {
    if let Some(sort_criteria) = sort {
        tasks_sort_by(tasks, sort_criteria);
    } else {
        tasks_sort(tasks);
    }
}

fn parse_property_filter(property: &Option<Vec<String>>) -> Option<(&str, Option<&str>)> {
    property.as_ref().map(|prop| {
        let parts: Vec<&str> = prop.iter().map(|s| s.as_str()).collect();
        let full = parts.first().map(|s| *s).unwrap();
        let kv: Vec<&str> = full.splitn(2, '=').collect();
        let key = kv[0];
        let value = if kv.len() > 1 { Some(kv[1]) } else { None };
        (key, value)
    })
}

fn handle_get_filtered_tasks(
    config: &AgendaConfig,
    state: &Option<Vec<String>>,
    exclude_state: &Option<Vec<String>>,
    tag: &Option<String>,
    property: &Option<Vec<String>>,
    sort: &Option<Vec<String>>,
    content: &Option<String>,
    include_children: bool,
    include_ancestors: bool,
) -> Value {
    let repo = MarkdownTaskRepository::new(config.clone());

    let include_states: Option<HashSet<String>> =
        state.as_ref().map(|s| s.iter().cloned().collect());
    let exclude_states: Option<HashSet<String>> =
        exclude_state.as_ref().map(|s| s.iter().cloned().collect());
    let filter_tag = tag.as_deref();
    let filter_property = parse_property_filter(property);

    let tasks = repo
        .load_tasks_with_filter(
            include_states.as_ref(),
            exclude_states.as_ref(),
            filter_tag,
            filter_property,
        )
        .unwrap_or_default();

    let mut tasks = tasks;
    sort_or_default(&mut tasks, sort);

    if let Some(states) = exclude_state {
        tasks = tasks_filter_by_states(&tasks, states);
    }
    if let Some(states) = state {
        tasks = tasks_filter_by_states_included(&tasks, states);
    }
    if let Some(t) = tag {
        tasks = tasks_filter_by_tag(&tasks, t);
    }
    if let Some((key, value)) = filter_property {
        tasks = tasks_filter_by_property(&tasks, key, value);
    }
    if let Some(c) = content {
        tasks = tasks_filter_by_content(&tasks, c);
    }

    if include_children {
        for task in &mut tasks {
            task.children = get_task_children(config, &task.id);
        }
    }

    if include_ancestors {
        let mut tasks_with_ancestors: Vec<Task> = vec![];
        for task in &tasks {
            let rooted = get_task_ancestors(config, task);
            tasks_with_ancestors.push(rooted);
        }
        tasks = merge_task_trees(tasks_with_ancestors);

        let mut non_roots: HashSet<String> = HashSet::new();
        for task in &tasks {
            collect_child_ids(task, &mut non_roots);
        }
        tasks = tasks
            .into_iter()
            .filter(|t| !non_roots.contains(&t.id))
            .collect();
    }

    if include_children && !include_ancestors {
        let mut non_roots: HashSet<String> = HashSet::new();
        for task in &tasks {
            collect_child_ids(task, &mut non_roots);
        }
        tasks = tasks
            .into_iter()
            .filter(|t| !non_roots.contains(&t.id))
            .collect();
    }

    json!(tasks)
}

fn handle_set(
    config: &AgendaConfig,
    id: Option<&str>,
    line: Option<&str>,
    content: Option<&str>,
    state: Option<&str>,
    property: &Option<Vec<String>>,
    tag: &Option<Vec<String>>,
    remove_tags: bool,
    remove_properties: bool,
) -> Value {
    let has_property = property.is_some();
    let has_tag = tag.is_some();
    let has_line = line.is_some();
    let has_fields = content.is_some()
        || state.is_some()
        || has_property
        || has_tag
        || remove_tags
        || remove_properties;

    if has_line && has_fields {
        return json!({"code": 400, "error": "Cannot use --line with field arguments (--content, --state, --property, --tag). They are mutually exclusive."});
    }

    let id = match id {
        Some(id) => id,
        None => return json!({"code": 400, "error": "Missing required argument: --id"}),
    };

    if has_line {
        let res = task_line_set(config, id, line.unwrap());
        return json!({"code": 200, "newLine": res});
    }

    if !has_fields {
        return json!({"code": 400, "error": "Either --line or at least one field argument (--content, --state, --property, --tag) is required."});
    }

    let properties: std::collections::HashMap<String, String> = property
        .as_ref()
        .map(|props| {
            props
                .iter()
                .filter_map(|p| {
                    if p.contains('=') {
                        let kv: Vec<&str> = p.splitn(2, '=').collect();
                        if kv.len() == 2 {
                            return Some((kv[0].to_string(), kv[1].to_string()));
                        }
                    } else if p.contains('(') && p.contains(')') {
                        let re = regex::Regex::new(r"(\w+)\(([^)]+)\)").unwrap();
                        if let Some(caps) = re.captures(p) {
                            return Some((caps[1].to_string(), caps[2].to_string()));
                        }
                    }
                    None
                })
                .collect()
        })
        .unwrap_or_default();

    let tags: Option<Vec<String>> = tag.as_ref().map(|t| t.to_vec());

    match task_set_fields(
        config,
        id,
        content,
        state,
        if properties.is_empty() { None } else { Some(&properties) },
        tags.as_deref(),
        remove_tags,
        remove_properties,
    ) {
        Ok(task) => json!({"code": 200, "message": "task updated", "task": task}),
        Err(e) => json!({"code": 400, "error": e}),
    }
}

fn handle_add(config: &AgendaConfig, content: &str, task_id: Option<&str>) -> Value {
    task_add(config, content, task_id);
    json!({"code": 200, "message": "task added"})
}

fn handle_change(
    config: &AgendaConfig,
    task_id: &str,
    state: Option<&str>,
    property: Option<&Vec<String>>,
) -> Value {
    if let Some(s) = state {
        if let Err(e) = task_change_state(config, task_id, s) {
            return json!({"code": 400, "error": e});
        }
    }
    if let Some(prop) = property {
        let key = &prop[0];
        let value = if prop.len() > 1 { Some(&prop[1]) } else { None };
        if let Err(e) = task_change_property(config, task_id, key, value.cloned().as_deref()) {
            return json!({"code": 400, "error": e});
        }
    }
    let updated = task_get_by_id(config, task_id);
    json!({"code": 200, "task": updated})
}

fn handle_capture(
    config: &AgendaConfig,
    content: &str,
    target: Option<&str>,
    state: Option<&str>,
    property: &Option<Vec<String>>,
    tag: &Option<Vec<String>>,
) -> Value {
    let target = target.unwrap_or("capture.md");

    let tags: Vec<String> = tag.as_ref().map(|t| t.to_vec()).unwrap_or_default();
    let properties: std::collections::HashMap<String, String> = property
        .as_ref()
        .map(|props| {
            props
                .iter()
                .filter_map(|p| {
                    if p.contains('=') {
                        let kv: Vec<&str> = p.splitn(2, '=').collect();
                        if kv.len() == 2 {
                            return Some((kv[0].to_string(), kv[1].to_string()));
                        }
                    } else if p.contains('(') && p.contains(')') {
                        let re = regex::Regex::new(r"(\w+)\(([^)]+)\)").unwrap();
                        if let Some(caps) = re.captures(p) {
                            return Some((caps[1].to_string(), caps[2].to_string()));
                        }
                    }
                    None
                })
                .collect()
        })
        .unwrap_or_default();

    match task_capture(config, content, target, state, &properties, &tags) {
        Ok(task_id) => json!({"code": 200, "message": "task captured", "taskId": task_id}),
        Err(e) => json!({"code": 400, "error": e}),
    }
}

pub fn get_output() -> Value {
    let args = Args::parse();
    let config = AgendaConfig::new(get_vault_dir(&args));

    match &args.command {
        Commands::Get {
            group,
            task_id,
            next_state,
            previous_state,
            parent_of,
            tag,
            property,
            exclude_state,
            state,
            sort,
            content,
            include_children,
            include_ancestors,
        } => {
            if let Some(id) = task_id {
                handle_get_by_id(&config, id)
            } else if let Some(group) = group {
                handle_get_grouped(&config, group, sort)
            } else if let Some(ns) = next_state {
                handle_get_next_state(ns)
            } else if let Some(ps) = previous_state {
                handle_get_previous_state(ps)
            } else if let Some(cid) = parent_of {
                handle_get_parent(&config, cid)
            } else {
                handle_get_filtered_tasks(
                    &config,
                    state,
                    exclude_state,
                    tag,
                    property,
                    sort,
                    content,
                    *include_children,
                    *include_ancestors,
                )
            }
        }
        Commands::Set {
            id,
            line,
            content,
            state,
            property,
            tag,
            remove_tags,
            remove_properties,
        } => handle_set(
            &config,
            id.as_deref(),
            line.as_deref(),
            content.as_deref(),
            state.as_deref(),
            property,
            tag,
            *remove_tags,
            *remove_properties,
        ),
        Commands::Add { content, task_id } => handle_add(&config, content, task_id.as_deref()),
        Commands::Change {
            task_id,
            state,
            property,
        } => handle_change(&config, task_id, state.as_deref(), property.as_ref()),
        Commands::Backlinks { target } => {
            run_backlinks(&config, target.as_deref());
            json!({"code": 200, "message": "backlinks generated"})
        }
        Commands::Capture {
            content,
            target,
            state,
            property,
            tag,
        } => handle_capture(&config, content, target.as_deref(), state.as_deref(), property, tag),
    }
}
