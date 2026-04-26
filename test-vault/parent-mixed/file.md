* #TODO parent
  * #TODO child1
  Some note text
  > A quote
  * #DONE child2
  - non-task bullet
* #TODO other

# Test Case: parent with children mixed with non-task lines
### assumptions
* parent is at line 1
* `--parent file.md:1` should return 2 children: child1 (line 2) and child2 (line 5)
* non-bullet lines (note, quote) and non-state bullets should NOT be returned as children