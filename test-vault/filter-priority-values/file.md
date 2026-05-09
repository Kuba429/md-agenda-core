* #TODO task1 @priority(low)
* #TODO task2 @priority(medium)
* #TODO task3 @priority(high)
* #TODO task4
* #TODO task5 @priority(urgent)

# Test Case: filter-priority-values
### assumptions
* query priority=low → 1 task (task1)
* query priority=medium → 1 task (task2)
* query priority=high → 1 task (task3)
* query priority=urgent → 1 task (task5)
* query priority=none → 1 task (task4 - no priority set)