# md-agenda-core

A Rust library for managing task lists in Markdown files. Tasks are defined using hashtags for state (e.g., `#TODO`, `#DONE`) and property annotations (e.g., `@scheduled(2024-01-15)`).

## Features

- **Task State Management**: Track tasks with states like TODO, IN_PROGRESS, DONE, NEXT, WAIT, LATER
- **Property Annotations**: Add metadata like `@scheduled(date)`, `@priority(1)`
- **Tag Support**: Tag tasks with arbitrary tags
- **Hierarchy Support**: Tasks can have parent-child relationships
- **Backlinks Support**: Automatically excludes tasks in `<!-- BACKLINKS:START -->...<!-- BACKLINKS:END -->` sections

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
agenda-core = "0.1.0"
```

## Environment Variables

Set the vault directory before using the API:

```rust
std::env::set_var("AGENDA_VAULT_DIR", "/path/to/your/vault");
```

## API Overview

### Task States

```rust
use agenda_core::grep::{TASK_STATES, task_state_next, task_state_prev};

// Available states: TODO, IN_PROGRESS, DONE, NEXT, WAIT, LATER
assert_eq!(task_state_next("TODO"), "IN_PROGRESS");
assert_eq!(task_state_prev("TODO"), "LATER");
```

### Grep Functions

#### `tasks_grep`

Searches for all tasks with any state tag.

```rust
use agenda_core::grep::tasks_grep;
use std::collections::HashSet;

// Get all tasks except DONE and CANCELLED
let mut exclude = HashSet::new();
exclude.insert("DONE".to_string());
exclude.insert("CANCELLED".to_string());

let result = tasks_grep(Some(&exclude)).unwrap();
println!("{}", result);
// Output: file.md:3:- Task #TODO @scheduled(2024-01-15)
//          file.md:5:- Another task #IN_PROGRESS
```

#### `tasks_grep_property`

Searches for tasks with a specific property.

```rust
use agenda_core::grep::tasks_grep_property;

// Get all tasks with @scheduled property
let result = tasks_grep_property("scheduled").unwrap();
// Output: file.md:3:- Task #TODO @scheduled(2024-01-15)
```

### Task Management

#### `tasks_get`

Retrieves all tasks as structured `Task` objects.

```rust
use agenda_core::task::tasks_get;

let tasks = tasks_get();
// Returns Vec<Task> with full task data
```

#### `Task` struct

```rust
pub struct Task {
    pub content: String,      // Task text without tags/properties
    pub state: String,        // TODO, IN_PROGRESS, etc.
    pub nest_level: u8,       // Indentation level
    pub tags: Vec<String>,    // Custom tags
    pub body: String,         // Raw line content
    pub id: String,           // "filename:line" identifier
    pub parent: Option<String>, // Parent task ID
    pub children: Vec<Task>,  // Child tasks
    pub properties: IndexMap<String, String>, // @property(value) pairs
}
```

#### `tasks_ids_get`

Get just task IDs (filename:line pairs).

```rust
use agenda_core::task::tasks_ids_get;

let ids = tasks_ids_get();
// Returns HashSet<String> like {"file.md:3", "file.md:5"}
```

### Grouping Functions

#### `tasks_group_by_filename`

Group task IDs by their source file.

```rust
use agenda_core::task::{tasks_ids_get, tasks_group_by_filename};

let ids = tasks_ids_get();
let grouped = tasks_group_by_filename(&ids);
// Returns IndexMap<String, Vec<&str>>: {"file.md" -> ["file.md:3", "file.md:5"]}
```

#### `tasks_group_date`

Group tasks by their `@scheduled` date (date only, not time).

```rust
use agenda_core::task::{tasks_get, tasks_group_date};

let tasks = tasks_get();
let by_date = tasks_group_date(&tasks);
// Returns IndexMap<String, Vec<&Task>>: {"2024-01-15" -> [task1, task2]}
```

#### `tasks_group_tag`

Group tasks by their custom tags.

```rust
use agenda_core::task::{tasks_get, tasks_group_tag};

let tasks = tasks_get();
let by_tag = tasks_group_tag(&tasks);
// Returns IndexMap<String, Vec<&Task>>: {"bug" -> [task1], "feature" -> [task2]}
// Tasks without tags are grouped under "NO TAG"
```

#### `tasks_group_by_property`

Group tasks by any property dynamically.

```rust
use agenda_core::task::{tasks_get, tasks_group_by_property};

let tasks = tasks_get();
let by_priority = tasks_group_by_property(&tasks, "priority");
// Returns IndexMap<String, Vec<&Task>>: {"1" -> [task1], "2" -> [task2]}
// Only includes tasks that have the specified property
```

#### `tasks_group_by_date_and_tag`

Single-pass grouping by both date and tag.

```rust
use agenda_core::task::{tasks_get, tasks_group_by_date_and_tag};

let tasks = tasks_get();
let (by_date, by_tag) = tasks_group_by_date_and_tag(&tasks);
// Returns (IndexMap, IndexMap)
```

### Task Modification

#### `task_add`

Add a new task.

```rust
use agenda_core::task::task_add;

// Add top-level task
task_add("Buy groceries", None);

// Add subtask to parent
task_add("Buy milk", Some("file.md:3"));
```

#### `task_change_state`

Change task state.

```rust
use agenda_core::task::task_change_state;

task_change_state("file.md:3", "DONE").unwrap();
```

#### `task_change_property`

Set, update, or remove a property.

```rust
use agenda_core::task::task_change_property;

// Set property
task_change_property("file.md:3", "scheduled", Some("2024-01-15")).unwrap();

// Remove property
task_change_property("file.md:3", "priority", None).unwrap();
```

### Utility Functions

#### `tasks_sort`

Sort tasks by scheduled date then priority.

```rust
use agenda_core::task::{tasks_get, tasks_sort};

let mut tasks = tasks_get();
tasks_sort(&mut tasks);
```

#### `flatten_and_collect`

Flatten task tree and get all IDs.

```rust
use agenda_core::task::{tasks_get, flatten_and_collect};

let tasks = tasks_get();
let (flat_tasks, child_ids) = flatten_and_collect(&tasks);
```

#### `task_parent_get`

Get parent task of a child task.

```rust
use agenda_core::task::task_parent_get;

let parent = task_parent_get("file.md:5".to_string());
// Returns Option<Task>
```

## Task Syntax

### States
```
#TODO, #IN_PROGRESS, #DONE, #NEXT, #WAIT, #LATER
```

### Properties
```
@property(value)
Examples: @scheduled(2024-01-15), @priority(1), @due(2024-02-01)
```

### Tags
```
#tagname
Examples: #bug, #feature, #high-priority
```

### Task Lines
```
- Task text #TODO @scheduled(2024-01-15) #bug
  - Child task #TODO @priority(1)
```

### Backlinks Section
Tasks between `<!-- BACKLINKS:START -->` and `<!-- BACKLINKS:END -->` are automatically excluded from grep results.

## Running Tests

```bash
# Set vault directory and run tests
AGENDA_VAULT_DIR=test-vault cargo test --lib -- --test-threads=1
```