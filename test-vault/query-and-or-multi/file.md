# Test Case: query-and-or-multi

## Purpose
Test that OR between different query types (state exclusion, property values)
correctly loads ALL files that could match either branch, not just files
matching the "best" strategy.

## Tasks (across 2 files)

### file-a.md
- task_a: TODO, scheduled=2026-05-15
- task_b: DONE, scheduled=2026-05-16

### file-b.md
- task_c: TODO
- task_d: CANCELLED, scheduled=2026-05-16

## Edge Cases Covered

1. Property OR chain where first value doesn't exist in any file
   - Query: property:scheduled=2026-05-14 OR property:scheduled=2026-05-15
   - Should return task_a (has scheduled=2026-05-15)

2. State exclusion OR property chain
   - Query: (-state:DONE) OR (property:scheduled=2026-05-15 OR property:scheduled=2026-05-16)
   - Should return task_a (matches left via TODO), task_b (matches right via scheduled=16), task_c (matches left via TODO), task_d (matches right via scheduled=16)

3. Complex full query (user scenario)
   - Query: (-state:DONE AND -state:CANCELLED AND -state:ARCHIVED) OR (property:scheduled=2026-05-14 OR property:scheduled=2026-05-15 OR property:scheduled=2026-05-16)
   - Expected: task_a, task_b, task_c, task_d = 4 tasks

4. Non-existent property in OR
   - Query: property:scheduled=2026-05-99 OR property:scheduled=2026-05-98
   - Should return 0 tasks (no matches)

5. Same property value in OR
   - Query: property:scheduled=2026-05-15 OR property:scheduled=2026-05-15
   - Should return task_a (1 task)

6. OR between same strategy type (tag)
   - Query: tag:nonexistent OR tag:nonexistent2
   - Should return 0 tasks (no such tags)

## Expected Strategy Behavior

For queries where OR branches have different grep strategy types, or
different values of the same type, analyze_query should return
GrepStrategy::All to ensure all files are loaded before in-memory filtering.
