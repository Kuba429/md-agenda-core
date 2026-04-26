* #TODO parent #bug
  * #TODO child1
  * #DONE child2
* #TODO other #bug
  * #TODO child3
* #TODO unrelated

# Test Case: include-children filtered
### assumptions
* When --tag bug is combined with --include-children, only parent and other match
* parent gets child1 and child2 as children (child2 is DONE but still included)
* other gets child3 as child
* child1, child2, child3 are not at root level (deduped)
* unrelated task is not matched (no #bug tag)