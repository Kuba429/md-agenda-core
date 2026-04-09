#![cfg(test)]

use std::path::{Path, PathBuf};

use crate::backlinks::{
    build_backlink_section, collect_backlinks, collect_backlinks_using_index, generate_backlinks,
    scan_vault_files, LinkIndex,
};
use crate::config::AgendaConfig;
use crate::grep::{tasks_grep, tasks_grep_property};
use crate::task::{task_capture, task_change_property, task_change_state, task_get_by_id, task_set_fields, tasks_ids_get};

#[test]
fn test_integration_with_context() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);
    let backlinks = collect_backlinks("target", &files);
    let with_context = backlinks.iter().find(|b| b.source_file == "with-context");
    assert!(with_context.is_some());
    let blocks = &with_context.unwrap().blocks;
    assert!(!blocks.is_empty());
    let has_context = blocks.iter().any(|b| b.lines.len() > 1);
    assert!(has_context, "should contain context lines");
}

#[test]
fn test_integration_nested_bullets() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);
    let backlinks = collect_backlinks("target", &files);
    let nested = backlinks.iter().find(|b| b.source_file == "nested");
    assert!(nested.is_some());
    let blocks = &nested.unwrap().blocks;
    assert!(!blocks.is_empty());
}

#[test]
fn test_integration_multiple_sources() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);
    let backlinks = collect_backlinks("target", &files);
    let sources: Vec<&str> = backlinks.iter().map(|b| b.source_file.as_str()).collect();
    assert!(sources.len() >= 2);
}

#[test]
fn test_integration_link_only_excluded() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);
    let backlinks = collect_backlinks("target", &files);

    let section = build_backlink_section(&backlinks);

    assert!(
        section.contains("[[link-only]]"),
        "link-only should be included in rendered output"
    );
}

#[test]
fn test_integration_link_only_included() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);
    let backlinks = collect_backlinks("a-link-without-children", &files);

    let section = build_backlink_section(&backlinks);

    assert!(
        section.contains("[[no-children]]"),
        "link-only should now be included in backlinks"
    );
}

#[test]
fn test_collect_backlinks_index_vs_parallel_parity() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);
    let index = LinkIndex::build(&files);

    let parallel_backlinks = collect_backlinks("target", &files);
    let index_backlinks = collect_backlinks_using_index("target", &index, &files);

    assert_eq!(
        parallel_backlinks.len(),
        index_backlinks.len(),
        "same number of source files"
    );

    let parallel_sources: Vec<_> = parallel_backlinks.iter().map(|b| &b.source_file).collect();
    let index_sources: Vec<_> = index_backlinks.iter().map(|b| &b.source_file).collect();
    assert_eq!(parallel_sources, index_sources, "same source files");

    for (p, i) in parallel_backlinks.iter().zip(index_backlinks.iter()) {
        assert_eq!(
            p.blocks.len(),
            i.blocks.len(),
            "same number of blocks per source"
        );
    }
}

#[test]
fn test_collect_backlinks_index_no_unrelated_blocks_regression() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);
    let index = LinkIndex::build(&files);
    let backlinks = collect_backlinks_using_index("target", &index, &files);

    assert!(!backlinks.is_empty());
}

#[test]
fn test_collect_backlinks_index_nested_bullets() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);
    let index = LinkIndex::build(&files);

    let parallel_backlinks = collect_backlinks("target", &files);
    let index_backlinks = collect_backlinks_using_index("target", &index, &files);

    let nested_parallel = parallel_backlinks
        .iter()
        .find(|b| b.source_file == "nested");
    let nested_index = index_backlinks.iter().find(|b| b.source_file == "nested");

    if nested_parallel.is_some() {
        assert!(nested_index.is_some());
        assert_eq!(
            nested_parallel.unwrap().blocks.len(),
            nested_index.unwrap().blocks.len(),
            "nested blocks count should match"
        );
    }
}

