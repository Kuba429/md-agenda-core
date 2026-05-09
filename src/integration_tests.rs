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

    #[test]
    fn analyze_or_property_different_values() {
        let expr = parse_query("property:scheduled=2026-05-14 OR property:scheduled=2026-05-15").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::All,
            "OR of properties with different values must use All strategy");
    }

    #[test]
    fn analyze_or_same_property_value() {
        let expr = parse_query("property:scheduled=2026-05-15 OR property:scheduled=2026-05-15").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::Property { property: "scheduled=2026-05-15".to_string() },
            "OR of identical property values should reuse the same strategy");
    }

    #[test]
    fn analyze_or_excludestate_and_property() {
        let expr = parse_query("(-state:DONE) OR property:scheduled=2026-05-15").unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::All,
            "OR of ExcludeState and Property must use All strategy");
    }

    #[test]
    fn analyze_or_complex_state_exclusion_and_property_chain() {
        let expr = parse_query(
            "(-state:DONE AND -state:CANCELLED AND -state:ARCHIVED) OR \
             (property:scheduled=2026-05-14 OR property:scheduled=2026-05-15 OR property:scheduled=2026-05-16)"
        ).unwrap();
        let strategy = analyze_query(&expr);
        assert_eq!(strategy, GrepStrategy::All,
            "OR of complex state exclusion and property chain must use All strategy");
    }
}

#[cfg(test)]
mod query_and_or_strategy {
    use super::scenarios::*;
    use crate::config::AgendaConfig;
    use crate::query::{analyze_query, filter_tasks, parse_query, GrepStrategy};
    use crate::repository::MarkdownTaskRepository;

    fn load_via_strategy_on(scenario: &str, strategy: &GrepStrategy) -> Vec<crate::task::Task> {
        let config = AgendaConfig::new(vault_path(scenario));
        let repo = MarkdownTaskRepository::new(config);
        repo.load_tasks_for_query(strategy).unwrap()
    }

    #[test]
    fn property_or_where_first_date_missing() {
        let expr = parse_query("property:scheduled=2026-05-14 OR property:scheduled=2026-05-15").unwrap();
        let strategy = analyze_query(&expr);
        let tasks = load_via_strategy_on("query-and-or-multi", &strategy);
        let result = filter_tasks(&expr, &tasks);
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        assert!(titles.contains(&"task_a"), "task_a has scheduled=2026-05-15");
        assert_eq!(result.len(), 1, "titles: {:?}", titles);
    }

    #[test]
    fn state_exclusion_or_property_chain() {
        let expr = parse_query(
            "(-state:DONE) OR \
             (property:scheduled=2026-05-15 OR property:scheduled=2026-05-16)"
        ).unwrap();
        let strategy = analyze_query(&expr);
        let tasks = load_via_strategy_on("query-and-or-multi", &strategy);
        let result = filter_tasks(&expr, &tasks);
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        // task_a: -state:DONE? yes (TODO) → left ✓
        // task_b: -state:DONE? no (DONE) → left ✗. property:scheduled=2026-05-16? yes → right ✓
        // task_c: -state:DONE? yes (TODO) → left ✓
        // task_d: -state:DONE? yes (CANCELLED, not DONE) → left ✓
        assert!(titles.contains(&"task_a"), "task_a: left side (TODO, not DONE)");
        assert!(titles.contains(&"task_b"), "task_b: right side (scheduled=16)");
        assert!(titles.contains(&"task_c"), "task_c: left side (TODO, not DONE)");
        assert!(titles.contains(&"task_d"), "task_d: left side (CANCELLED, not DONE)");
        assert_eq!(result.len(), 4, "titles: {:?}", titles);
    }

