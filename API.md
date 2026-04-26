# md-agenda-core CLI

Manage tasks in markdown files. All output is JSON. Set `AGENDA_VAULT_DIR` to your vault path.

## Task IDs

Format: `<filename>:<line>` (e.g., `agenda.md:5`, `notes.md:10`)

## States

TODO → IN_PROGRESS → DONE → NEXT → WAIT → LATER → (cycles back)

## `get` — Query tasks

```bash
agenda-core get [FLAGS]
```

| Flag | Description |
|------|-------------|
| `--task-id <ID>` | Get single task by ID (with full child tree and body) |
| `--next-state <STATE>` | Next state in cycle |
| `--previous-state <STATE>` | Previous state in cycle |
| `--parent-of <ID>` | Parent task of given ID |
| `--parent <ID>` | Get direct children of a task (returns partial tasks, no grandchildren) |
| `--tag <TAG>` | Filter by tag |
| `--property <KEY[=VALUE]>` | Filter by property (optionally with value) |
| `--title <TEXT>` | Filter by title (case-insensitive substring match) |
| `--state <STATE>...` | Include only these states |
| `--exclude-state <STATE>...` | Exclude these states |
| `--sort <CRITERIA>...` | Sort by criteria (`state`, `scheduled`, `priority`, etc.) |
| `--include-children` | Include all children of matched tasks (full recursive subtree, deduplicated) |
| `--include-ancestors` | Build ancestor path from matched task to root (currently no-op) |
| `--query <QUERY>` | Complex filter expression (see below) |
| `--query-debug <QUERY>` | Debug query: returns AST, strategy, and regex as JSON |

```bash
agenda-core get                                    # all tasks
agenda-core get --tag bug                          # filtered by tag
agenda-core get --property scheduled=2024-01-15    # filtered by property
agenda-core get --title "meeting"                  # filtered by title
agenda-core get --exclude-state DONE               # exclude done tasks
agenda-core get --state TODO --tag urgent          # combine filters
agenda-core get --title "review" --tag urgent      # combine filters
agenda-core get --tag bug --include-children       # bug tasks with full subtrees
agenda-core get --tag bug --include-ancestors      # bug tasks (ancestors currently no-op)
agenda-core get --query "tag:foo AND property:priority=high"  # complex query
agenda-core get --parent "agenda.md:5"                        # direct children of task
agenda-core get --parent "agenda.md:5" --tag bug             # children with tag bug
```

### `--query` DSL Syntax

The `--query` flag accepts composable filter expressions with `AND`, `OR`, and `NOT` (`-` prefix):

| Expression | Description |
|-----------|-------------|
| `tag:foo` | Has tag "foo" |
| `-tag:foo` | Does NOT have tag "foo" |
| `property:key` | Has property "key" |
| `property:key=val` | Property "key" equals "val" |
| `-property:key=val` | Property "key" does NOT equal "val" |
| `title:"text"` | Title contains "text" (case-insensitive) |
| `title:text` | Title contains "text" |
| `state:TODO` | State is TODO |
| `-state:DONE` | State is NOT DONE |

Use `AND`/`OR` and parentheses `()` for composition:

```bash
agenda-core get --query "tag:foo AND property:priority=high"
agenda-core get --query "tag:foo OR tag:bar"
agenda-core get --query "(tag:foo OR tag:bar) AND -state:DONE"
agenda-core get --query "tag:foo AND property:bar=baz AND -tag:foo_bar"
```

When `--query` is used, tasks are loaded without default state exclusions (DONE/CANCELLED are included). Combine with existing flags for additional AND filtering.

## `add` — Add a task

```bash
agenda-core add --title <TEXT> [--task-id <PARENT_ID>]
```

```bash
agenda-core add --title "Buy groceries"
agenda-core add --title "Buy milk" --task-id "agenda.md:5"  # as subtask
```

## `change` — Modify a task

```bash
agenda-core change --task-id <ID> [--state <STATE>] [--property <KEY> [VALUE]]
```

```bash
agenda-core change --task-id "agenda.md:5" --state "DONE"
agenda-core change --task-id "agenda.md:5" --property "scheduled" "2024-01-15"
agenda-core change --task-id "agenda.md:5" --property "priority"  # removes property
```

## `set` — Modify task line

Two modes: **raw line** (replace entire line) or **field-based** (modify individual fields). They are mutually exclusive.

### Raw line mode

```bash
agenda-core set --id <ID> --line <LINE>
```

```bash
agenda-core set --id "agenda.md:5" --line "- New content #TODO @scheduled(2024-01-15)"
```

### Field-based mode

```bash
agenda-core set --id <ID> [--title <TEXT>] [--state <STATE>] [--property <KEY=VALUE>...] [--tag <TAG>...]
```

| Flag | Description |
|------|-------------|
| `--title <TEXT>` | Replace task title |
| `--state <STATE>` | Replace state tag |
| `--property <KEY=VALUE>` | Add/update property (merge with existing) |
| `--tag <TAG>` | Replace all non-state tags |
| `--remove-tags` | Remove all tags from task |
| `--remove-properties` | Remove all properties from task |

