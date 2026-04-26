use crate::config::AgendaConfig;
use crate::query::GrepStrategy;
use crate::task::{compute_parent, task_get_from_lines, task_line_get, tasks_group_by_filename, Task};
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

    #[deprecated(note = "use load_all_tasks() and apply filters explicitly instead")]
    pub fn load_tasks_with_filter(
        &self,
        include_states: Option<&HashSet<String>>,
        exclude_states: Option<&HashSet<String>>,
        filter_tag: Option<&str>,
        filter_property: Option<(&str, Option<&str>)>,
    ) -> Result<Vec<Task>, String> {
        use crate::grep::{tasks_grep, tasks_grep_include, tasks_grep_property, tasks_grep_tag};

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
                tasks_grep(&self.config, exclude_states).map_err(|e| e.to_string())?
            }
        };

        self.parse_task_ids(rg_result)
    }

    pub fn load_all_tasks(&self) -> Result<Vec<Task>, String> {
        use crate::grep::tasks_grep;

        let excluded: HashSet<String> = HashSet::new();
        let rg_result = tasks_grep(&self.config, Some(&excluded)).map_err(|e| e.to_string())?;
        self.parse_task_ids(rg_result)
    }

    pub fn load_tasks_for_query(&self, strategy: &GrepStrategy) -> Result<Vec<Task>, String> {
        use crate::grep::{tasks_grep, tasks_grep_include, tasks_grep_multi_tag, tasks_grep_property, tasks_grep_tag};

        let rg_result = match strategy {
            GrepStrategy::All => {
                let excluded: HashSet<String> = HashSet::new();
                tasks_grep(&self.config, Some(&excluded)).map_err(|e| e.to_string())?
            }
            GrepStrategy::Tag { tag } => {
                tasks_grep_tag(&self.config, tag, None).map_err(|e| e.to_string())?
            }
            GrepStrategy::MultiTag { tags } => {
                tasks_grep_multi_tag(&self.config, tags, None).map_err(|e| e.to_string())?
            }
            GrepStrategy::IncludeState { states } => {
                tasks_grep_include(&self.config, states).map_err(|e| e.to_string())?
            }
            GrepStrategy::ExcludeState { exclude } => {
                tasks_grep(&self.config, Some(exclude)).map_err(|e| e.to_string())?
            }
            GrepStrategy::Property { property } => {
                tasks_grep_property(&self.config, property).map_err(|e| e.to_string())?
            }
        };

        self.parse_task_ids(rg_result)
    }

    fn parse_task_ids(&self, rg_result: String) -> Result<Vec<Task>, String> {
        use crate::fs_utils::file_lines_get_result;

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
                    .filter_map(|t_id| {
                        let mut task = task_get_from_lines(t_id, &lines, &task_ids)?;
                        task.parent = compute_parent(t_id, &lines);
                        Some(task)
                    })
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
    #[allow(deprecated)]
    fn load_tasks(&self) -> Result<Vec<Task>, String> {
        self.load_tasks_with_filter(None, None, None, None)
    }

    fn save_tasks(&self, _tasks: &[Task]) -> Result<(), String> {
        Ok(())
    }
}