#[test]
fn test_parent_link_backlink_section() {
    let vault_path = Path::new("test-vault/parent-link-test");
    let files = scan_vault_files(vault_path);

    let backlinks = collect_backlinks("target-new", &files);

    assert_eq!(backlinks.len(), 2, "should have 2 source files");

    let section = build_backlink_section(&backlinks);

    assert!(section.contains("[[bar]]"), "should include bar");
    assert!(section.contains("[[foo]]"), "should include foo");

    assert!(
        !section.contains("* [[target-new]]"),
        "link-only line should be skipped"
    );
    assert!(
        section.contains("this should be included"),
        "should include child context"
    );
}

#[test]
fn test_double_link_single_block_both_targets() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);

    let backlinks_target = collect_backlinks("target", &files);
    let backlinks_target2 = collect_backlinks("target2", &files);

    let double_link = backlinks_target
        .iter()
        .find(|b| b.source_file == "double-link");
    assert!(
        double_link.is_some(),
        "double-link should be in target's backlinks"
    );

    let double_link2 = backlinks_target2
        .iter()
        .find(|b| b.source_file == "double-link");
    assert!(
        double_link2.is_some(),
        "double-link should be in target2's backlinks"
    );
}

#[test]
fn test_double_link_header_omitted_when_only_link() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);

    let backlinks = collect_backlinks("target", &files);
    let section = build_backlink_section(&backlinks);

    assert!(
        section.contains("[[double-link]]"),
        "double-link should be included"
    );
    let double_link_in_section = backlinks.iter().find(|b| b.source_file == "double-link");
    assert!(
        double_link_in_section.is_some(),
        "double-link should have backlink"
    );
}

#[test]
fn test_double_link_context_included_with_other_links() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);

    let backlinks = collect_backlinks("target", &files);
    let double_link = backlinks.iter().find(|b| b.source_file == "double-link");

    assert!(double_link.is_some());
    let blocks = &double_link.unwrap().blocks;
    assert!(!blocks.is_empty(), "should have blocks with context");
}

#[test]
fn test_nested_link_only_includes_direct_children() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);

    let backlinks = collect_backlinks("target", &files);
    let section = build_backlink_section(&backlinks);

    let parent_text_count = section.matches("this is a parent of a link").count();

    assert_eq!(
        parent_text_count, 0,
        "should NOT include parent text bullets before the link"
    );
}

#[test]
fn test_multi_link_sibling_bullets_not_included() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);

    let backlinks = collect_backlinks("target", &files);
    let section = build_backlink_section(&backlinks);

    assert!(
        !section.contains("it should't be included"),
        "should NOT include sibling bullets of link"
    );
    assert!(
        !section.contains("only bullets under the link bullet should be included"),
        "should NOT include sibling bullets of link"
    );
    assert!(
        !section.contains("this one should NOT be included"),
        "should NOT include bullets not directly under link"
    );

    assert!(
        section.contains("like this one"),
        "should include direct child of link"
    );
}

#[test]
fn test_duplicate_link_from_same_file_separate_bullets() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);

    let backlinks = collect_backlinks("target", &files);

    let test_file = backlinks
        .iter()
        .find(|b| b.source_file == "backlink-tasks-test");
    assert!(
        test_file.is_some(),
        "backlink-tasks-test should be in backlinks"
    );

    let blocks = &test_file.unwrap().blocks;
    assert!(blocks.len() >= 3, "should have at least 3 separate blocks");

    let section = build_backlink_section(&backlinks);
    assert!(
        section.contains("backlink-tasks-test"),
        "should include backlink-tasks-test in section"
    );
}

