* #TODO parent
  * #TODO child_bug #bug
  * #TODO child_urgent #urgent
  * #DONE child_done
  * #TODO child_no_tag
* #TODO other

# Test Case: parent with children with different tags/states
### assumptions
* parent is at line 1
* `--parent file.md:1` should return 4 children
* `--parent file.md:1 --tag bug` should return only child_bug
* `--parent file.md:1 --state DONE` should return only child_done
* `--parent file.md:1 --state TODO` should return 3 TODO children: child_bug, child_urgent, child_no_tag