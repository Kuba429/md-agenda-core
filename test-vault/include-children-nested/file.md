* #TODO grandparent
  * #TODO parent
    * #TODO child
* #TODO other

# Test Case: include-children nested
### assumptions
* grandparent has 1 child (parent), which has 1 child (child)
* When include-children is on grandparent, result should be 1 task (grandparent) with parent as child
* parent should have child as its own child (nested hierarchy preserved)