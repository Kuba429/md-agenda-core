# Agent Workflow Guidelines

## CLI Usage

* AGENVA_VAULT_DIR environment variable must be an absolute path
* Use full path when running CLI commands, e.g. `AGENDA_VAULT_DIR=/full/path cargo run -- ...`

---

## Feature Workflow

When add new feature:

1. Add test in test-vault

   * Make test files for new behavior
   * Put files in different folder levels if needed

2. Add unit/integration tests

   * Write tests in `src/backlinks.rs`
   * Use `#[cfg(test)]`
   * Tests must check new behavior

3. Start code

   * Run tests first → see fail
   * Write code step by step
   * Stop when tests pass

4. Commit

   * ALWAYS commit after feature or tests
   * Do not ask user
   * Just commit
   * Commit message format: `verb: namespace - short description`
     * Verbs: `added:`, `fixed:`, `removed:`, `changed:`
     * Namespace: symbolic module area like `cli`, `tasks`, `grep`, `backlinks`, `tests`
     * Examples: `changed: tasks - property syntax`, `fixed: grep - case sensitivity bug`, `added: cli - include children flag`

## Important: ALWAYS commit after completing a task

* Never leave uncommitted changes
* Commit after each feature, bug fix, or test addition
* If you forget, the user will remind you (annoyedly)

## Why

* Feature works with real files
* Catch edge cases early
* Tests show how code works
* Easy to undo changes


## Test vault
* use projectRoot/test-vault for testing purposes.
* never run the cli without explicitly providing this vault as AGENDA_VAULT_DIR
* NEVER use user's personal vault for testing - ONLY test-vault
* NEVER read or access files outside test-vault

## Integration tests

Each scenario is a subfolder in test-vault (e.g., `test-vault/basic/`).

Each scenario folder contains `.md` file(s) with a `## Test Case` header describing what to test and expected result.

When user asks to add a test:
1. Read the scenario folder to find the test case description
2. Write a test function in `src/integration_tests.rs` that:
   - Uses `vault_path("scenario-name")` to point to the folder
   - Runs the appropriate function (e.g., `tasks_grep`, `tasks_grep_property`)
   - Asserts the expected result based on the test case description
3. Run the test to verify it passes

Each test case = one test function. Don't add extra tests.

## What is a task

* A task is a bullet line (-, *, +) that has a state tag
* State tags: #TODO, #IN_PROGRESS, #DONE, #NEXT, #WAIT, #LATER
* A bullet with only non-state tags (e.g., #foo, #bug) is NOT a task
* A non-bullet line with a state tag is NOT a task
