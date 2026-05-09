* #TODO task1 @scheduled(2026-05-12)
* #IN_PROGRESS task2 @scheduled(2026-05-13)
* #DONE task3 @scheduled(2026-05-14)
* #CANCELLED task4 @scheduled(2026-05-15)
* #ARCHIVED task5 @scheduled(2026-05-16)
* #DONE task6 @scheduled(2026-06-01)

# Test Case: query-and-or-precedence

## assumptions

* Query: (-state:DONE AND -state:CANCELLED AND -state:ARCHIVED) OR (property:scheduled=2026-05-12 OR property:scheduled=2026-05-13 OR property:scheduled=2026-05-14 OR property:scheduled=2026-05-15 OR property:scheduled=2026-05-16)
* Expected: 5 tasks (task1, task2, task3, task4, task5)
* Left side alone: 2 tasks (task1, task2) - only non-excluded states
* Right side alone: 5 tasks (task1, task2, task3, task4, task5) - all scheduled in range
* Combined with OR: should return all tasks that match either side
* key: tasks with excluded states (DONE, CANCELLED, ARCHIVED) that have scheduled dates in range should appear via the right side of OR