    #[test]
    fn complex_state_exclusion_or_property_chain() {
        let expr = parse_query(
            "(-state:DONE AND -state:CANCELLED AND -state:ARCHIVED) OR \
             (property:scheduled=2026-05-14 OR property:scheduled=2026-05-15 OR property:scheduled=2026-05-16)"
        ).unwrap();
        let strategy = analyze_query(&expr);
        let tasks = load_via_strategy_on("query-and-or-multi", &strategy);
        let result = filter_tasks(&expr, &tasks);
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        // task_a: -state:DONE? yes (TODO) → left ✓. -state:CANCELLED? yes → left ✓. -state:ARCHIVED? yes → left ✓
        // task_b: -state:DONE? no (DONE) → left ✗. property:scheduled=2026-05-16? yes → right ✓
        // task_c: -state:DONE? yes (TODO) → left ✓
        // task_d: -state:DONE? yes (CANCELLED) → left ✓. -state:CANCELLED? no (CANCELLED) → left ✗
        //         property:scheduled=2026-05-16? yes → right ✓
        assert!(titles.contains(&"task_a"), "task_a: left side (TODO)");
        assert!(titles.contains(&"task_b"), "task_b: right side (scheduled=16)");
        assert!(titles.contains(&"task_c"), "task_c: left side (TODO)");
        assert!(titles.contains(&"task_d"), "task_d: right side (scheduled=16)");
        assert_eq!(result.len(), 4, "titles: {:?}", titles);
    }

    #[test]
    fn property_or_first_date_in_second_file() {
        let expr = parse_query(
            "property:scheduled=2026-05-17 OR property:scheduled=2026-05-15"
        ).unwrap();
        let strategy = analyze_query(&expr);
        let tasks = load_via_strategy_on("query-and-or-multi", &strategy);
        let result = filter_tasks(&expr, &tasks);
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        assert!(titles.contains(&"task_a"), "task_a has scheduled=2026-05-15");
        assert_eq!(result.len(), 1, "titles: {:?}", titles);
    }

    #[test]
    fn or_between_tag_and_property() {
        let expr = parse_query("tag:foo OR property:scheduled=2026-05-15").unwrap();
        let strategy = analyze_query(&expr);
        let tasks = load_via_strategy_on("query-and-or-multi", &strategy);
        let result = filter_tasks(&expr, &tasks);
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        assert!(titles.contains(&"task_a"), "task_a has scheduled=2026-05-15 (right side)");
        assert_eq!(result.len(), 1, "titles: {:?}", titles);
    }

    #[test]
    fn non_existent_property_or_returns_empty() {
        let expr = parse_query("property:scheduled=2026-05-99 OR property:scheduled=2026-05-98").unwrap();
        let strategy = analyze_query(&expr);
        let tasks = load_via_strategy_on("query-and-or-multi", &strategy);
        let result = filter_tasks(&expr, &tasks);
        assert!(result.is_empty(), "no tasks match non-existent dates");
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

    #[test]
    fn deep_nesting_returns_direct_child_only() {
        let config = AgendaConfig::new(vault_path("parent-deep-nesting"));
        let children = task_get_direct_children(&config, "file.md:1");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].title, "level2");
    }

    #[test]
    fn deep_nesting_level2_returns_level3() {
        let config = AgendaConfig::new(vault_path("parent-deep-nesting"));
        let children = task_get_direct_children(&config, "file.md:2");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].title, "level3");
    }

    #[test]
    fn deep_nesting_level4_returns_level5() {
        let config = AgendaConfig::new(vault_path("parent-deep-nesting"));
        let children = task_get_direct_children(&config, "file.md:4");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].title, "level5");
    }

    #[test]
    fn deep_nesting_traverse_root_to_leaf() {
        let config = AgendaConfig::new(vault_path("parent-deep-nesting"));
        let l2 = task_get_direct_children(&config, "file.md:1");
        assert_eq!(l2[0].title, "level2");
        let l3 = task_get_direct_children(&config, &l2[0].id);
        assert_eq!(l3[0].title, "level3");
        let l4 = task_get_direct_children(&config, &l3[0].id);
        assert_eq!(l4[0].title, "level4");
        let l5 = task_get_direct_children(&config, &l4[0].id);
        assert_eq!(l5[0].title, "level5");
        assert!(l5[0].children.is_empty(), "level5 has no children");
    }

    #[test]
    fn deep_nesting_level3_returns_level4() {
        let config = AgendaConfig::new(vault_path("parent-deep-nesting"));
        let children = task_get_direct_children(&config, "file.md:3");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].title, "level4");
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

#[cfg(test)]
mod task_body {
    use super::scenarios::*;
    use crate::config::AgendaConfig;
    use crate::task::task_get_by_id;

    #[test]
    fn body_trim_whitespace() {
        let config = AgendaConfig::new(vault_path("body-trim-whitespace"));
        let task = task_get_by_id(&config, "file.md:1");

        assert!(task.body.contains("extra spaces here"));
        assert!(!task.body.starts_with("extra spaces here"), "body has relative indent");
    }

