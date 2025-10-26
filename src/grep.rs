use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::process::Command;

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

pub fn vault_dir() -> String {
    env::var("AGENDA_VAULT_DIR").expect("AGENDA_VAULT_DIR environment variable not set")
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

pub fn tasks_grep(excluded: Option<&HashSet<String>>) -> Result<String, Box<dyn Error>> {
    let pattern = pattern_get(excluded);
    let vault_dir = vault_dir();

    let output = Command::new("rg")
        .args(["-H", "-n", "--pcre2", "--multiline", &pattern, &vault_dir])
        .output()?;

    let mut lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|s| s.to_string())
        .collect();

    lines.sort();

    Ok(lines.join("\n"))
}