#[test]
fn test_multi_link_bullet_properly_indented() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);

    let backlinks = collect_backlinks("target", &files);
    let section = build_backlink_section(&backlinks);

    let double_link_section = backlinks
        .iter()
        .find(|b| b.source_file == "double-link")
        .map(|b| {
            let start = section
                .find(&format!("- [[{}]]", b.source_file))
                .unwrap_or(0);
            &section[start..]
        });

    if let Some(ds) = double_link_section {
        let lines: Vec<&str> = ds.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.contains("* [[target]]") {
                let indent = line.len() - line.trim_start().len();
                assert!(
                    indent > 0,
                    "multi-link bullet should be indented, got indent {}",
                    indent
                );
                if i + 1 < lines.len() {
                    let next_line = lines[i + 1];
                    if next_line.trim().starts_with('*') {
                        let next_indent = next_line.len() - next_line.trim_start().len();
                        assert!(
                            next_indent > indent,
                            "child should be more indented than parent"
                        );
                    }
                }
                break;
            }
        }
    }
}

#[test]
fn test_tasks_grep_excludes_backlinks_section() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = tasks_grep(&config, None).unwrap();

    let backlink_tasks = vec!["with-context.md:7", "with-context.md:8"];

    for task_ref in backlink_tasks {
        assert!(
            !result.contains(task_ref),
            "should not find task {} in backlinks section",
            task_ref
        );
    }
}

#[test]
fn test_tasks_grep_excludes_backlinks_in_multiple_files() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = tasks_grep(&config, None).unwrap();

    assert!(result.contains("perf-test-1.md:3"), "should find task 1");
    assert!(result.contains("perf-test-1.md:4"), "should find task 2");
    assert!(
        result.contains("perf-test-1.md:5"),
        "should find task 3 (IN_PROGRESS)"
    );

    assert!(
        !result.contains("perf-test-1.md:8"),
        "should NOT find task in backlinks"
    );
    assert!(
        !result.contains("perf-test-1.md:9"),
        "should NOT find DONE in backlinks"
    );

    assert!(result.contains("perf-test-2.md:3"), "should find Task A");
    assert!(
        result.contains("perf-test-2.md:4"),
        "should find Task B (DONE)"
    );

    assert!(
        !result.contains("perf-test-2.md:8"),
        "should NOT find backlink task"
    );
}

#[test]
fn test_tasks_grep_property_scheduled() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = tasks_grep_property(&config, "scheduled").unwrap();

    assert!(result.contains("@scheduled(2024-01-01)"));
    assert!(result.contains("@scheduled(2024-01-03)"));
}

#[test]
fn test_tasks_grep_property_priority() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = tasks_grep_property(&config, "priority").unwrap();

    assert!(result.contains("@priority(1)"));
    assert!(result.contains("@priority(2)"));
}

#[test]
fn test_task_ids_get_returns_ids() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let ids = tasks_ids_get(&config);
    assert!(!ids.is_empty(), "should get task ids");
}

#[test]
fn test_collect_backlinks_nested_context_lines() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);
    let backlinks = collect_backlinks("test1-target", &files);
    let test1 = backlinks.iter().find(|b| b.source_file == "test1");
    assert!(test1.is_some(), "test1 should be in backlinks");
    let blocks = &test1.unwrap().blocks;
    assert!(!blocks.is_empty(), "should have blocks");

    let section = build_backlink_section(&backlinks);
    assert!(
        section.contains("this line should be included"),
        "should include nested context lines"
    );
    assert!(
        !section.contains("this line should NOT be included"),
        "should NOT include sibling bullets of link"
    );
}

