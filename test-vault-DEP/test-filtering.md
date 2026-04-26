# Test Cases for Filtering Optimization

This file contains test cases to ensure the filtering optimization works correctly.

## State Filtering Tests

### Test 1: Include only TODO state
- Task file: should contain TODO tasks
- Expected: only TODO tasks are returned (grepped)
- Command: `agenda-core get --state TODO`
- Pattern: `#TODO` (only)

### Test 2: Include multiple states (TODO, IN_PROGRESS)
- Task file: contains TODO, IN_PROGRESS, DONE tasks
- Expected: only TODO and IN_PROGRESS tasks are grepped
- Command: `agenda-core get --state TODO IN_PROGRESS`
- Pattern: `#TODO|#IN_PROGRESS`

### Test 3: Exclude single state (DONE)
- Task file: contains TODO, IN_PROGRESS, DONE tasks
- Expected: TODO and IN_PROGRESS tasks are grepped (not DONE)
- Command: `agenda-core get --exclude-state DONE`
- Pattern: `#TODO|#IN_PROGRESS|#NEXT|#WAIT|#LATER`

### Test 4: Exclude multiple states (DONE, CANCELLED)
- Task file: contains TODO, DONE, CANCELLED tasks
- Expected: TODO tasks are grepped only
- Command: `agenda-core get --exclude-state DONE CANCELLED`
- Pattern: `#TODO|#IN_PROGRESS|#NEXT|#WAIT|#LATER`

### Test 5: Default behavior (exclude DONE, CANCELLED)
- Task file: contains TODO, IN_PROGRESS, DONE, CANCELLED
- Expected: TODO, IN_PROGRESS are grepped
- Pattern: `#TODO|#IN_PROGRESS|#NEXT|#WAIT|#LATER`

## Tag Filtering Tests

### Test 6: Filter by tag (bug)
- Task file: contains tasks with #bug, #feature tags
- Expected: only tasks with #bug are grepped
- Command: `agenda-core get --tag bug`
- Pattern includes `#bug` in grep

### Test 7: Filter by tag combined with state
- Task file: TODO tasks with #bug, #feature; DONE tasks with #bug
- Expected: TODO #bug tasks are grepped
- Command: `agenda-core get --tag bug --state TODO`
- Pattern: `#TODO` AND `#bug`

## Property Filtering Tests

### Test 8: Filter by property key (scheduled)
- Task file: tasks with @scheduled() and @due() properties
- Expected: tasks with @scheduled() are grepped
- Command: `agenda-core get --property scheduled`
- Pattern: `@scheduled\([^)]+\)` 

### Test 9: Filter by property key=value
- Task file: tasks with @scheduled(2024-01-15), @scheduled(2024-01-20)
- Expected: only tasks with @scheduled(2024-01-15) are grepped
- Command: `agenda-core get --property scheduled=2024-01-15`
- Pattern: `@scheduled\(2024-01-15\)`

## Edge Cases

### Test 10: No tasks match filter
- Task file: has TODO tasks, filter is for DONE only (but DONE is excluded)
- Expected: empty result, no panic

### Test 11: Case sensitivity
- Task file: contains #todo (lowercase)
- Expected: uppercase #TODO still matches (regex is case sensitive by default)
- Note: current implementation is case-sensitive

### Test 12: Tasks in backlink section should never be included
- Task file: task before <!-- BACKLINKS:START --> and task after
- Expected: only task before backlinks section is grepped
- This is already handled in the code

### Test 14: Property filter with key=value
- Task file: tasks with @scheduled(2024-01-15), @scheduled(2024-01-20), @priority(high)
- Command: `agenda-core get --property scheduled=2024-01-15`
- Expected: only tasks with @scheduled(2024-01-15) are returned

### Test 15: Property filter with key=value (different key)
- Task file: tasks with @priority(high), @priority(low)
- Command: `agenda-core get --property priority=high`
- Expected: only tasks with @priority(high) are returned