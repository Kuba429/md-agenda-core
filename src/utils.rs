use std::cmp::Ordering;

use crate::task::Task;

pub fn filter_tasks<'a>(tasks: &'a [Task], f: impl Fn(&Task) -> bool) -> Vec<&'a Task> {
    tasks.iter().filter(|t| f(*t)).collect()
}

pub fn sort_tasks_by(tasks: &mut [Task], cmp: impl Fn(&Task, &Task) -> Ordering) {
    tasks.sort_by(cmp);
}