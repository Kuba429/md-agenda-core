* #TODO parent_task
  * #TODO child1
  * #DONE child2
* #TODO other_task

# Test Case: parent with direct children
### assumptions
* parent is at line 1
* `--parent file.md:1` should return 2 children: child1 (line 2, TODO) and child2 (line 3, DONE)
* `--parent file.md:1` should NOT return parent_task or other_task
* `--parent file.md:1 --state DONE` should return only child2