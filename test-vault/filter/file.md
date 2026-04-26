* #TODO task1
* #NEXT task2
* #TODO task3 @scheduled(2025-04-20)
* #TODO task4 #foo
    * #TODO task5 #foo #bar

# Test cases
## query tasks with state 'NEXT'
### assumptions
* only 1 task gets returned - 'task2'


## query tasks with state other than 'NEXT'
### assumptions
* only 3 tasks get returned - 'task1', 'task3' and 'task4'

## query tasks with property 'scheduled'
### assumptions
* only 1 task gets returned - 'task3'

## query tasks with property 'scheduled' equal to '2025-04-20'
### assumptions
* only 1 task gets returned - 'task3'

## query tasks with property 'scheduled' equal to '2025-04-21'
### assumptions
* no tasks should be returned

## query tasks with tag 'foo'
### assumptions
* exactly 2 tasks should be returned - 'task4' and 'task5'

## query tasks with tag 'bar'
### assumptions
* exactly 1 task should be returned - 'task5'
