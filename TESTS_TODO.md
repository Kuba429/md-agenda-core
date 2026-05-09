# Test Scenario Ideas

## Task Body & Content

- [x] **body-trim-whitespace** - Task with extra leading/trailing whitespace in body - verify body is properly trimmed when retrieved
- [x] **body-multiline** - Task with multiline body - verify all lines are captured
- [x] **body-with-code** - Task body containing markdown code blocks - verify they're preserved

## Parent/Children Operations

- [x] **parent-deep-nesting** - 4+ levels of nesting - verify all ancestors can be queried
- [x] **parent-no-children** - Parent with no children - verify empty array returned
- [x] **parent-filtered** - Parent with children filtered by multiple criteria (state + tag)

## Include Children (already in vault)

- [x] include-children-basic
- [x] include-children-filtered
- [x] include-children-nested

## Include Ancestors

- [ ] **include-ancestors-basic** - Task with parent/grandparent - verify ancestors are included when flag is set
- [ ] **include-ancestors-filtered** - Include ancestors combined with tag/state filters
- [ ] **include-ancestors-dedup** - Multiple children share same ancestor - verify no duplicates

## Filtering

- [x] filter (state, tag, property)
- [x] query (complex DSL with AND/OR/NOT)

### Additional Filtering Scenarios

- [x] **filter-multi-property** - Filter by multiple properties at once (e.g., scheduled + priority)
- [x] **filter-content-search** - Search task title (not body - not implemented)
- [x] **filter-date-range** - Filter by exact date match (range operators not implemented)
- [x] **filter-empty-results** - Filters that return zero tasks
- [x] **filter-priority-values** - Different priority values (low, medium, high)

### Complex Queries (negators, AND/OR/NOT, 7+ leaf nesting)

- [x] Complex queries with negators, AND/OR combinations up to 7 levels

## Task Modification

- [ ] **set-state-cycling** - Cycle through all states: TODO -> NEXT -> IN_PROGRESS -> DONE -> LATER -> WAIT
- [ ] **set-property-new** - Add new property to task that didn't have it
- [ ] **set-property-overwrite** - Change existing property value
- [ ] **set-tags-add** - Add tags to task without overwriting existing
- [ ] **set-tags-remove** - Remove specific tags from task

## Task Creation (Capture)

- [ ] **capture-simple** - Add simple task to root
- [ ] **capture-with-parent** - Add task as child of existing task
- [ ] **capture-with-tags** - Add task with initial tags
- [ ] **capture-with-properties** - Add task with scheduled/priority properties

## Edge Cases

- [ ] **edge-no-tasks** - File with no tasks (only regular text)
- [ ] **edge-all-done** - All tasks are DONE - verify filtering works
- [ ] **edge-only-children** - No root-level tasks, only nested children
- [ ] **edge-non-task-bullets** - Bullet points without state tags (shouldn't be tasks)
- [ ] **edge-deep-indent** - Very deep indentation (8+ levels)

## Sorting

- [ ] **sort-by-state** - Tasks sorted by state order
- [ ] **sort-by-scheduled** - Tasks sorted by scheduled date
- [ ] **sort-by-priority** - Tasks sorted by priority
- [ ] **sort-by-filename** - Tasks sorted by source file

## Complex Queries

- [ ] **query-complex-and-or-not** - Complex boolean logic with multiple AND/OR/NOT
- [ ] **query-property-existence** - Query tasks that have ANY property (existence check)
- [ ] **query-negation** - NOT tag:foo, NOT state:DONE, etc.
