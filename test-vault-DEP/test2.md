* LINE 1 this line is not a child of a task so it should not be included
    * LINE 2 neither should this one
        * LINE 3 neither should this one
    * LINE 4 should be included [[test2-target]] because it has content other than the link
        * LINE 6 should be included - because its a child of the link
            * LINE 7 should be included - because its descendant of the child
        * LINE 8 should be included
    * LINE 9 should NOT be included
        * LINE 10 should also NOT be included
