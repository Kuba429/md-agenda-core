use crate::backlinks::run as run_backlinks;
use crate::config::AgendaConfig;
use crate::grep::task_state_next;
use crate::grep::task_state_prev;
use crate::repository::MarkdownTaskRepository;
use crate::task::task_add;
use crate::task::task_capture;
use crate::task::task_change_property;
use crate::task::task_change_state;
use crate::task::tasks_filter_by_property;
use crate::task::tasks_filter_by_states;
use crate::task::tasks_filter_by_states_included;
use crate::task::tasks_filter_by_tag;
use crate::task::tasks_filter_by_title;
use crate::task::{
    task_get_by_id, task_get_direct_children, task_line_set, task_set_fields, resolve_task_tree, Task,
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
        task_id: Option<String>,
        #[arg(long)]
        next_state: Option<String>,
        #[arg(long)]
        previous_state: Option<String>,
        #[arg(long)]
        parent_of: Option<String>,
        #[arg(long)]
        parent: Option<String>,
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
        title: Option<String>,
        #[arg(long)]
        include_children: bool,
        #[arg(long)]
        include_ancestors: bool,
        #[arg(long, allow_hyphen_values = true)]
        query: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        query_debug: Option<String>,
    },
    Set {
        #[arg(short, long)]
        id: Option<String>,
        #[arg(short, long)]
        line: Option<String>,
        #[arg(short, long)]
        title: Option<String>,
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
        title: String,
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
        title: String,
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

fn handle_get_by_id(config: &AgendaConfig, id: &str) -> Value {
    json!({ "task": task_get_by_id(config, id) })
}

fn handle_get_next_state(state: &str) -> Value {
    serde_json::Value::String(task_state_next(state))
}

fn handle_get_previous_state(state: &str) -> Value {
    serde_json::Value::String(task_state_prev(state))
}

fn handle_get_parent() -> Value {
    // TODO: Temporarily neutered during parsing refactor. Needs reimplementation using resolve_task_tree.
    json!({ "task": null })
}

fn collect_child_ids(task: &Task, ids: &mut HashSet<String>) {
    for child in &task.children {
        ids.insert(child.id.clone());
        collect_child_ids(child, ids);
    }
}

fn handle_get_parent_children(
    config: &AgendaConfig,
    parent_id: &str,
    state: &Option<Vec<String>>,
    exclude_state: &Option<Vec<String>>,
    tag: &Option<String>,
    property: &Option<Vec<String>>,
    title: &Option<String>,
    query: &Option<String>,
) -> Value {
    let mut tasks = task_get_direct_children(config, &parent_id);

    if let Some(states) = exclude_state {
        tasks = tasks_filter_by_states(&tasks, states);
    }
    if let Some(states) = state {
        tasks = tasks_filter_by_states_included(&tasks, states);
    }
    if let Some(t) = tag {
        tasks = tasks_filter_by_tag(&tasks, t);
    }
    if let Some((key, value)) = parse_property_filter(property) {
        tasks = tasks_filter_by_property(&tasks, key, value);
    }
    if let Some(t) = title {
        tasks = tasks_filter_by_title(&tasks, t);
    }
    if let Some(q) = query.as_ref() {
        match crate::query::parse_and_filter(q, &tasks) {
            Ok(filtered) => tasks = filtered,
            Err(e) => {
                return json!({"code": 400, "error": e.to_string()});
            }
        }
    }

    json!(tasks)
}

fn sort_or_default(tasks: &mut Vec<Task>, sort: &Option<Vec<String>>) {
    if let Some(sort_criteria) = sort {
        tasks_sort_by(tasks, sort_criteria);
    } else {
        tasks_sort(tasks);
    }
}

fn handle_query_debug(query: &str) -> Value {
    use crate::query::{analyze_query, parse_query, strategy_regex};

    match parse_query(query) {
        Ok(expr) => {
            let strategy = analyze_query(&expr);
            let regex = strategy_regex(&strategy);
            json!({
                "ast": expr,
                "strategy": strategy,
                "regex": regex,
            })
        }
        Err(e) => json!({"code": 400, "error": e.to_string()}),
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

#[allow(deprecated)]
fn handle_get_filtered_tasks(
    config: &AgendaConfig,
    state: &Option<Vec<String>>,
    exclude_state: &Option<Vec<String>>,
    tag: &Option<String>,
    property: &Option<Vec<String>>,
    sort: &Option<Vec<String>>,
    title: &Option<String>,
    include_children: bool,
    include_ancestors: bool,
    query: &Option<String>,
) -> Value {
    let repo = MarkdownTaskRepository::new(config.clone());

    let include_states: Option<HashSet<String>> =
        state.as_ref().map(|s| s.iter().cloned().collect());
    let exclude_states: Option<HashSet<String>> =
        exclude_state.as_ref().map(|s| s.iter().cloned().collect());
    let filter_tag = tag.as_deref();
    let filter_property = parse_property_filter(property);

    let tasks = if let Some(q) = query.as_ref() {
        match crate::query::parse_query(q) {
            Ok(expr) => {
                let strategy = crate::query::analyze_query(&expr);
                match repo.load_tasks_for_query(&strategy) {
                    Ok(tasks) => tasks,
                    Err(_) => repo.load_all_tasks().unwrap_or_default(),
                }
            }
            Err(_) => repo.load_all_tasks().unwrap_or_default(),
        }
    } else {
        repo.load_tasks_with_filter(
            include_states.as_ref(),
            exclude_states.as_ref(),
            filter_tag,
            filter_property,
        )
        .unwrap_or_default()
    };

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
    if let Some(c) = title {
        tasks = tasks_filter_by_title(&tasks, c);
    }
    if let Some(q) = query.as_ref() {
        match crate::query::parse_and_filter(q, &tasks) {
            Ok(filtered) => tasks = filtered,
            Err(e) => {
                return json!({"code": 400, "error": e.to_string()});
            }
        }
    }

    if include_children {
        for task in &mut tasks {
            *task = resolve_task_tree(config, &task.id);
        }

        let mut non_roots: HashSet<String> = HashSet::new();
        for task in &tasks {
            collect_child_ids(task, &mut non_roots);
        }
        tasks = tasks
            .into_iter()
            .filter(|t| !non_roots.contains(&t.id))
            .collect();
    }

    if include_ancestors {
        // TODO: Temporarily neutered during parsing refactor. Needs reimplementation.
        // Parent/ancestor chain resolution will be rebuilt using resolve_task_tree.
    }

    json!(tasks)
}

fn handle_set(
    config: &AgendaConfig,
    id: Option<&str>,
    line: Option<&str>,
    title: Option<&str>,
    state: Option<&str>,
    property: &Option<Vec<String>>,
    tag: &Option<Vec<String>>,
    remove_tags: bool,
    remove_properties: bool,
) -> Value {
    let has_property = property.is_some();
    let has_tag = tag.is_some();
    let has_line = line.is_some();
    let has_fields = title.is_some()
        || state.is_some()
        || has_property
        || has_tag
        || remove_tags
        || remove_properties;

    if has_line && has_fields {
        return json!({"code": 400, "error": "Cannot use --line with field arguments (--title, --state, --property, --tag). They are mutually exclusive."});
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
        return json!({"code": 400, "error": "Either --line or at least one field argument (--title, --state, --property, --tag) is required."});
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
        title,
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

fn handle_add(config: &AgendaConfig, title: &str, task_id: Option<&str>) -> Value {
    task_add(config, title, task_id);
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
    title: &str,
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

    match task_capture(config, title, target, state, &properties, &tags) {
        Ok(task_id) => json!({"code": 200, "message": "task captured", "taskId": task_id}),
        Err(e) => json!({"code": 400, "error": e}),
    }
}

pub fn get_output() -> Value {
    let args = Args::parse();
    let config = AgendaConfig::new(get_vault_dir(&args));

    match &args.command {
        Commands::Get {
            task_id,
            next_state,
            previous_state,
            parent_of,
            parent,
            tag,
            property,
            exclude_state,
            state,
            sort,
            title,
            include_children,
            include_ancestors,
            query,
            query_debug,
        } => {
            if let Some(q) = query_debug {
                handle_query_debug(q)
            } else if let Some(id) = task_id {
                handle_get_by_id(&config, id)
            } else if let Some(ns) = next_state {
                handle_get_next_state(ns)
            } else if let Some(ps) = previous_state {
                handle_get_previous_state(ps)
            } else if let Some(_cid) = parent_of {
                handle_get_parent()
            } else if let Some(parent_id) = parent {
                handle_get_parent_children(
                    &config,
                    parent_id.as_str(),
                    state,
                    exclude_state,
                    tag,
                    property,
                    title,
                    query,
                )
            } else {
                handle_get_filtered_tasks(
                    &config,
                    state,
                    exclude_state,
                    tag,
                    property,
                    sort,
                    title,
                    *include_children,
                    *include_ancestors,
                    query,
                )
            }
        }
        Commands::Set {
            id,
            line,
            title,
            state,
            property,
            tag,
            remove_tags,
            remove_properties,
        } => handle_set(
            &config,
            id.as_deref(),
            line.as_deref(),
            title.as_deref(),
            state.as_deref(),
            property,
            tag,
            *remove_tags,
            *remove_properties,
        ),
        Commands::Add { title, task_id } => handle_add(&config, title, task_id.as_deref()),
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
            title,
            target,
            state,
            property,
            tag,
        } => handle_capture(&config, title, target.as_deref(), state.as_deref(), property, tag),
    }
}