#[test]
fn test_generate_backlinks_creates_missing_file() {
    let temp_dir = std::env::temp_dir().join("test_create_missing");
    std::fs::create_dir_all(&temp_dir).ok();

    let test_file = temp_dir.join("source.md");
    std::fs::write(&test_file, "- [[new target file]]\n    * Some context").ok();

    let result = generate_backlinks(temp_dir.to_str().unwrap(), "new target file");

    assert!(result.is_ok());

    let new_file = temp_dir.join("new-target-file.md");
    assert!(new_file.exists(), "should create new target file");

    let content = std::fs::read_to_string(&new_file).unwrap();
    assert!(content.contains("# new target file"));
    assert!(content.contains("[[source]]"));

    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_backlink_context_includes_only_link_tree() {
    let vault_path = Path::new("test-vault");
    let files = scan_vault_files(vault_path);
    let backlinks = collect_backlinks("test2-target", &files);
    let test2 = backlinks.iter().find(|b| b.source_file == "test2");
    assert!(test2.is_some(), "test2 should be in backlinks");
    let blocks = &test2.unwrap().blocks;
    assert!(!blocks.is_empty(), "should have blocks");

    let section = build_backlink_section(&backlinks);
    assert!(
        section.contains("should be included"),
        "should include lines under the link"
    );
    assert!(
        !section.contains("this line is not a child"),
        "should NOT include lines before/beside the link"
    );
    assert!(
        !section.contains("should NOT be included"),
        "should NOT include sibling bullets of link"
    );
}

// Test scenarios from test-vault/test-filtering.md

#[test]
fn test_scenario_6_tag_filter_bug() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));
    use crate::grep::tasks_grep_tag;
    use std::collections::HashSet;

    let mut excluded = HashSet::new();
    excluded.insert("DONE".to_string());
    excluded.insert("CANCELLED".to_string());

    let result = tasks_grep_tag(&config, "bug", Some(&excluded)).unwrap();

    assert!(
        result.contains("test-case-6.md:3"),
        "should find first bug task"
    );
    assert!(
        result.contains("test-case-6.md:5"),
        "should find second bug task"
    );
    assert!(
        !result.contains("test-case-6.md:4"),
        "should NOT find feature task"
    );
}

#[test]
fn test_scenario_8_property_key_filter() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = tasks_grep_property(&config, "scheduled").unwrap();

    assert!(
        result.contains("test-case-8.md:3"),
        "should find task with scheduled"
    );
    assert!(
        result.contains("@scheduled(2024-01-15)"),
        "should find scheduled property"
    );
    assert!(
        !result.contains("test-case-8.md:5"),
        "should NOT find task without scheduled"
    );
}

#[test]
fn test_scenario_10_no_tasks_match_filter() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));
    use crate::grep::tasks_grep;
    use std::collections::HashSet;

    let mut exclude_set = HashSet::new();
    exclude_set.insert("DONE".to_string());
    exclude_set.insert("CANCELLED".to_string());

    let result = tasks_grep(&config, Some(&exclude_set)).unwrap();

    assert!(
        !result.contains("test-case-10.md"),
        "should NOT find only done/cancelled tasks"
    );
}

#[test]
fn test_scenario_12_backlink_exclusion() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));
    use crate::grep::tasks_grep;
    use std::collections::HashSet;

    let mut exclude_set = HashSet::new();
    exclude_set.insert("DONE".to_string());
    exclude_set.insert("CANCELLED".to_string());

    let result = tasks_grep(&config, Some(&exclude_set)).unwrap();

    assert!(
        result.contains("test-case-12.md:3"),
        "should find task before backlinks"
    );
    assert!(
        !result.contains("test-case-12.md:6"),
        "should NOT find task in backlinks section"
    );
}

#[test]
fn test_scenario_14_property_key_value_scheduled() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = tasks_grep_property(&config, "scheduled=2024-01-15").unwrap();

    assert!(
        result.contains("test-case-14.md:3"),
        "should find task with 2024-01-15"
    );
    assert!(
        !result.contains("test-case-14.md:4"),
        "should NOT find task with 2024-01-20"
    );
}

#[test]
fn test_scenario_15_property_key_value_priority() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = tasks_grep_property(&config, "priority=high").unwrap();

    assert!(
        result.contains("test-case-15.md:3"),
        "should find first high priority"
    );
    assert!(
        result.contains("test-case-15.md:5"),
        "should find second high priority"
    );
    assert!(
        !result.contains("test-case-15.md:4"),
        "should NOT find low priority task"
    );
}

