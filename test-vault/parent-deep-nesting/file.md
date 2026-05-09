* #TODO level1
  * #TODO level2
    * #TODO level3
      * #TODO level4
        * #TODO level5

# Test Case: parent-deep-nesting
### assumptions
* level1 at line 1, level2 at line 2, level3 at line 3, level4 at line 4, level5 at line 5
* query --parent file.md:1 should return level2 as child (not grandchildren)
* query --parent file.md:2 should return level3 as child
* query --parent file.md:3 should return level4 as child
* query --parent file.md:4 should return level5 as child
* each level returns only direct children, not all descendants