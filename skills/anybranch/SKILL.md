---
name: snapshotdb
description: Do all database work on a disposable snapshotdb branch cloned from the production replica. Use when a task touches a database: migrations, backfills, data investigations, tests, or anything an agent might get wrong.
---

# snapshotdb: work on a branch, never on the main database

`snapshotdb` sends commands to a deployed server configured with `SNAPSHOTDB_SERVER` and
`SNAPSHOTDB_TOKEN`. All database files and engines remain on that server; without a server,
the CLI fails. Only deployment operators run `snapshotdb serve`.

It gives you an isolated, writable copy of the production replica on the server.
Writes on a branch never reach production or other branches. A branch URL carries no
production credentials, so it is safe to use freely.

## Rules

1. Never connect to the main/production connection string. Create a branch and use its URL.
2. Wrap URLs in single quotes in shell commands; passwords contain shell characters.
3. If `--print-url` prints nothing, stop. Do not fall back to another database.
4. Delete the branch when the task is done unless the user wants to inspect it.
5. Never write a branch URL into a committed file.

## Commands

```sh
snapshotdb list                                   # what exists; * marks the current branch; passwords redacted
snapshotdb create <task-name> --from prod --print-url   # new branch, becomes current; prints only the URL
snapshotdb info --print-url                       # full URL of the current branch (use this on every step)
snapshotdb reset <task-name>                      # throw away changes, re-clone from prod
snapshotdb rm <task-name>                         # delete; prints nothing on success
```

`prod` is the name of the synced root in most setups; run `snapshotdb list` to see the roots
(they have `-` in the parent column). Use `--format json` on `list`, `info`, or `create` when
you need to parse output. Re-running `create` with the same name is safe and returns the
same branch.

## Recipe

Shell state rarely survives between an agent's steps, so re-read the URL each time instead
of exporting it:

```sh
snapshotdb create fix-orders-index --from prod --print-url >/dev/null || exit 1
psql "$(snapshotdb info --print-url)" -v ON_ERROR_STOP=1 <<'SQL'
begin;
-- migration / backfill here; a failed step leaves the branch clean, or run `snapshotdb reset`
commit;
SQL
snapshotdb rm fix-orders-index
```

Do not write the URL to a file; ask `snapshotdb info --print-url` again instead.

Branch names: 1-40 letters, digits, `-`, `_`, or `.`, not starting with `.` or `_`.

## What a branch contains

Everything the replica had at the moment you branched: schema, data, functions, triggers,
grants. Sequences are moved past the copied rows so inserts work. Changes on production after
you branched do not appear; `reset` gives you a fresh copy.

Idle branches suspend after 5 minutes and resume on the next connection, so a first query
after a pause takes a moment longer.