    #[test]
    fn body_multiline() {
        let config = AgendaConfig::new(vault_path("body-multiline"));
        let task = task_get_by_id(&config, "file.md:1");

        assert_eq!(task.body, "Line one of body\nLine two of body\nLine three of body");
    }

    #[test]
    fn body_with_code() {
        let config = AgendaConfig::new(vault_path("body-with-code"));
        let task = task_get_by_id(&config, "file.md:1");

        assert!(task.body.contains("```"));
        assert!(task.body.contains("fn main()"));
    }
}

#[cfg(test)]
mod additional_filtering {
    use super::scenarios::*;
    use crate::config::AgendaConfig;
    use crate::query::parse_and_filter;
    use crate::repository::MarkdownTaskRepository;

    fn load_tasks(scenario: &str) -> Vec<crate::task::Task> {
        let config = AgendaConfig::new(vault_path(scenario));
        let repo = MarkdownTaskRepository::new(config);
        repo.load_all_tasks().unwrap()
    }

    #[test]
    fn filter_multi_property_scheduled_and_priority() {
        let tasks = load_tasks("filter-multi-property");
        let result = parse_and_filter("property:scheduled=2024-01-01 AND property:priority=high", &tasks).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "task2");
    }

    #[test]
    fn filter_multi_property_all_three() {
        let tasks = load_tasks("filter-multi-property");
        let result = parse_and_filter("property:scheduled=2024-01-02 AND property:priority=high", &tasks).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "task3");
    }

    #[test]
    fn filter_content_search_title() {
        let tasks = load_tasks("filter-content-search");
        let result = parse_and_filter("title:login", &tasks).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].title.contains("login"));
    }

    #[test]
    fn filter_content_search_title_api() {
        let tasks = load_tasks("filter-content-search");
        let result = parse_and_filter("title:documentation", &tasks).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_content_search_title_nonexistent() {
        let tasks = load_tasks("filter-content-search");
        let result = parse_and_filter("title:nonexistent", &tasks).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn filter_date_exact_match() {
        let tasks = load_tasks("filter-date-range");
        let result = parse_and_filter("property:scheduled=2024-01-15", &tasks).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "task2");
    }

    #[test]
    fn filter_date_range_combined() {
        let tasks = load_tasks("filter-date-range");
        let result = parse_and_filter("property:scheduled=2024-01-01 OR property:scheduled=2024-01-15", &tasks).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_empty_results_state_done() {
        let tasks = load_tasks("filter-empty-results");
        let result = parse_and_filter("state:DONE", &tasks).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn filter_empty_results_priority_medium() {
        let tasks = load_tasks("filter-empty-results");
        let result = parse_and_filter("property:priority=medium", &tasks).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn filter_empty_results_nonexistent_tag() {
        let tasks = load_tasks("filter-empty-results");
        let result = parse_and_filter("tag:nonexistent", &tasks).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn filter_priority_values_low() {
        let tasks = load_tasks("filter-priority-values");
        let result = parse_and_filter("property:priority=low", &tasks).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "task1");
    }

    #[test]
    fn filter_priority_values_high() {
        let tasks = load_tasks("filter-priority-values");
        let result = parse_and_filter("property:priority=high", &tasks).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "task3");
    }

    #[test]
    fn filter_priority_values_urgent() {
        let tasks = load_tasks("filter-priority-values");
        let result = parse_and_filter("property:priority=urgent", &tasks).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "task5");
    }

#[test]
    fn filter_priority_values_none() {
        let tasks = load_tasks("filter-priority-values");
        let result = parse_and_filter("-property:priority", &tasks).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "task4");
    }
}

#[cfg(test)]
mod complex_queries {
    use super::scenarios::*;
    use crate::config::AgendaConfig;
    use crate::query::parse_and_filter;
    use crate::repository::MarkdownTaskRepository;

    fn load_tasks(scenario: &str) -> Vec<crate::task::Task> {
        let config = AgendaConfig::new(vault_path(scenario));
        let repo = MarkdownTaskRepository::new(config);
        repo.load_all_tasks().unwrap()
    }

