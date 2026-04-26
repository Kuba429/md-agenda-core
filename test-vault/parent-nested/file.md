* #TODO grandparent
  * #TODO parent
    * #TODO child1
    * #TODO child2
  * #DONE parent_sibling

# Test Case: parent is itself a child of another task
### assumptions
* parent is at line 2
* `--parent file.md:2` should return 2 children: child1 (line 3) and child2 (line 4)
* `--parent file.md:2` should NOT return grandparent, parent_sibling, or grandchildren