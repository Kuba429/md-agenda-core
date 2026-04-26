* #TODO parent
  * #TODO child1
  * #DONE child2
* #TODO other

# Test Case: include-children basic
### assumptions
* parent has 2 children (child1 TODO, child2 DONE)
* When parent is the only matched task + include-children, result should be 1 task (parent) with 2 children
* Children should have body and children resolved (they're partial tasks from the tree)
* child2 is DONE but still included as child