    #[test]
    fn complex_2_level_and() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("tag:foo AND property:priority=high", &tasks).unwrap();
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(result.len(), 4, "titles: {:?}", titles);
        assert!(titles.contains(&"task1"));
        assert!(titles.contains(&"task3"));
    }

    #[test]
    fn complex_2_level_or() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("tag:foo OR tag:bar", &tasks).unwrap();
        assert!(result.len() >= 5);
    }

    #[test]
    fn complex_2_level_not() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("-tag:foo", &tasks).unwrap();
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        assert!(titles.contains(&"task2"));
        assert!(titles.contains(&"task5"));
        assert!(titles.contains(&"task6"));
        assert!(!titles.contains(&"task1"));
    }

    #[test]
    fn complex_3_level_and_not() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("tag:foo AND property:priority=high AND -tag:bar", &tasks).unwrap();
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(result.len(), 3);
        assert!(titles.contains(&"task1"));
        assert!(titles.contains(&"task8"));
    }

    #[test]
    fn complex_3_level_or_not() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("tag:foo OR (tag:bar AND -property:priority=low)", &tasks).unwrap();
        assert!(result.len() >= 4);
    }

    #[test]
    fn complex_4_level_and_not() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("tag:foo AND property:priority=high AND -tag:bar", &tasks).unwrap();
        assert!(result.len() >= 1);
    }

    #[test]
    fn complex_5_level_nested_or() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("tag:foo AND (property:priority=high OR property:priority=medium) AND -tag:bar", &tasks).unwrap();
        assert!(result.len() >= 1);
    }

    #[test]
    fn complex_6_level_double_not() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("-tag:foo AND -tag:bar", &tasks).unwrap();
        assert!(result.len() >= 2);
    }

    #[test]
    fn complex_7_level_mixed() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("(tag:foo OR tag:bar) AND property:priority=high AND -state:DONE", &tasks).unwrap();
        assert!(result.len() >= 2);
    }

    #[test]
    fn complex_not_state_done() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("tag:foo AND -state:DONE", &tasks).unwrap();
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(result.len(), 5);
        assert!(!titles.contains(&"task11"));
    }

    #[test]
    fn complex_triple_and() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("tag:foo AND tag:bar AND property:priority=high", &tasks).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "task3");
    }

    #[test]
    fn complex_double_negator_and() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("-tag:foo AND -tag:bar", &tasks).unwrap();
        assert!(result.len() >= 2);
    }

    #[test]
    fn complex_or_with_and_not() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("(tag:foo OR tag:bar) AND -state:DONE", &tasks).unwrap();
        assert!(result.len() >= 4);
    }

    #[test]
    fn complex_not_property_high() {
        let tasks = load_tasks("query-complex");
        let result = parse_and_filter("-property:priority=high", &tasks).unwrap();
        assert!(result.len() >= 2);
    }

    #[test]
    fn query_and_or_precedence_left_side_only() {
        let tasks = load_tasks("query-and-or-precedence");
        let result = parse_and_filter("-state:DONE AND -state:CANCELLED AND -state:ARCHIVED", &tasks).unwrap();
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(result.len(), 2, "titles: {:?}", titles);
        assert!(titles.contains(&"task1"));
        assert!(titles.contains(&"task2"));
    }

    #[test]
    fn query_and_or_precedence_right_side_only() {
        let tasks = load_tasks("query-and-or-precedence");
        let result = parse_and_filter("property:scheduled=2026-05-12 OR property:scheduled=2026-05-13 OR property:scheduled=2026-05-14 OR property:scheduled=2026-05-15 OR property:scheduled=2026-05-16", &tasks).unwrap();
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(result.len(), 5, "titles: {:?}", titles);
        assert!(titles.contains(&"task1"));
        assert!(titles.contains(&"task2"));
        assert!(titles.contains(&"task3"));
        assert!(titles.contains(&"task4"));
        assert!(titles.contains(&"task5"));
    }

    #[test]
    fn query_and_or_precedence_full_query() {
        let tasks = load_tasks("query-and-or-precedence");
        let result = parse_and_filter("(-state:DONE AND -state:CANCELLED AND -state:ARCHIVED) OR (property:scheduled=2026-05-12 OR property:scheduled=2026-05-13 OR property:scheduled=2026-05-14 OR property:scheduled=2026-05-15 OR property:scheduled=2026-05-16)", &tasks).unwrap();
        let titles: Vec<_> = result.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(result.len(), 5, "titles: {:?}", titles);
        assert!(titles.contains(&"task1"));
        assert!(titles.contains(&"task2"));
        assert!(titles.contains(&"task3"));
        assert!(titles.contains(&"task4"));
        assert!(titles.contains(&"task5"));
        assert!(!titles.contains(&"task6"));
    }
}