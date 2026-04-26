#![cfg(test)]

mod scenarios {
    use std::path::{Path, PathBuf};

    pub fn vault_path(scenario: &str) -> PathBuf {
        Path::new("test-vault").join(scenario)
    }
}

#[cfg(test)]
mod task_grep {
    use super::scenarios::*;
    use crate::config::AgendaConfig;
    use crate::grep::tasks_grep;

    #[test]
    fn basic_no_filter_returns_1_task() {
        let config = AgendaConfig::new(vault_path("basic"));
        let result = tasks_grep(&config, None).unwrap();

        let tasks: Vec<&str> = result.lines().filter(|l| l.contains(".md:")).collect();
        assert_eq!(tasks.len(), 1, "should return exactly 1 task");

        assert!(
            tasks[0].contains("task") && !tasks[0].contains("child"),
            "task should be 'task' not 'child'"
        );
    }

    #[test]
    fn filter_state_next_returns_1_task() {
        use std::collections::HashSet;
        use crate::grep::tasks_grep_include;

        let config = AgendaConfig::new(vault_path("filter"));
        let mut included = HashSet::new();
        included.insert("NEXT".to_string());
        let result = tasks_grep_include(&config, &included).unwrap();

        let tasks: Vec<&str> = result.lines().filter(|l| l.contains(".md:")).collect();
        assert_eq!(tasks.len(), 1, "should return exactly 1 task");
        assert!(tasks[0].contains("task2"), "task should be 'task2'");
    }

    #[test]
    fn filter_exclude_next_returns_3_tasks() {
        let config = AgendaConfig::new(vault_path("filter"));
        let mut excluded = std::collections::HashSet::new();
        excluded.insert("NEXT".to_string());
        let result = tasks_grep(&config, Some(&excluded)).unwrap();

        let tasks: Vec<&str> = result.lines().filter(|l| l.contains(".md:")).collect();
        assert_eq!(tasks.len(), 4, "should return all TODO tasks when NEXT excluded (including nested)");
    }
}

#[cfg(test)]
mod task_grep_property {
    use super::scenarios::*;
    use crate::config::AgendaConfig;
    use crate::grep::tasks_grep_property;

    #[test]
    fn filter_property_scheduled_returns_1_task() {
        let config = AgendaConfig::new(vault_path("filter"));
        let result = tasks_grep_property(&config, "scheduled").unwrap();

        let tasks: Vec<&str> = result.lines().filter(|l| l.contains(".md:")).collect();
        assert_eq!(tasks.len(), 1, "should return exactly 1 task");
        assert!(tasks[0].contains("task3"), "task should be 'task3'");
    }

    #[test]
    fn filter_property_scheduled_value_returns_1_task() {
        let config = AgendaConfig::new(vault_path("filter"));
        let result = tasks_grep_property(&config, "scheduled=2025-04-20").unwrap();

        let tasks: Vec<&str> = result.lines().filter(|l| l.contains(".md:")).collect();
        assert_eq!(tasks.len(), 1, "should return exactly 1 task");
        assert!(tasks[0].contains("task3"), "task should be 'task3'");
    }

    #[test]
    fn filter_property_scheduled_value_no_match_returns_0_tasks() {
        let config = AgendaConfig::new(vault_path("filter"));
        let result = tasks_grep_property(&config, "scheduled=2025-04-21").unwrap();

        let tasks: Vec<&str> = result.lines().filter(|l| l.contains(".md:")).collect();
        assert_eq!(tasks.len(), 0, "should return 0 tasks");
    }
}

#[cfg(test)]
mod query_dsl {
    use super::scenarios::*;
    use crate::config::AgendaConfig;
    use crate::query::parse_and_filter;
    use crate::repository::MarkdownTaskRepository;

    fn load_tasks() -> Vec<crate::task::Task> {
        let config = AgendaConfig::new(vault_path("query"));
        let repo = MarkdownTaskRepository::new(config);
        repo.load_all_tasks().unwrap()
    }

    fn count_by_title(tasks: &[crate::task::Task], title: &str) -> usize {
        tasks
            .iter()
            .filter(|t| t.title.contains(title))
            .count()
    }