#[test]
fn test_property_filter_excludes_non_task_bullets() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = tasks_grep_property(&config, "scheduled").unwrap();

    assert!(
        !result.contains("prop-test.md:1"),
        "should NOT find non-task bullet without state tag (line 1)"
    );
    assert!(
        !result.contains("prop-test.md:2"),
        "should NOT find non-task bullet without state tag (line 2)"
    );
    assert!(
        !result.contains("prop-test.md:3"),
        "should NOT find non-task bullet without state tag (line 3)"
    );
    assert!(
        !result.contains("prop-test.md:5"),
        "should NOT find non-task bullet without state tag (line 5)"
    );
    assert!(
        !result.contains("prop-test.md:6"),
        "should NOT find non-task bullet without state tag (line 6)"
    );
}

#[test]
fn test_property_filter_excludes_non_task_bullets_simple() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = tasks_grep_property(&config, "scheduled").unwrap();

    assert!(
        !result.contains("prop-test-simple.md:1"),
        "should NOT find non-task bullet with @scheduled but no state tag"
    );
    assert!(
        !result.contains("prop-test-simple.md:2"),
        "should NOT find non-task bullet with @scheduled and #foo (non-state tag)"
    );
    assert!(
        !result.contains("prop-test-simple.md:3"),
        "should NOT find non-task bullet with @scheduled but no state tag"
    );
}

#[test]
fn test_property_filter_includes_real_tasks_with_property() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = tasks_grep_property(&config, "scheduled=2024-01-01").unwrap();

    assert!(
        result.contains("perf-test-1.md:3"),
        "should find real task with #TODO and @scheduled"
    );
}

#[test]
fn test_include_ancestors_builds_path_to_root() {
    use crate::cli::get_task_ancestors;
    use crate::task::tasks_get;

    let config = AgendaConfig::new(PathBuf::from("test-vault"));
    let tasks = tasks_get(&config);

    let grandchild = tasks
        .iter()
        .find(|t| t.id == "test-ancestry-path.md:3")
        .expect("grandchild not found");
    let rooted = get_task_ancestors(&config, grandchild);

    assert_eq!(
        rooted.id, "test-ancestry-path.md:1",
        "root should be deep root"
    );
    assert_eq!(
        rooted.children.len(),
        1,
        "root should have exactly one child (the ancestry path)"
    );
    assert_eq!(
        rooted.children[0].id, "test-ancestry-path.md:2",
        "child should be deep child"
    );
    assert_eq!(
        rooted.children[0].children.len(),
        1,
        "deep child should have exactly one child"
    );
    assert_eq!(
        rooted.children[0].children[0].id, "test-ancestry-path.md:3",
        "grandchild should be at leaf"
    );
}

#[test]
fn test_include_ancestors_no_siblings_in_path() {
    use crate::cli::get_task_ancestors;
    use crate::task::tasks_get;

    let config = AgendaConfig::new(PathBuf::from("test-vault"));
    let tasks = tasks_get(&config);

    let grandchild = tasks
        .iter()
        .find(|t| t.id == "test-ancestry-path.md:3")
        .expect("grandchild not found");
    let rooted = get_task_ancestors(&config, grandchild);

    let deep_child = &rooted.children[0];
    let has_sibling = deep_child
        .children
        .iter()
        .any(|c| c.id == "test-ancestry-path.md:4");
    assert!(!has_sibling, "ancestry path should NOT include siblings");
}

#[test]
fn test_include_ancestors_root_task_unchanged() {
    use crate::cli::get_task_ancestors;
    use crate::task::tasks_get;

    let config = AgendaConfig::new(PathBuf::from("test-vault"));
    let tasks = tasks_get(&config);

    let root = tasks
        .iter()
        .find(|t| t.id == "test-ancestry-path.md:1")
        .expect("root not found");
    let rooted = get_task_ancestors(&config, root);

    assert_eq!(
        rooted.id, "test-ancestry-path.md:1",
        "root task should remain itself"
    );
    assert_eq!(
        rooted.children.len(),
        root.children.len(),
        "root task children should be unchanged"
    );
}

