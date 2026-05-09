* #TODO task1 @scheduled(2024-01-01) @priority(low)
* #TODO task2 @scheduled(2024-01-01) @priority(high)
* #TODO task3 @scheduled(2024-01-02) @priority(high)
* #TODO task4 @priority(high)
* #TODO task5 @scheduled(2024-01-01)

# Test Case: filter-multi-property
### assumptions
* query with scheduled=2024-01-01 AND priority=high → 1 task (task2)
* query with priority=high → 2 tasks (task2, task3, task4) = 3 tasks (wait, task2, task3, task4)
* query with scheduled=2024-01-01 → 3 tasks (task1, task2, task5)
* query with scheduled=2024-01-01 AND priority=low → 1 task (task1)
* query with scheduled=2024-01-02 AND priority=high → 1 task (task3)