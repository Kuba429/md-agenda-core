* #TODO task1 @priority(high)
* #TODO task2 @priority(low)

# Test Case: filter-empty-results
### assumptions
* query state=DONE → 0 tasks (all are TODO)
* query priority=medium → 0 tasks (no medium priority)
* query tag=nonexistent → 0 tasks
* query content="nothing here" → 0 tasks