#[test]
fn test_change_state_updates_task() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = task_change_state(&config, "test-state-1.md:1", "IN_PROGRESS");
    assert!(result.is_ok(), "state change should succeed");

    let task = task_get_by_id(&config, "test-state-1.md:1");
    assert_eq!(task.state, "IN_PROGRESS", "state should be IN_PROGRESS");
    assert!(
        task.body.contains("#IN_PROGRESS"),
        "body should contain #IN_PROGRESS"
    );

    let _ = task_change_state(&config, "test-state-1.md:1", "TODO");
}

#[test]
fn test_change_state_cycle() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let _ = task_change_state(&config, "test-state-2.md:1", "IN_PROGRESS");
    let task = task_get_by_id(&config, "test-state-2.md:1");
    assert_eq!(task.state, "IN_PROGRESS");

    let _ = task_change_state(&config, "test-state-2.md:1", "DONE");
    let task = task_get_by_id(&config, "test-state-2.md:1");
    assert_eq!(task.state, "DONE");

    let _ = task_change_state(&config, "test-state-2.md:1", "TODO");
}

#[test]
fn test_change_state_preserves_properties() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let _ = task_change_state(&config, "test-state-3.md:1", "DONE");
    let task = task_get_by_id(&config, "test-state-3.md:1");
    assert_eq!(task.state, "DONE", "state should be DONE after change");
    assert_eq!(
        task.properties.get("scheduled"),
        Some(&"2024-06-01".to_string()),
        "properties should be preserved after state change"
    );

    let _ = task_change_state(&config, "test-state-3.md:1", "TODO");
}

#[test]
fn test_change_property_add_and_retrieve() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = task_change_property(&config, "test-state-4.md:1", "priority", Some("high"));
    assert!(result.is_ok(), "property change should succeed");

    let task = task_get_by_id(&config, "test-state-4.md:1");
    assert_eq!(
        task.properties.get("priority"),
        Some(&"high".to_string()),
        "priority property should be added"
    );

    let _ = task_change_property(&config, "test-state-4.md:1", "priority", None);
}

#[test]
fn test_change_property_update_and_retrieve() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let _ = task_change_property(
        &config,
        "test-state-5.md:1",
        "scheduled",
        Some("2025-12-25"),
    );
    let task = task_get_by_id(&config, "test-state-5.md:1");
    assert_eq!(
        task.properties.get("scheduled"),
        Some(&"2025-12-25".to_string()),
        "scheduled property should be updated"
    );

    let _ = task_change_property(
        &config,
        "test-state-5.md:1",
        "scheduled",
        Some("2024-06-01"),
    );
}

#[test]
fn test_change_property_remove_and_retrieve() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let _ = task_change_property(&config, "test-state-6.md:1", "scheduled", None);
    let task = task_get_by_id(&config, "test-state-6.md:1");
    assert!(
        !task.properties.contains_key("scheduled"),
        "scheduled property should be removed"
    );

    let _ = task_change_property(
        &config,
        "test-state-6.md:1",
        "scheduled",
        Some("2024-06-01"),
    );
}

#[test]
fn test_change_state_returns_updated_task_via_cli() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let _ = task_change_state(&config, "test-state-7.md:1", "IN_PROGRESS");
    let task = task_get_by_id(&config, "test-state-7.md:1");
    assert_eq!(task.id, "test-state-7.md:1");
    assert_eq!(task.state, "IN_PROGRESS");
    assert!(
        task.children.is_empty(),
        "returned task should have no children"
    );

    let _ = task_change_state(&config, "test-state-7.md:1", "TODO");
}