    #[test]
    fn query_tag_foo() {
        let tasks = load_tasks();
        let result = parse_and_filter("tag:foo", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_tag_foo"), 1);
        assert_eq!(count_by_title(&result, "task_with_both_tags"), 1);
        assert_eq!(count_by_title(&result, "task_with_tag_and_property"), 1);
        assert_eq!(count_by_title(&result, "task_in_progress"), 1);
        assert_eq!(count_by_title(&result, "done_task"), 1);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn query_tag_bar() {
        let tasks = load_tasks();
        let result = parse_and_filter("tag:bar", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_tag_bar"), 1);
        assert_eq!(count_by_title(&result, "task_with_both_tags"), 1);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn query_property_priority_equals_high() {
        let tasks = load_tasks();
        let result = parse_and_filter("property:priority=high", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_property_prio"), 1);
        assert_eq!(count_by_title(&result, "task_with_tag_and_property"), 1);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn query_tag_foo_and_property_priority_equals_high() {
        let tasks = load_tasks();
        let result = parse_and_filter("tag:foo AND property:priority=high", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_tag_and_property"), 1);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn query_tag_foo_or_tag_bar() {
        let tasks = load_tasks();
        let result = parse_and_filter("tag:foo OR tag:bar", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_tag_foo"), 1);
        assert_eq!(count_by_title(&result, "task_with_tag_bar"), 1);
        assert_eq!(count_by_title(&result, "task_with_both_tags"), 1);
        assert_eq!(count_by_title(&result, "task_with_tag_and_property"), 1);
        assert_eq!(count_by_title(&result, "task_in_progress"), 1);
        assert_eq!(count_by_title(&result, "done_task"), 1);
        assert_eq!(result.len(), 6);
    }

    #[test]
    fn query_tag_foo_and_not_tag_bar() {
        let tasks = load_tasks();
        let result = parse_and_filter("tag:foo AND -tag:bar", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_tag_foo"), 1);
        assert_eq!(count_by_title(&result, "task_with_tag_and_property"), 1);
        assert_eq!(count_by_title(&result, "task_in_progress"), 1);
        assert_eq!(count_by_title(&result, "done_task"), 1);
        assert_eq!(result.len(), 4);
        assert_eq!(count_by_title(&result, "task_with_both_tags"), 0);
    }

    #[test]
    fn query_tag_foo_and_property_priority_high_and_not_tag_foo_bar() {
        let tasks = load_tasks();
        let result =
            parse_and_filter("tag:foo AND property:priority=high AND -tag:foo_bar", &tasks)
                .unwrap();
        assert_eq!(count_by_title(&result, "task_with_tag_and_property"), 1);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn query_state_todo() {
        let tasks = load_tasks();
        let result = parse_and_filter("state:TODO", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_tag_foo"), 1);
        assert_eq!(count_by_title(&result, "task_with_tag_bar"), 1);
        assert_eq!(count_by_title(&result, "task_with_both_tags"), 1);
        assert_eq!(count_by_title(&result, "task_with_foo_bar_tag"), 1);
        assert_eq!(count_by_title(&result, "task_with_property_prio"), 1);
        assert_eq!(count_by_title(&result, "task_with_tag_and_property"), 1);
        assert_eq!(count_by_title(&result, "task_with_property_scheduled"), 1);
        assert_eq!(count_by_title(&result, "meeting_notes_task"), 1);
        assert_eq!(count_by_title(&result, "task_with_scheduled_time"), 1);
        assert_eq!(result.len(), 9);
    }

    #[test]
    fn query_nested_and_or() {
        let tasks = load_tasks();
        let result = parse_and_filter("(tag:foo OR tag:bar) AND state:TODO", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_tag_foo"), 1);
        assert_eq!(count_by_title(&result, "task_with_tag_bar"), 1);
        assert_eq!(count_by_title(&result, "task_with_both_tags"), 1);
        assert_eq!(count_by_title(&result, "task_with_tag_and_property"), 1);
        assert_eq!(result.len(), 4);
        assert_eq!(count_by_title(&result, "task_in_progress"), 0);
        assert_eq!(count_by_title(&result, "done_task"), 0);
    }

    #[test]
    fn query_title_meeting() {
        let tasks = load_tasks();
        let result = parse_and_filter("title:meeting", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "meeting_notes_task"), 1);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn query_not_state_done() {
        let tasks = load_tasks();
        let result = parse_and_filter("-state:DONE", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "done_task"), 0);
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn query_property_scheduled_exists() {
        let tasks = load_tasks();
        let result = parse_and_filter("property:scheduled", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_property_scheduled"), 1);
        assert_eq!(count_by_title(&result, "task_with_scheduled_time"), 1);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn query_property_date_matches_datetime() {
        let tasks = load_tasks();
        let result = parse_and_filter("property:scheduled=2024-01-15", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_property_scheduled"), 1);
        assert_eq!(count_by_title(&result, "task_with_scheduled_time"), 1);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn query_property_datetime_exact_match() {
        let tasks = load_tasks();
        let result = parse_and_filter("property:scheduled=2024-01-15T10:30", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_scheduled_time"), 1);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn query_property_not_exists() {
        let tasks = load_tasks();
        let result = parse_and_filter("-property:scheduled", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_property_scheduled"), 0);
        assert_eq!(result.len(), 9);
    }

    #[test]
    fn query_negated_group() {
        let tasks = load_tasks();
        let result = parse_and_filter("-(tag:foo AND tag:bar)", &tasks).unwrap();
        assert_eq!(count_by_title(&result, "task_with_both_tags"), 0);
        assert_eq!(result.len(), 10);
    }

    #[test]
    fn query_empty_result() {
        let tasks = load_tasks();
        let result = parse_and_filter("tag:nonexistent", &tasks).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn query_invalid_returns_error() {
        let tasks = load_tasks();
        let result = parse_and_filter("tag:", &tasks);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod query_grep_strategy {
    use super::scenarios::*;
    use crate::config::AgendaConfig;
    use crate::query::{analyze_query, parse_query, filter_tasks, GrepStrategy};
    use crate::repository::MarkdownTaskRepository;
    use std::collections::HashSet;

    fn load_via_strategy(strategy: &GrepStrategy) -> Vec<crate::task::Task> {
        let config = AgendaConfig::new(vault_path("query"));
        let repo = MarkdownTaskRepository::new(config);
        repo.load_tasks_for_query(strategy).unwrap()
    }

    fn count_by_title(tasks: &[crate::task::Task], title: &str) -> usize {
        tasks
            .iter()
            .filter(|t| t.title.contains(title))
            .count()
    }

    #[test]
    fn strategy_tag_returns_tagged_tasks() {
        let strategy = GrepStrategy::Tag { tag: "foo".to_string() };
        let tasks = load_via_strategy(&strategy);
        let filtered = {
            let expr = parse_query("tag:foo").unwrap();
            filter_tasks(&expr, &tasks)
        };
        assert_eq!(filtered.len(), 5);
        assert_eq!(count_by_title(&filtered, "task_with_tag_foo"), 1);
    }

    #[test]
    fn strategy_multitag_returns_or_tagged_tasks() {
        let strategy = GrepStrategy::MultiTag { tags: vec!["foo".to_string(), "bar".to_string()] };
        let tasks = load_via_strategy(&strategy);
        let filtered = {
            let expr = parse_query("tag:foo OR tag:bar").unwrap();
            filter_tasks(&expr, &tasks)
        };
        assert_eq!(filtered.len(), 6);
    }

    #[test]
    fn strategy_include_state_returns_state_tasks() {
        let mut states = HashSet::new();
        states.insert("TODO".to_string());
        let strategy = GrepStrategy::IncludeState { states };
        let tasks = load_via_strategy(&strategy);
        let all_todo = tasks.iter().all(|t| t.state == "TODO");
        assert!(all_todo);
        assert!(tasks.len() >= 8);
    }

    #[test]
    fn strategy_exclude_state_excludes() {
        let mut exclude = HashSet::new();
        exclude.insert("DONE".to_string());
        let strategy = GrepStrategy::ExcludeState { exclude };
        let tasks = load_via_strategy(&strategy);
        assert!(tasks.iter().all(|t| t.state != "DONE"));
        assert!(tasks.iter().all(|t| t.state != "CANCELLED"));
    }

    #[test]
    fn strategy_property_returns_matching_tasks() {
        let strategy = GrepStrategy::Property { property: "priority=high".to_string() };
        let tasks = load_via_strategy(&strategy);
        assert!(tasks.len() >= 2);
        assert!(tasks.iter().any(|t| t.title.contains("task_with_property_prio")));
        assert!(tasks.iter().any(|t| t.title.contains("task_with_tag_and_property")));
    }

    #[test]
    fn strategy_all_returns_everything() {
        let strategy = GrepStrategy::All;
        let tasks = load_via_strategy(&strategy);
        assert_eq!(tasks.len(), 11);
    }

    #[test]
    fn analyze_end_to_end_tag_and_property() {
        let expr = parse_query("tag:foo AND property:priority=high").unwrap();
        let strategy = analyze_query(&expr);
        let tasks = load_via_strategy(&strategy);
        let result = filter_tasks(&expr, &tasks);
        assert_eq!(result.len(), 1);
        assert!(result[0].title.contains("task_with_tag_and_property"));
    }

    #[test]
    fn analyze_end_to_end_or_tags() {
        let expr = parse_query("tag:foo OR tag:bar").unwrap();
        let strategy = analyze_query(&expr);
        let tasks = load_via_strategy(&strategy);
        let result = filter_tasks(&expr, &tasks);
        assert_eq!(result.len(), 6);
    }
}

#[cfg(test)]
mod parent_children {
    use super::scenarios::*;
    use crate::config::AgendaConfig;
    use crate::task::task_get_direct_children;

    #[test]
    fn returns_direct_children() {
        let config = AgendaConfig::new(vault_path("parent-basic"));
        let children = task_get_direct_children(&config, "file.md:1");
        assert_eq!(children.len(), 2, "should return 2 direct children");
        assert!(children.iter().any(|c| c.title == "child1"));
        assert!(children.iter().any(|c| c.title == "child2"));
        assert!(children.iter().all(|c| c.parent == Some("file.md:1".to_string())));
    }

    #[test]
    fn does_not_include_parent_or_siblings() {
        let config = AgendaConfig::new(vault_path("parent-basic"));
        let children = task_get_direct_children(&config, "file.md:1");
        assert!(children.iter().all(|c| c.title != "parent_task"));
        assert!(children.iter().all(|c| c.title != "other_task"));
    }

    #[test]
    fn parent_filter_state_done() {
        let config = AgendaConfig::new(vault_path("parent-basic"));
        let children = task_get_direct_children(&config, "file.md:1");
        let done: Vec<_> = children.into_iter().filter(|c| c.state == "DONE").collect();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].title, "child2");
    }

    #[test]
    fn returns_all_children_with_tags() {
        let config = AgendaConfig::new(vault_path("parent-filtered"));
        let children = task_get_direct_children(&config, "file.md:1");
        assert_eq!(children.len(), 4, "should return 4 children");
    }

    #[test]
    fn parent_filter_tag() {
        let config = AgendaConfig::new(vault_path("parent-filtered"));
        let children = task_get_direct_children(&config, "file.md:1");
        let bug: Vec<_> = children.into_iter().filter(|c| c.tags.contains(&"bug".to_string())).collect();
        assert_eq!(bug.len(), 1);
        assert_eq!(bug[0].title, "child_bug");
    }

    #[test]
    fn parent_nested_returns_only_direct_children() {
        let config = AgendaConfig::new(vault_path("parent-nested"));
        let children = task_get_direct_children(&config, "file.md:2");
        assert_eq!(children.len(), 2, "should return 2 direct children");
        assert!(children.iter().all(|c| c.title == "child1" || c.title == "child2"));
    }

    #[test]
    fn parent_nested_does_not_include_siblings() {
        let config = AgendaConfig::new(vault_path("parent-nested"));
        let children = task_get_direct_children(&config, "file.md:2");
        assert!(children.iter().all(|c| c.title != "parent_sibling"));
        assert!(children.iter().all(|c| c.title != "grandparent"));
    }

    #[test]
    fn parent_no_children_returns_empty() {
        let config = AgendaConfig::new(vault_path("parent-no-children"));
        let children = task_get_direct_children(&config, "file.md:1");
        assert!(children.is_empty());
    }

    #[test]
    fn parent_mixed_ignores_non_task_lines() {
        let config = AgendaConfig::new(vault_path("parent-mixed"));
        let children = task_get_direct_children(&config, "file.md:1");
        assert_eq!(children.len(), 2, "should return 2 children, not non-task lines");
        assert!(children.iter().any(|c| c.title == "child1"));
        assert!(children.iter().any(|c| c.title == "child2"));
        assert!(children.iter().all(|c| c.state == "TODO" || c.state == "DONE"));
    }
}

#[cfg(test)]
mod include_children {
    use super::scenarios::*;
    use crate::config::AgendaConfig;
    use crate::repository::MarkdownTaskRepository;
    use crate::task::{resolve_task_tree, tasks_filter_by_tag};

    fn load_all(scenario: &str) -> Vec<crate::task::Task> {
        let config = AgendaConfig::new(vault_path(scenario));
        let repo = MarkdownTaskRepository::new(config);
        repo.load_all_tasks().unwrap()
    }

    #[test]
    fn basic_includes_direct_children() {
        let tasks = load_all("include-children-basic");
        let mut matched: Vec<_> = tasks.into_iter().filter(|t| t.title == "parent").collect();
        for task in &mut matched {
            *task = resolve_task_tree(&AgendaConfig::new(vault_path("include-children-basic")), &task.id);
        }
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].children.len(), 2);
        assert!(matched[0].children.iter().any(|c| c.title == "child1"));
        assert!(matched[0].children.iter().any(|c| c.title == "child2"));
    }

    #[test]
    fn basic_done_children_included() {
        let tasks = load_all("include-children-basic");
        let mut matched: Vec<_> = tasks.into_iter().filter(|t| t.title == "parent").collect();
        for task in &mut matched {
            *task = resolve_task_tree(&AgendaConfig::new(vault_path("include-children-basic")), &task.id);
        }
        assert!(matched[0].children.iter().any(|c| c.state == "DONE"));
    }

    #[test]
    fn nested_preserves_hierarchy() {
        let mut matched = load_all("include-children-nested");
        for task in &mut matched {
            *task = resolve_task_tree(&AgendaConfig::new(vault_path("include-children-nested")), &task.id);
        }
        let gp = matched.iter().find(|t| t.title == "grandparent").unwrap();
        assert_eq!(gp.children.len(), 1);
        assert_eq!(gp.children[0].title, "parent");
        assert_eq!(gp.children[0].children.len(), 1);
        assert_eq!(gp.children[0].children[0].title, "child");
    }

    #[test]
    fn nested_dedup_removes_children_from_root() {
        let tasks = load_all("include-children-nested");
        // simulate filtering — no filter, all tasks matched
        let mut matched: Vec<_> = tasks;
        for task in &mut matched {
            *task = resolve_task_tree(&AgendaConfig::new(vault_path("include-children-nested")), &task.id);
        }

        // collect child IDs
        let mut child_ids = std::collections::HashSet::new();
        fn collect(c: &crate::task::Task, ids: &mut std::collections::HashSet<String>) {
            for child in &c.children {
                ids.insert(child.id.clone());
                collect(child, ids);
            }
        }
        for t in &matched {
            collect(t, &mut child_ids);
        }

        // dedup
        matched.retain(|t| !child_ids.contains(&t.id));
        assert_eq!(matched.len(), 2); // grandparent and other remain
        assert!(matched.iter().any(|t| t.title == "grandparent"));
        assert!(matched.iter().any(|t| t.title == "other"));
    }

    #[test]
    fn filtered_only_matched_tasks_get_children() {
        let config = AgendaConfig::new(vault_path("include-children-filtered"));
        let tasks = load_all("include-children-filtered");
        let mut matched = tasks_filter_by_tag(&tasks, "bug");
        for task in &mut matched {
            *task = resolve_task_tree(&config, &task.id);
        }
        // only tasks with #bug get children
        assert!(matched.iter().any(|t| t.title == "parent" && t.children.len() == 2));
        assert!(matched.iter().any(|t| t.title == "other" && t.children.len() == 1));
        assert!(matched.iter().all(|t| t.title != "unrelated"));
    }

    #[test]
    fn filtered_children_include_all_states() {
        let config = AgendaConfig::new(vault_path("include-children-filtered"));
        let tasks = load_all("include-children-filtered");
        let mut matched = tasks_filter_by_tag(&tasks, "bug");
        for task in &mut matched {
            *task = resolve_task_tree(&config, &task.id);
        }
        let parent = matched.iter().find(|t| t.title == "parent").unwrap();
        // children include DONE tasks even though filter affected parent matching
        assert!(parent.children.iter().any(|c| c.state == "DONE"));
    }
}