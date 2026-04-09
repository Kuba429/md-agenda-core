use crate::config::AgendaConfig;
use crate::task::Task;
use rayon::prelude::*;
use std::collections::HashSet;

pub trait TaskRepository: Send + Sync {
    fn load_tasks(&self) -> Result<Vec<Task>, String>;
    fn save_tasks(&self, tasks: &[Task]) -> Result<(), String>;
}

pub struct MarkdownTaskRepository {
    config: AgendaConfig,
}

impl MarkdownTaskRepository {
    pub fn new(config: AgendaConfig) -> Self {
        MarkdownTaskRepository { config }
    }

    pub fn config(&self) -> &AgendaConfig {
        &self.config
    }

    pub fn load_tasks_with_filter(
        &self,
        include_states: Option<&HashSet<String>>,
        exclude_states: Option<&HashSet<String>>,
        filter_tag: Option<&str>,
        filter_property: Option<(&str, Option<&str>)>,
    ) -> Result<Vec<Task>, String> {
        use crate::fs_utils::file_lines_get_result;
        use crate::grep::{tasks_grep, tasks_grep_include, tasks_grep_property, tasks_grep_tag};
        use crate::task::{task_get_from_lines, task_line_get, tasks_group_by_filename};

        let rg_result = match (include_states, filter_tag, filter_property) {
            (Some(states), None, None) => {
                tasks_grep_include(&self.config, states).map_err(|e| e.to_string())?
            }
            (None, Some(tag), None) => {
                tasks_grep_tag(&self.config, tag, exclude_states).map_err(|e| e.to_string())?
            }
            (None, None, Some((prop, value))) => {
                if let Some(v) = value {
                    let full_prop = format!("{}={}", prop, v);
                    tasks_grep_property(&self.config, &full_prop).map_err(|e| e.to_string())?
                } else {
                    tasks_grep_property(&self.config, prop).map_err(|e| e.to_string())?
                }
            }
            _ => {
                let exclude = exclude_states.map(|e| e.clone()).unwrap_or_else(|| {
                    let mut s = HashSet::new();
                    s.insert("DONE".to_string());
                    s.insert("CANCELLED".to_string());
                    s
                });
                tasks_grep(&self.config, Some(&exclude)).map_err(|e| e.to_string())?
            }
        };

        let task_ids: HashSet<String> = rg_result
            .lines()
            .filter_map(|line| {
                let mut parts = line.splitn(3, ':');
                let filename = parts.next()?;
                let line_nr = parts.next()?;
                Some(format!("{}:{}", filename, line_nr))
            })
            .collect();

        let grouped = tasks_group_by_filename(&task_ids);

        let vault_dir = self.config.vault_dir.clone();

        let mut filenames: Vec<_> = grouped.keys().cloned().collect();
        filenames.sort();

        let mut indexed_tasks: Vec<(usize, Task)> = filenames
            .par_iter()
            .enumerate()
            .filter_map(|(idx, filename)| {
                let tasks = grouped.get(filename)?;
                let full_path = vault_dir.join(filename);
                let lines = match file_lines_get_result(full_path.to_string_lossy().as_ref()) {
                    Ok(l) => l,
                    Err(_) => return None,
                };

                let mut sorted_tasks: Vec<String> =
                    tasks.iter().map(|s| (*s).to_string()).collect();
                sorted_tasks.sort_by_key(|id| task_line_get(id));

                let parsed_tasks: Vec<Task> = sorted_tasks
                    .iter()
                    .filter_map(|t_id| task_get_from_lines(t_id, &lines, &task_ids))
                    .collect();

                if parsed_tasks.is_empty() {
                    None
                } else {
                    Some((idx, parsed_tasks))
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
            .flat_map(|(idx, tasks)| tasks.into_iter().map(move |t| (idx, t)))
            .collect();

        indexed_tasks.sort_by_key(|(idx, _)| *idx);

        let res: Vec<Task> = indexed_tasks.into_iter().map(|(_, t)| t).collect();

        Ok(res)
    }
}

impl TaskRepository for MarkdownTaskRepository {
    fn load_tasks(&self) -> Result<Vec<Task>, String> {
        self.load_tasks_with_filter(None, None, None, None)
    }

    fn save_tasks(&self, _tasks: &[Task]) -> Result<(), String> {
        Ok(())
    }
}
