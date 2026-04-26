* #TODO task_with_tag_foo #foo
* #TODO task_with_tag_bar #bar
* #TODO task_with_both_tags #foo #bar
* #TODO task_with_foo_bar_tag #foo_bar
* #TODO task_with_property_prio @priority(high)
* #TODO task_with_tag_and_property #foo @priority(high)
* #TODO task_with_property_scheduled @scheduled(2024-01-15)
* #TODO meeting_notes_task
* #IN_PROGRESS task_in_progress #foo
* #DONE done_task #foo
* #TODO task_with_scheduled_time @scheduled(2024-01-15T10:30)

# Test Cases

## Test Case: query tasks with tag foo
### assumptions
* should return 6 tasks: task_with_tag_foo, task_with_both_tags, task_with_tag_and_property, task_in_progress, done_task, task_with_foo_bar_tag
* wait actually tag:foo matches tasks having the tag "foo"
* task_with_foo_bar_tag has tag "foo_bar" not "foo" → should NOT match
* 5 tasks should match: task_with_tag_foo, task_with_both_tags, task_with_tag_and_property, task_in_progress, done_task

## Test Case: query tasks with property priority=high
### assumptions
* 2 tasks should match: task_with_property_prio, task_with_tag_and_property

## Test Case: query tasks with tag foo AND property priority=high
### assumptions
* 1 task should match: task_with_tag_and_property

## Test Case: query tasks with tag foo OR tag bar
### assumptions
* 4 tasks should match: task_with_tag_foo, task_with_tag_bar, task_with_both_tags, task_with_tag_and_property, task_in_progress, done_task
* wait, tag:tag_with_both_tags has both foo and bar - counted once
* let me think: foo → task_with_tag_foo, task_with_both_tags, task_with_tag_and_property, task_in_progress, done_task = 5
* bar → task_with_tag_bar, task_with_both_tags = 2
* union = 6 unique: task_with_tag_foo, task_with_tag_bar, task_with_both_tags, task_with_tag_and_property, task_in_progress, done_task

## Test Case: query tasks with tag foo AND NOT tag bar
### assumptions
* should match tasks with tag foo that do NOT have tag bar
* matches: task_with_tag_foo, task_with_tag_and_property, task_in_progress, done_task = 4

## Test Case: query tasks with tag foo AND property priority=high AND NOT tag foo_bar
### assumptions
* task_with_tag_and_property has foo, has priority=high, does NOT have foo_bar → matches
* 1 task should match

## Test Case: query tasks with state TODO
### assumptions
* tasks with TODO state: task_with_tag_foo, task_with_tag_bar, task_with_both_tags, task_with_foo_bar_tag, task_with_property_prio, task_with_tag_and_property, task_with_property_scheduled, meeting_notes_task = 8

## Test Case: query tasks with (tag foo OR tag bar) AND state TODO
### assumptions
* TODO + foo: task_with_tag_foo, task_with_both_tags, task_with_tag_and_property
* TODO + bar: task_with_tag_bar, task_with_both_tags
* union = task_with_tag_foo, task_with_tag_bar, task_with_both_tags, task_with_tag_and_property = 4

## Test Case: query tasks with content meeting
### assumptions
* meeting_notes_task = 1

## Test Case: query tasks with NOT state DONE
### assumptions
* all except done_task = 9

## Test Case: query tasks with property scheduled (existence only)
### assumptions
* task_with_property_scheduled = 1

## Test Case: combined with existing flags
### assumptions
* query `property:priority=high` + existing `--tag foo` (AND) → 1 task: task_with_tag_and_property
