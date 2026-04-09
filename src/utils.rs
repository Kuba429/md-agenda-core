use indexmap::IndexMap;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::task::Task;

pub fn group_by<K: Eq + Hash>(
    tasks: &[Task],
    key_fn: impl Fn(&Task) -> Option<K>,
) -> IndexMap<K, Vec<&Task>>
where
    K: Eq + Hash + Clone,
{
    let mut grouped: IndexMap<K, Vec<&Task>> = IndexMap::new();

    fn collect<'a, K: Eq + Hash + Clone>(
        task: &'a Task,
        grouped: &mut IndexMap<K, Vec<&'a Task>>,
        seen: &mut HashMap<K, HashSet<&'a str>>,
        key_fn: &impl Fn(&Task) -> Option<K>,
    ) {
        for child in &task.children {
            collect(child, grouped, seen, key_fn);
        }

        if let Some(key) = key_fn(task) {
            let seen_set = seen.entry(key.clone()).or_insert_with(HashSet::new);
            if !seen_set.contains(task.id.as_str()) {
                grouped.entry(key.clone()).or_default().push(task);
                seen_set.insert(task.id.as_str());
            }
        }
    }

    let mut seen: HashMap<K, HashSet<&str>> = HashMap::new();

    for task in tasks {
        collect(task, &mut grouped, &mut seen, &key_fn);
    }

    grouped
}

pub fn group_by_multi<K: Eq + Hash>(
    tasks: &[Task],
    keys_fn: impl Fn(&Task) -> Vec<K>,
) -> IndexMap<K, Vec<&Task>>
where
    K: Eq + Hash + Clone,
{
    let mut grouped: IndexMap<K, Vec<&Task>> = IndexMap::new();

    fn collect<'a, K: Eq + Hash + Clone>(
        task: &'a Task,
        grouped: &mut IndexMap<K, Vec<&'a Task>>,
        seen: &mut HashMap<K, HashSet<&'a str>>,
        keys_fn: &impl Fn(&Task) -> Vec<K>,
    ) {
        for child in &task.children {
            collect(child, grouped, seen, keys_fn);
        }

        for key in keys_fn(task) {
            let seen_set = seen.entry(key.clone()).or_insert_with(HashSet::new);
            if !seen_set.contains(task.id.as_str()) {
                grouped.entry(key.clone()).or_default().push(task);
                seen_set.insert(task.id.as_str());
            }
        }
    }

    let mut seen: HashMap<K, HashSet<&str>> = HashMap::new();

    for task in tasks {
        collect(task, &mut grouped, &mut seen, &keys_fn);
    }

    grouped
}

pub fn filter_tasks<'a>(tasks: &'a [Task], f: impl Fn(&Task) -> bool) -> Vec<&'a Task> {
    tasks.iter().filter(|t| f(*t)).collect()
}

pub fn sort_tasks_by(tasks: &mut [Task], cmp: impl Fn(&Task, &Task) -> Ordering) {
    tasks.sort_by(cmp);
}

#[derive(Debug, Default)]
pub struct TaskIndex {
    pub by_id: HashMap<String, Task>,
    pub by_tag: HashMap<String, Vec<String>>,
}

impl TaskIndex {
    pub fn build(tasks: &[Task]) -> Self {
        let mut by_id = HashMap::new();
        let mut by_tag: HashMap<String, Vec<String>> = HashMap::new();

        fn collect<'a>(
            task: &'a Task,
            by_id: &mut HashMap<String, Task>,
            by_tag: &mut HashMap<String, Vec<String>>,
        ) {
            by_id.insert(task.id.clone(), task.clone());

            for tag in &task.tags {
                by_tag.entry(tag.clone()).or_default().push(task.id.clone());
            }

            if task.tags.is_empty() {
                by_tag
                    .entry("NO TAG".to_string())
                    .or_default()
                    .push(task.id.clone());
            }

            for child in &task.children {
                collect(child, by_id, by_tag);
            }
        }

        for task in tasks {
            collect(task, &mut by_id, &mut by_tag);
        }

        TaskIndex { by_id, by_tag }
    }

    pub fn get_by_id(&self, id: &str) -> Option<&Task> {
        self.by_id.get(id)
    }

    pub fn get_by_tag(&self, tag: &str) -> Option<&Vec<String>> {
        self.by_tag.get(tag)
    }
}
