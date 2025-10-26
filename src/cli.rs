use crate::grep::task_state_next;
use crate::grep::task_state_prev;
use crate::task::task_add;
use crate::task::task_change_property;
use crate::task::task_change_state;
use crate::task::task_parent_get;
use crate::task::tasks_sort;
use crate::task::{task_get_by_id, task_line_set, tasks_get, tasks_group_date, tasks_group_tag};
use clap::Parser;
use clap::Subcommand;
use serde_json::{Value, json};

#[derive(Parser, Debug)]
#[command(name = "agenda-core")]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Get {
        #[arg(short, long)]
        group: Option<String>,
        #[arg(short, long)]
        task_id: Option<String>,
        #[arg(short, long)]
        next_state: Option<String>,
        #[arg(short, long)]
        previous_state: Option<String>,
        #[arg(short, long)]
        parent_of: Option<String>,
    },
    Set {
        #[arg(short, long)]
        id: Option<String>,
        #[arg(short, long)]
        line: Option<String>,
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
}

pub fn get_output() -> Value {
    let args = Args::parse();

    match &args.command {
        Commands::Get {
            group,
            task_id,
            next_state,
            previous_state,
            parent_of,
        } => {
            if let Some(id) = task_id {
                json!({ "task": task_get_by_id(id) })
            } else if let Some(group) = group {
                let mut tasks = tasks_get();
                tasks_sort(&mut tasks);
                match group.as_str() {
                    "date" => json!(tasks_group_date(&tasks)),
                    "tag" => json!(tasks_group_tag(&tasks)),
                    _ => json!(tasks),
                }
            } else if let Some(next_state) = next_state {
                serde_json::Value::String(task_state_next(next_state))
            } else if let Some(previous_state) = previous_state {
                serde_json::Value::String(task_state_prev(previous_state))
            } else if let Some(child_id) = parent_of {
                json!({ "task": task_parent_get(child_id.to_string()) })
            } else {
                let tasks = tasks_get();
                json!(tasks)
            }
        }
        Commands::Set { id, line } => {
            let res = task_line_set(
                id.as_ref().unwrap().to_string(),
                line.as_ref().unwrap().to_string(),
            );
            // TODO: dont return this like its an http response - just return the json or text if
            // its just confirmation message
            json!({"code":200, "newLine": res})
        }
        Commands::Add { content, task_id } => {
            task_add(content, task_id.as_deref());
            json!({"code":200, "message":"task added"})
        }
        Commands::Change {
            task_id,
            state,
            property,
        } => {
            if let Some(state) = state {
                let _ = task_change_state(task_id, state);
            }
            if let Some(prop) = property {
                let key = &prop[0];
                let value = if prop.len() > 1 { Some(&prop[1]) } else { None };
                let _ = task_change_property(task_id, key, value.cloned().as_deref());
            }

            json!({"code":200, "message":"task changed"})
        }
    }
}
