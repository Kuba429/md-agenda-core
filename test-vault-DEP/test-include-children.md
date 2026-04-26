* filter task by tag `test-include-children`
* #TODO root task #test-include-children
    * proxy bullet
        * all tasks below proxy bullet should be matched too
        * proxy bullet shouldn't stop the ancestry line of tasks
        * #TODO child task #test-include-children
            * #TODO nested child task with tag #test-include-children
            * #TODO nested child task without tag 
        
* all subtasks are children of the root task 
    * so all children should only appear once in the result
    * the program should return just 1 task - the root task.
        * all other tasks in this file should just be its children
