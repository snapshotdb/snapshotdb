---
name: anybranch
description: Do all database work on a disposable anybranch branch cloned from the production replica. Use when a task touches a database: migrations, backfills, data investigations, tests, or anything an agent might get wrong.
---

# anybranch: work on a branch, never on the main database

`anybranch` gives you an isolated, writable copy of the production database in well under a
second. Writes on a branch never reach production or other branches. A branch URL carries no
production credentials, so it is safe to use freely.

## Rules

1. Never connect to the main/production connection string. Create a branch and use its URL.
2. Wrap URLs in single quotes in shell commands; passwords contain shell characters.
3. If `--print-url` prints nothing, stop. Do not fall back to another database.
4. Delete the branch when the task is done unless the user wants to inspect it.
5. Never write a branch URL into a committed file.

## Commands

```sh
anybranch list                                   # what exists; * marks the current branch
anybranch create <task-name> --from prod --print-url   # new branch; prints only the URL
anybranch info --print-url                       # URL of the current branch
anybranch reset <task-name>                      # throw away changes, re-clone from prod
anybranch rm <task-name>                         # delete
```

`prod` is the name of the synced root in most setups; run `anybranch list` to see the roots
(they have `-` in the parent column). Use `--format json` on `list`, `info`, or `create` when
you need to parse output.

## Recipe

```sh
DATABASE_URL="$(anybranch create fix-orders-index --from prod --print-url)"
[ -n "$DATABASE_URL" ] || { echo "branch creation failed" >&2; exit 1; }
export DATABASE_URL
# ... run migrations, tests, queries against $DATABASE_URL ...
anybranch rm fix-orders-index
```

Branch names: 1-40 characters, no `/`, not starting with `.` or `_`.

## What a branch contains

Everything the replica had at the moment you branched: schema, data, functions, triggers,
grants. Sequences are moved past the copied rows so inserts work. Changes on production after
you branched do not appear; `reset` gives you a fresh copy.

Idle branches suspend after 5 minutes and resume on the next connection, so a first query
after a pause takes a moment longer.