#[test]
fn test_include_ancestors_deduplicates_child_in_results() {
    use crate::cli::{collect_child_ids, get_task_ancestors, merge_task_trees};
    use crate::task::task_get_by_id;
    use std::collections::HashSet;

    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let parent = task_get_by_id(&config, "test-ancestor-dedup.md:1");
    let child = task_get_by_id(&config, "test-ancestor-dedup.md:4");

    let rooted_parent = get_task_ancestors(&config, &parent);
    let rooted_child = get_task_ancestors(&config, &child);

    let merged = merge_task_trees(vec![rooted_parent, rooted_child]);

    let mut non_roots: HashSet<String> = HashSet::new();
    for root in &merged {
        collect_child_ids(root, &mut non_roots);
    }
    let deduped: Vec<_> = merged
        .into_iter()
        .filter(|t| !non_roots.contains(&t.id))
        .collect();

    assert_eq!(deduped.len(), 1, "should have exactly one root after dedup");
    assert_eq!(
        deduped[0].id, "test-ancestor-dedup.md:1",
        "root should be the parent"
    );
}

#[test]
fn test_property_date_matches_datetime() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = tasks_grep_property(&config, "scheduled=2026-04-13").unwrap();

    assert!(
        result.contains("test-date-filter.md:1"),
        "should find task with date-only scheduled"
    );
    assert!(
        result.contains("test-date-filter.md:2"),
        "should find task with datetime scheduled when filtering by date only"
    );
    assert!(
        !result.contains("test-date-filter.md:3"),
        "should NOT find task with different date"
    );
}

#[test]
fn test_property_date_filter_in_memory() {
    use crate::task::{tasks_filter_by_property, tasks_get};
    let config = AgendaConfig::new(PathBuf::from("test-vault"));
    let tasks = tasks_get(&config);

    let filtered = tasks_filter_by_property(&tasks, "scheduled", Some("2026-04-13"));

    let date_only = filtered.iter().find(|t| t.id == "test-date-filter.md:1");
    let with_time = filtered.iter().find(|t| t.id == "test-date-filter.md:2");
    let different = filtered.iter().find(|t| t.id == "test-date-filter.md:3");

    assert!(date_only.is_some(), "date-only task should match");
    assert!(
        with_time.is_some(),
        "datetime task should match when filtering by date"
    );
    assert!(different.is_none(), "different date should not match");
}

#[test]
fn test_set_fields_content_only() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = task_set_fields(
        &config,
        "test-set-content.md:1",
        Some("updated task content"),
        None,
        None,
        None,
    );
    assert!(result.is_ok(), "set fields should succeed: {:?}", result);

    let task = task_get_by_id(&config, "test-set-content.md:1");
    assert_eq!(task.content, "updated task content");
    assert_eq!(task.state, "TODO");
    assert_eq!(task.properties.get("scheduled"), Some(&"2024-06-01".to_string()));

    let _ = task_set_fields(
        &config,
        "test-set-content.md:1",
        Some("set content test"),
        None,
        None,
        None,
    );
}

#[test]
fn test_set_fields_state_only() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let result = task_set_fields(
        &config,
        "test-set-state.md:1",
        None,
        Some("DONE"),
        None,
        None,
    );
    assert!(result.is_ok(), "set fields state should succeed: {:?}", result);

    let task = task_get_by_id(&config, "test-set-state.md:1");
    assert_eq!(task.state, "DONE");
    assert_eq!(task.content, "set state test");
    assert_eq!(task.properties.get("scheduled"), Some(&"2024-06-01".to_string()));

    let _ = task_set_fields(
        &config,
        "test-set-state.md:1",
        None,
        Some("TODO"),
        None,
        None,
    );
}

#[test]
fn test_set_fields_properties_merge() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let mut props = std::collections::HashMap::new();
    props.insert("priority".to_string(), "1".to_string());
    props.insert("scheduled".to_string(), "2025-12-25".to_string());

    let result = task_set_fields(
        &config,
        "test-set-props.md:1",
        None,
        None,
        Some(&props),
        None,
    );
    assert!(result.is_ok(), "set fields properties should succeed: {:?}", result);

    let task = task_get_by_id(&config, "test-set-props.md:1");
    assert_eq!(task.properties.get("priority"), Some(&"1".to_string()));
    assert_eq!(task.properties.get("scheduled"), Some(&"2025-12-25".to_string()));

    let _ = task_change_property(&config, "test-set-props.md:1", "priority", None);
    let _ = task_change_property(&config, "test-set-props.md:1", "scheduled", Some("2024-06-01"));
}