```bash
# change title only
agenda-core set --id "agenda.md:5" --title "Updated text"

# change state only
agenda-core set --id "agenda.md:5" --state "DONE"

# add/update properties
agenda-core set --id "agenda.md:5" --property priority=1 --property scheduled=2024-01-15

# replace all tags
agenda-core set --id "agenda.md:5" --tag urgent --tag release

# remove all tags (keep title, state, properties)
agenda-core set --id "agenda.md:5" --remove-tags

# remove all properties
agenda-core set --id "agenda.md:5" --remove-properties

# remove both tags and properties
agenda-core set --id "agenda.md:5" --remove-tags --remove-properties

# combine fields
agenda-core set --id "agenda.md:5" --title "Done!" --state "DONE" --tag win --property "finished(2024-01-15)"
```

If both `--line` and any field args are provided, returns `{"code": 400, "error": "Cannot use --line with field arguments..."}`.

## `capture` — Add task to file or as subtask

`--target` is optional. If not provided, defaults to `capture.md`. Can be a filename (appends to file) or a task ID (adds as subtask).

```bash
agenda-core capture --title <TEXT> [--target <TARGET>] [--state <STATE>] [--property <PROP>...] [--tag <TAG>...]
```

```bash
agenda-core capture --title "New task"                           # to capture.md (default)
agenda-core capture --title "New task" --target "agenda.md"      # append to file
agenda-core capture --title "Bug" --target "agenda.md" --state "TODO" --tag "bug" --property "priority(high)"
agenda-core capture --title "Buy milk" --target "agenda.md:5"    # as subtask of task at line 5
```

## `backlinks` — Generate backlinks

```bash
agenda-core backlinks [TARGET]
```

```bash
agenda-core backlinks              # all files
agenda-core backlinks "target.md"  # specific file
```

## Output format

Task object (in list/filtered output — no children, no body):

```json
{
  "id": "agenda.md:5",
  "title": "Buy groceries",
  "raw": "- Buy groceries #TODO @scheduled(2024-01-15) #bug",
  "state": "TODO",
  "tags": ["bug"],
  "properties": {"scheduled": "2024-01-15"},
  "body": "",
  "parent": null,
  "children": []
}
```

Task object (via `--task-id` — with full child tree and body):

```json
{
  "id": "agenda.md:5",
  "title": "Buy groceries",
  "raw": "- Buy groceries #TODO @scheduled(2024-01-15) #bug",
  "state": "TODO",
  "tags": ["bug"],
  "properties": {"scheduled": "2024-01-15"},
  "body": "- Subtask #DONE\nSome note text",
  "parent": null,
  "children": [
    {
      "id": "agenda.md:6",
      "title": "Subtask",
      "raw": "  - Subtask #DONE",
      "state": "DONE",
      "tags": [],
      "properties": {},
      "body": "",
      "parent": "agenda.md:5",
      "children": []
    }
  ]
}
```

Mutations return:

- `set --line`: `{"code": 200, "newLine": "..."}`
- `set --title/--state/--property/--tag`: `{"code": 200, "message": "task updated", "task": {...}}`
- others: `{"code": 200, "message": "..."}` on success or `{"code": 400, "error": "..."}` on failure.

## Recent Changes

### Parsing refactor

- **`content` field renamed to `title`**: The `content` field has been renamed to `title` in both the Task JSON output and CLI flags (`--content` → `--title`). The query DSL also changed: `content:` → `title:`.
- **`--title` filter replaces `--content`**: The `--content` CLI flag is now `--title`.
- **`body` field change**: `body` now contains the multiline text below the task line (only populated in `--task-id` output). Indentation is relative to the task (minimum body indent is stripped so direct children have 0 indent). In list output, `body` is an empty string.
- **New `raw` field**: Contains the original task line verbatim (e.g., `- Buy groceries #TODO @scheduled(2024-01-15) #bug`).
- **`nest_level` removed**: This dead field has been removed entirely.
- **`--group` removed**: Grouping is no longer server-side. Clients handle grouping themselves.
- **`--include-children` reimplemented**: Uses new body-based parser (`resolve_task_tree`). Full recursive subtree, deduplicated (child tasks removed from root level). Children not affected by filters (all children included regardless of state/tags).
- **`--include-ancestors` temporarily no-op**: To be reimplemented.
- **`--parent-of` temporarily returns null**: Will be reimplemented.
- **`--task-id` resolves full recursive subtree**: Using `--task-id` now returns the complete child tree via the new body-based parser.
- **New `--parent <ID>` flag**: Returns direct children of a task (partial parse, no grandchildren). Supports further filtering via `--tag`, `--state`, `--property`, `--title`, and `--query`.
- **`parent` field populated in lists**: Bulk list output now sets `parent` by scanning backward for less-indented task bullets.
- **Tab handling**: Tabs are treated as 2 spaces for indentation (configurable in the future).
