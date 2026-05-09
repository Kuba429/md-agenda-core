* #TODO task1 #foo @priority(high)
* #TODO task2 #bar @priority(low)
* #TODO task3 #foo #bar @priority(high)
* #TODO task4 #foo @priority(medium)
* #TODO task5 #bar @priority(high)
* #TODO task6
* #TODO task7 #baz
* #TODO task8 #foo @priority(high) @scheduled(2024-01-01)
* #TODO task9 #foo @priority(low) @scheduled(2024-01-02)
* #TODO task10 #bar @priority(high) @scheduled(2024-01-03)
* #DONE task11 #foo @priority(high)
* #DONE task12 #bar

# Test Cases

## 2-level: AND
### assumptions
* foo AND high → 3 tasks (task1, task3, task8)

## 2-level: OR
### assumptions
* foo OR bar → 5 tasks (task1, task2, task3, task4, task5)

## 2-level: NOT
### assumptions
* -foo → 6 tasks (task2, task5, task6, task7, task10, task12)

## 3-level: AND + NOT
### assumptions
* foo AND high AND -bar → 2 tasks (task1, task8)

## 3-level: OR + NOT
### assumptions
* foo OR bar AND -priority:low → 4 tasks (task1, task3, task4, task5, task8, task10)
* wait: foo OR (bar AND -priority:low)
* foo matches: task1, task3, task4, task8, task9
* bar AND -low: task5, task10 (bar with priority high)
* union: task1, task3, task4, task5, task8, task9, task10 = 7

## 4-level: (AND) AND (OR) AND NOT
### assumptions
* foo AND priority:high AND -bar AND -scheduled → 2 tasks (task1, task8)
* foo AND priority:high → task1, task3, task8
* -bar removes task3 → task1, task8
* -scheduled removes task8 → task1

## 5-level: nested OR inside AND inside NOT
### assumptions
* foo AND (priority:high OR priority:medium) AND -bar → 2 tasks (task1, task4, task8)
* foo: task1, task3, task4, task8, task9
* (high OR medium): task1, task3, task4, task8
* -bar: task1, task4, task8

## 6-level: double NOT
### assumptions
* -foo AND -bar AND -priority:high → 4 tasks (task2, task6, task7, task12)
* -foo removes task1,3,4,8,9,11
* -bar removes task2,3,5,10,12
* -priority:high removes task1,3,5,8,10,11
* combined: task6, task7, task12

## 7-level: complex mixed
### assumptions
* (foo OR bar) AND priority:high AND -scheduled AND -state:DONE → 2 tasks (task1, task5)
* foo OR bar: task1,2,3,4,5,10,11,12
* priority:high: task1,3,5,8,10,11
* -scheduled: removes task8,10
* -state:DONE: removes task11
* result: task1, task5