#[test]
fn test_set_fields_tags_replace() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let tags = vec!["urgent".to_string(), "release".to_string()];

    let result = task_set_fields(
        &config,
        "test-set-tags.md:1",
        None,
        None,
        None,
        Some(&tags),
    );
    assert!(result.is_ok(), "set fields tags should succeed: {:?}", result);

    let task = task_get_by_id(&config, "test-set-tags.md:1");
    assert_eq!(task.tags, vec!["urgent", "release"]);
    assert_eq!(task.state, "IN_PROGRESS");
    assert_eq!(task.properties.get("priority"), Some(&"high".to_string()));

    let original_tags = vec!["bug".to_string()];
    let _ = task_set_fields(
        &config,
        "test-set-tags.md:1",
        None,
        None,
        None,
        Some(&original_tags),
    );
}

#[test]
fn test_set_fields_multiple_fields() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));

    let mut props = std::collections::HashMap::new();
    props.insert("due".to_string(), "2025-01-01".to_string());

    let tags = vec!["important".to_string()];

    let result = task_set_fields(
        &config,
        "test-set-multi.md:1",
        Some("completely new content"),
        Some("IN_PROGRESS"),
        Some(&props),
        Some(&tags),
    );
    assert!(result.is_ok(), "set fields multiple should succeed: {:?}", result);

    let task = task_get_by_id(&config, "test-set-multi.md:1");
    assert_eq!(task.content, "completely new content");
    assert_eq!(task.state, "IN_PROGRESS");
    assert_eq!(task.properties.get("scheduled"), Some(&"2024-06-01".to_string()));
    assert_eq!(task.properties.get("due"), Some(&"2025-01-01".to_string()));
    assert!(task.tags.contains(&"important".to_string()));

    let _ = task_set_fields(
        &config,
        "test-set-multi.md:1",
        Some("set multi test"),
        Some("TODO"),
        None,
        None,
    );
    let _ = task_change_property(&config, "test-set-multi.md:1", "due", None);
}

#[test]
fn test_capture_to_file() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));
    let result = task_capture(
        &config,
        "integration test task",
        "test-capture-integration.md",
        None,
        &std::collections::HashMap::new(),
        &["test".to_string()],
    );
    assert!(result.is_ok(), "capture to file should succeed: {:?}", result);
    let task_id = result.unwrap();
    assert!(task_id.starts_with("test-capture-integration.md:"));

    let _ = std::fs::remove_file("test-vault/test-capture-integration.md");
}

#[test]
fn test_capture_to_task_id() {
    use std::fs;

    let config = AgendaConfig::new(PathBuf::from("test-vault"));
    fs::write(
        "test-vault/test-capture-parent.md",
        "* #TODO parent task",
    ).unwrap();

    let result = task_capture(
        &config,
        "subtask from integration test",
        "test-capture-parent.md:1",
        Some("IN_PROGRESS"),
        &std::collections::HashMap::new(),
        &[],
    );
    assert!(result.is_ok(), "capture to task ID should succeed: {:?}", result);
    let task_id = result.unwrap();
    assert!(task_id.starts_with("test-capture-parent.md:2"));

    fs::write(
        "test-vault/test-capture-parent.md",
        "* #TODO parent task\n  * #TODO subtask from integration test",
    ).unwrap();
    let content = fs::read_to_string("test-vault/test-capture-parent.md").unwrap();
    assert!(content.contains("subtask from integration test"));
    assert!(content.contains("IN_PROGRESS"));

    let _ = std::fs::remove_file("test-vault/test-capture-parent.md");
}

#[test]
fn test_capture_invalid_task_id() {
    let config = AgendaConfig::new(PathBuf::from("test-vault"));
    let result = task_capture(
        &config,
        "should fail",
        "nonexistent.md:99",
        None,
        &std::collections::HashMap::new(),
        &[],
    );
    assert!(result.is_err());
}
