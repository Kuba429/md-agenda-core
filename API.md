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
| `--group <PROPERTY>` | Group by property (`scheduled`, `priority`, `tag`, or custom) |
| `--task-id <ID>` | Get single task by ID |
| `--next-state <STATE>` | Next state in cycle |
| `--previous-state <STATE>` | Previous state in cycle |
| `--parent-of <ID>` | Parent task of given ID |
| `--tag <TAG>` | Filter by tag |
| `--property <KEY[=VALUE]>` | Filter by property (optionally with value) |
| `--content <TEXT>` | Filter by content (case-insensitive substring match) |
| `--state <STATE>...` | Include only these states |
| `--exclude-state <STATE>...` | Exclude these states |
| `--sort <CRITERIA>...` | Sort by criteria (`state`, `scheduled`, `priority`, etc.) |
| `--include-children` | Include all children of matched tasks |
| `--include-ancestors` | Build ancestor path from matched task to root |

**`--include-children`** — Each matched task gets its full subtree of children. Duplicate roots are deduped.

**`--include-ancestors`** — Each matched task is wrapped in its ancestor chain up to the root (only the direct path, no siblings). If multiple tasks share a root, trees are merged.

**Both flags** — Children are collected first, then ancestors. Merged at the root level to eliminate duplicates.

```bash
agenda-core get                                    # all tasks
agenda-core get --group scheduled                  # grouped by date
agenda-core get --tag bug                          # filtered by tag
agenda-core get --property scheduled=2024-01-15    # filtered by property
agenda-core get --content "meeting"                # filtered by content
agenda-core get --exclude-state DONE               # exclude done tasks
agenda-core get --state TODO --tag urgent           # combine filters
agenda-core get --content "review" --tag urgent       # combine filters
agenda-core get --tag bug --include-children       # bug tasks with full subtrees
agenda-core get --tag bug --include-ancestors       # bug tasks with ancestor paths
agenda-core get --tag bug --include-children --include-ancestors  # both
```

## `add` — Add a task

```bash
agenda-core add --content <TEXT> [--task-id <PARENT_ID>]
```

```bash
agenda-core add --content "Buy groceries"
agenda-core add --content "Buy milk" --task-id "agenda.md:5"  # as subtask
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
agenda-core set --id <ID> [--content <TEXT>] [--state <STATE>] [--property <KEY=VALUE>...] [--tag <TAG>...]
```

| Flag | Description |
|------|-------------|
| `--content <TEXT>` | Replace task content |
| `--state <STATE>` | Replace state tag |
| `--property <KEY=VALUE>` | Add/update property (merge with existing) |
| `--tag <TAG>` | Replace all non-state tags |
| `--remove-tags` | Remove all tags from task |
| `--remove-properties` | Remove all properties from task |

```bash
# change content only
agenda-core set --id "agenda.md:5" --content "Updated text"

# change state only
agenda-core set --id "agenda.md:5" --state "DONE"

# add/update properties
agenda-core set --id "agenda.md:5" --property priority=1 --property scheduled=2024-01-15

# replace all tags
agenda-core set --id "agenda.md:5" --tag urgent --tag release

# remove all tags (keep content, state, properties)
agenda-core set --id "agenda.md:5" --remove-tags

# remove all properties
agenda-core set --id "agenda.md:5" --remove-properties

# remove both tags and properties
agenda-core set --id "agenda.md:5" --remove-tags --remove-properties

# combine fields
agenda-core set --id "agenda.md:5" --content "Done!" --state "DONE" --tag win --property "finished(2024-01-15)"
```

If both `--line` and any field args are provided, returns `{"code": 400, "error": "Cannot use --line with field arguments..."}`.

## `capture` — Add task to file or as subtask

`--target` is optional. If not provided, defaults to `capture.md`. Can be a filename (appends to file) or a task ID (adds as subtask).

```bash
agenda-core capture --content <TEXT> [--target <TARGET>] [--state <STATE>] [--property <PROP>...] [--tag <TAG>...]
```

```bash
agenda-core capture --content "New task"                           # to capture.md (default)
agenda-core capture --content "New task" --target "agenda.md"      # append to file
agenda-core capture --content "Bug" --target "agenda.md" --state "TODO" --tag "bug" --property "priority(high)"
agenda-core capture --content "Buy milk" --target "agenda.md:5"      # as subtask of task at line 5
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

Task object:

```json
{
  "id": "agenda.md:5",
  "content": "Buy groceries",
  "state": "TODO",
  "tags": ["bug"],
  "properties": {"scheduled": "2024-01-15"},
  "body": "- Buy groceries #TODO @scheduled(2024-01-15) #bug",
  "parent": null,
  "children": [],
  "nest_level": 0
}
```

Mutations return:

- `set --line`: `{"code": 200, "newLine": "..."}`
- `set --content/--state/--property/--tag`: `{"code": 200, "task": {...}}`
- others: `{"code": 200, "message": "..."}` on success or `{"code": 400, "error": "..."}` on failure.