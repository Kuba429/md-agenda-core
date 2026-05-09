* #TODO task1 @scheduled(2024-01-01)
* #TODO task2 @scheduled(2024-01-15)
* #TODO task3 @scheduled(2024-02-01)
* #TODO task4 @scheduled(2024-03-01)
* #TODO task5 @scheduled(2024-01-10)

# Test Case: filter-date-range
### assumptions
* query scheduled=2024-01-15 → 1 task (task2)
* query scheduled=2024-01-01 OR scheduled=2024-01-15 → 2 tasks (task1, task2)
* query scheduled=2024-01-01 → 1 task (task1)
* query scheduled=2024-01-10 → 1 task (task5)