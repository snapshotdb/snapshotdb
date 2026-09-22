#!/usr/bin/env bash
# End-to-end check: a fake "production" made by snapshotdb itself, preflight, a synced
# replica, schema changes, a poisoned transaction repaired, detached branches, suspend and
# resume through the proxy, settings, and a clean teardown. Needs the engine binaries on
# PATH; engines that are not installed are skipped. Uses a throwaway SNAPSHOTDB_HOME unless
# one is exported (e.g. a Btrfs mount on Linux).
set -u
cd "$(dirname "$0")"
cargo build --release -q || exit 1
B=$PWD/target/release/snapshotdb
export SNAPSHOTDB_HOME=${SNAPSHOTDB_HOME:-$(mktemp -d /tmp/snapshotdb-e2e.XXXX)}
mkdir -p "$SNAPSHOTDB_HOME" || exit 1
if [ -n "$(find "$SNAPSHOTDB_HOME" -mindepth 1 -maxdepth 1 -print -quit)" ]; then
  echo 'E2E storage must be empty; use a disposable directory' >&2
  exit 1
fi
chmod 700 "$SNAPSHOTDB_HOME" || exit 1
export SNAPSHOTDB_IDLE_MINUTES=0
export SNAPSHOTDB_TOKEN=$(openssl rand -hex 32)
api_port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
export SNAPSHOTDB_SERVER="http://127.0.0.1:$api_port"
start_server() {
  $B serve --bind "127.0.0.1:$api_port" --public-host 127.0.0.1 >"$SNAPSHOTDB_HOME/server.log" 2>&1 &
  server_pid=$!
  for _ in $(seq 1 50); do
    curl -fsS -H "Authorization: Bearer $SNAPSHOTDB_TOKEN" "$SNAPSHOTDB_SERVER/v1/health" >/dev/null 2>&1 && return 0
    sleep 0.1
  done
  cat "$SNAPSHOTDB_HOME/server.log"; return 1
}
start_server || exit 1
trap 'kill "$server_pid" 2>/dev/null; wait "$server_pid" 2>/dev/null' EXIT
fail=0
check() { if [ "$2" = "$3" ]; then echo "  ok   $1 = $2"; else echo "  FAIL $1: got '$2' want '$3'"; fail=1; fi; }
waitfor() { for _ in $(seq 1 120); do v=$(eval "$1" 2>/dev/null); [ "$v" = "$2" ] && break; sleep 0.3; done; check "$3" "$v" "$2"; }
port() { local u; u=$($B url "$1"); u=${u##*:}; echo "${u%%/*}"; }
st() { $B list | sed 's/^[* ] //' | awk -F'\t' -v n="$1" '$1==n{print $4}'; }
pwof() { local a=${1#*://}; a=${a%%@*}; echo "${a#*:}"; }
refused() { "$@" >/dev/null 2>&1 && echo allowed || echo refused; }

if [ "${E2E_SKIP_POSTGRES:-0}" != 1 ] && command -v pg_ctl >/dev/null; then
  echo "postgres"
  $B import postgres src --new >/dev/null || exit 1
  SRC=$($B url src); [ -n "$SRC" ] || exit 1
  echo "$SRC" | grep -qE '^postgresql://[^:]+:[0-9a-f]{32}@'; check "root has a generated password" "$?" 0
  check "TCP without the password is refused" "$(refused psql "postgresql://$USER@127.0.0.1:$(port src)/postgres" -Atc 'select 1')" refused
  psql "$SRC" -Xq -c "CREATE ROLE snapshotdb_agent SUPERUSER; GRANT pg_read_server_files TO snapshotdb_agent; ALTER ROLE snapshotdb_agent SET search_path='pg_catalog';"
  psql "$SRC" -Xq -c "create table users(id serial primary key, name text); insert into users(name) select 'u'||g from generate_series(1,1000) g; create table nopk(x int);"
  $B preflight postgres "$SRC" >/dev/null 2>&1; check "preflight passes on a good source" "$?" 0
  $B preflight postgres "postgresql://nobody@127.0.0.1:1/x" >/dev/null 2>&1; check "preflight exits 2 on a bad source" "$?" 2
  $B preflight postgres "$SRC" --format json | grep -q '"name":"replica identity","state":"warn"'; check "preflight warns about table without primary key" "$?" 0
  $B clone prod "$SRC" >/dev/null 2>&1 || exit 1
  PROD=$($B url prod); [ -n "$PROD" ] || exit 1
  waitfor "psql '$PROD' -Atc 'select count(*) from users'" 1000 "initial copy"
  $B status prod --format json | grep -q '"state":"syncing"'; check "status json" "$?" 0
  psql "$SRC" -Atqc "insert into users(name) values ('live')"
  waitfor "psql '$PROD' -Atc 'select count(*) from users'" 1001 "live change replicated"
  # schema changes on production, one statement per query as migration tools send them
  psql "$SRC" -Xq <<'SQL'
alter table users add column email text;
update users set email = 'e' where id = 1;
create table orders(id serial primary key, amt int);
insert into orders(amt) values (7);
create index concurrently users_email on users(email);
create role reporter;
grant select on users to reporter;
SQL
  waitfor "psql '$PROD' -Atc \"select email from users where id = 1\"" e "column added on production appears on replica"
  waitfor "$B status prod >/dev/null; psql '$PROD' -Atc 'select count(*) from orders'" 1 "new table joins replication after refresh"
  waitfor "psql '$PROD' -Atc \"select count(*) from pg_indexes where indexname = 'users_email'\"" 1 "CREATE INDEX CONCURRENTLY replayed as a plain index"
  $B status prod | grep -q '1 failed'; check "non-replayable DDL (grant to a role only on source) is recorded, not fatal" "$?" 0
  check "failed DDL refuses new branches" "$(refused $B create bad-schema --from prod)" refused
  psql "$PROD" -Atqc 'CREATE ROLE reporter'
  $B repair prod | grep -q replayed; check "repair retries failed DDL after prerequisites are fixed" "$?" 0
  check "repaired grant is present" "$(psql "$PROD" -Atc "select has_table_privilege('reporter','users','SELECT')")" t
  $B status prod | grep -q 'retaining'; check "status shows WAL retained on source" "$?" 0
  psql "$SRC" -v ON_ERROR_STOP=1 -Atqc "ALTER TABLE users ADD COLUMN mixed text DEFAULT 'literal;CONCURRENTLY'; INSERT INTO users(id,name) VALUES(4900,'mixed-batch');"
  waitfor "psql '$PROD' -Atc \"select count(*) from users where name='mixed-batch'\"" 1 "mixed DDL/DML batch inserts exactly once"
  check "DDL replay preserves quoted semicolons and keywords" "$(psql "$PROD" -Atc "select mixed from users where id=1")" 'literal;CONCURRENTLY'
  psql "$SRC" -v ON_ERROR_STOP=1 -Atq <<'SQL'
/* outer /* nested */ comment */ CREATE FUNCTION quoted_body() RETURNS text LANGUAGE sql AS $body$ SELECT 'kept;inside'::text $body$;
SQL
  waitfor "psql '$PROD' -Atc 'select quoted_body()'" 'kept;inside' "DDL replay preserves dollar-quoted function bodies"
  psql "$SRC" -Atqc "DELETE FROM users WHERE id=4900"
  waitfor "psql '$PROD' -Atc 'select count(*) from users where id=4900'" 0 "mixed fixture cleanup replicated"
  # a poisoned transaction: the replica already has id 5000, production inserts it too
  psql "$PROD" -Atqc "insert into users(id, name) values (5000, 'local')"
  psql "$SRC" -Atqc "insert into users(id, name) values (5000, 'remote')"
  waitfor "$B status prod | grep -c PAUSED" 1 "conflict pauses replication instead of retrying forever"
  $B create incomplete --from prod >/dev/null 2>&1; check "branching a paused replica is refused" "$?" 1
  check "refused branch creates no data directory" "$([ ! -d "$SNAPSHOTDB_HOME/incomplete" ] && echo absent)" absent
  $B repair prod | grep -q skipped; check "repair skips the poisoned transaction" "$?" 0
  psql "$SRC" -Atqc "insert into users(name) values ('after-repair')"
  waitfor "psql '$PROD' -Atc \"select count(*) from users where name = 'after-repair'\"" 1 "rows flow again after repair"
  check "replica has no event trigger" "$(psql "$PROD" -Atc 'select count(*) from pg_event_trigger')" 0
  # Warm the freshness plumbing, then deliberately block apply while committing
  # a source update. Returning the old snapshot (or killing the blocker by
  # stopping the parent too early) must not satisfy create.
  $B create freshness-warm --from prod >/dev/null || exit 1
  $B rm freshness-warm >/dev/null || exit 1
  psql "$PROD" -Xq -v ON_ERROR_STOP=1 -c 'BEGIN; LOCK TABLE users IN ACCESS EXCLUSIVE MODE; SELECT pg_sleep(5); COMMIT' >/dev/null &
  freshness_blocker=$!
  waitfor "psql '$PROD' -Atc \"SELECT count(*) FROM pg_locks WHERE relation='users'::regclass AND mode='AccessExclusiveLock' AND granted\"" 1 "freshness fixture blocks apply"
  psql "$SRC" -Xq -v ON_ERROR_STOP=1 -c "UPDATE users SET name='fresh-at-request' WHERE id=1"
  FRESH=$($B create fresh-after-write --from prod --print-url) || exit 1
  wait "$freshness_blocker"; check "create lets blocked apply finish before stopping parent" "$?" 0
  check "create includes the source commit without an external catch-up wait" "$(psql "$FRESH" -Atc 'select name from users where id=1')" fresh-at-request
  $B rm fresh-after-write >/dev/null || exit 1
  before_freshness_timeout=$(psql "$PROD" -Atc 'select pg_postmaster_start_time()')
  psql "$PROD" -Xq -v ON_ERROR_STOP=1 -c 'BEGIN; LOCK TABLE users IN ACCESS EXCLUSIVE MODE; SELECT pg_sleep(5); COMMIT' >/dev/null &
  freshness_blocker=$!
  waitfor "psql '$PROD' -Atc \"SELECT count(*) FROM pg_locks WHERE relation='users'::regclass AND mode='AccessExclusiveLock' AND granted\"" 1 "freshness timeout fixture blocks apply"
  psql "$SRC" -Xq -v ON_ERROR_STOP=1 -c "UPDATE users SET name='fresh-after-timeout' WHERE id=1"
  printf '%s\n' '["create","fresh-timeout","--from","prod"]' | env SNAPSHOTDB_INTERNAL=1 SNAPSHOTDB_FRESHNESS_TIMEOUT_SECONDS=1 "$B" _worker >"$SNAPSHOTDB_HOME/freshness-timeout.log" 2>&1
  check "freshness timeout refuses a stale branch" "$?" 1
  grep -q 'has not applied the source commit' "$SNAPSHOTDB_HOME/freshness-timeout.log"; check "refusal is a freshness timeout" "$?" 0
  check "freshness timeout leaves no branch directory" "$([ ! -d "$SNAPSHOTDB_HOME/fresh-timeout" ] && echo absent)" absent
  check "freshness timeout does not restart parent" "$(psql "$PROD" -Atc 'select pg_postmaster_start_time()')" "$before_freshness_timeout"
  wait "$freshness_blocker"
  # branches
  $B settings prod set branch_sql "insert into users(name) values ('seeded')"
  $B settings prod set branch_sql "insert into users(name) values ('fixtures')" --hook 20-fixtures
  $B create dev --from prod --print-url >/dev/null || exit 1; DEV=$($B url dev)
  check "create --print-url made dev current" "$($B info --print-url)" "$DEV"
  check "re-running create returns the same URL" "$($B create dev --from prod --print-url)" "$DEV"
  $B create dev >/dev/null 2>&1; check "usage error exits 2" "$?" 2
  $B create dev --from nope --format json | grep -q '"error"'; check "json error output" "$?" 0
  check "branch has its own password" "$([ "$(pwof "$DEV")" != "$(pwof "$PROD")" ] && echo distinct)" distinct
  check "agent is not a superuser" "$(psql "$DEV" -Atc 'select rolsuper from pg_roles where rolname=current_user')" f
  check "agent inherited memberships removed" "$(psql "$DEV" -Atc "select count(*) from pg_auth_members where member=current_user::regrole")" 0
  check "agent cannot read server files" "$(refused psql "$DEV" -v ON_ERROR_STOP=1 -Atc "select pg_read_file('/etc/passwd')")" refused
  check "agent cannot execute host programs" "$(refused psql "$DEV" -v ON_ERROR_STOP=1 -Atc "COPY (SELECT 1) TO PROGRAM 'true'")" refused
  canary="$SNAPSHOTDB_HOME/src/audit-canary"
  printf 'harmless-canary' > "$canary"; chmod 600 "$canary"
  maintenance="postgresql:///postgres?host=$SNAPSHOTDB_HOME/dev/run&port=$(cat "$SNAPSHOTDB_HOME/dev/run/eport")&user=$USER"
  check "sandbox hides sibling files even from maintenance superuser" "$(psql "$maintenance" -Atc "select pg_read_file('$canary',0,100,true) is null")" t
  check "sandbox hides host process environment" "$(psql "$maintenance" -Atc "select pg_read_file('/proc/1/environ',0,100,true) is null")" t
  rm "$canary"
  $B settings prod set branch_sql 'SELECT nonexistent_hook_for_security_test()' >/dev/null
  check "failed hook refuses branch creation" "$(refused $B create failed-hook --from prod --print-url)" refused
  check "failed hook exposes no URL" "$(refused $B url failed-hook)" refused
  check "failed hook branch is removed" "$([ ! -d "$SNAPSHOTDB_HOME/failed-hook" ] && echo absent)" absent
  $B settings prod set branch_sql "insert into users(name) values ('seeded')" >/dev/null
  check "branch has no subscription" "$(psql "$DEV" -Atc 'select count(*) from pg_subscription')" 0
  check "branch_sql hooks ran once on the new branch" "$(psql "$DEV" -Atc "select count(*) from users where name in ('seeded', 'fixtures')")" 2
  check "branch sequence advanced past replicated rows" "$(psql "$DEV" -Atqc "insert into users(name) values ('dev') returning id > 5000")" t
  psql "$SRC" -Atqc "insert into users(name) values ('after-branch')"
  waitfor "psql '$PROD' -Atc \"select count(*) from users where name = 'after-branch'\"" 1 "replica keeps streaming after branch"
  check "branch isolated from later production writes" "$(psql "$DEV" -Atc "select count(*) from users where name = 'after-branch'")" 0
  # suspend and resume through the proxy
  $B stop dev; check "stopped branch shows suspended" "$(st dev)" suspended
  check "connecting to a suspended branch resumes it" "$(psql "$DEV" -Atc 'select 1')" 1
  check "branch running again" "$(st dev)" running
  $B create dev2 --from prod --format json | grep -q '"name":"dev2","engine":"postgres","parent":"prod"'; check "create --format json" "$?" 0
  psql "$PROD" -Atqc 'ALTER SUBSCRIPTION snapshotdb_prod DISABLE'
  $B reset dev >/dev/null 2>&1; check "reset refuses a paused parent" "$?" 1
  check "refused reset preserves existing branch writes" "$(psql "$DEV" -Atc "select count(*) from users where name = 'dev'")" 1
  psql "$PROD" -Atqc 'ALTER SUBSCRIPTION snapshotdb_prod ENABLE'
  $B lock prod; $B rm prod >/dev/null 2>&1; check "locked root refuses rm" "$?" 1
  $B unlock prod
  # Simulate an orderly reboot: stop the subscriber before its fake production source.
  # Stopping the publisher first while the subscriber stays up deliberately creates a
  # replication error (disable_on_error can pause it), which is a different repair test.
  for b in prod dev dev2 src; do kill -TERM "$(cat "$SNAPSHOTDB_HOME/$b/run/proxypid")" 2>/dev/null; $B stop "$b" 2>/dev/null; done; sleep 1
  check "after 'reboot' URLs are dead" "$(st dev)" stopped
  kill "$server_pid"; wait "$server_pid" 2>/dev/null
  start_server || exit 1
  check "up restores the synced root" "$(st prod)" syncing
  check "up restores branch proxies (suspended, resume on connect)" "$(st dev)" suspended
  check "URL unchanged across the reboot" "$($B url dev)" "$DEV"
  check "branch resumes on first connection after reboot" "$(psql "$DEV" -Atc 'select 1')" 1
  $B start src >/dev/null
  psql "$SRC" -Atqc "insert into users(name) values ('after-reboot')"
  waitfor "psql '$PROD' -Atc \"select count(*) from users where name = 'after-reboot'\"" 1 "replication flows after restart"
  $B reset dev >/dev/null; check "reset command succeeds" "$?" 0
  check "reset re-clones from replica (no seeded row twice)" "$(psql "$($B url dev)" -Atc "select count(*) from users where name = 'seeded'")" 1
  check "reset removes branch-only writes" "$(psql "$($B url dev)" -Atc "select count(*) from users where name = 'dev'")" 0
  check "reset includes newer replicated rows" "$(psql "$($B url dev)" -Atc "select count(*) from users where name = 'after-reboot'")" 1
  $B rm dev; $B rm dev2; $B rm prod
  # RDS-like source: a non-superuser role in an rds_superuser group. No event trigger is
  # possible, pg_dumpall --roles-only is refused, and migrations must be reconciled.
  psql "$SRC" -Xq -c "create role rds_superuser nologin; create role app login replication password 'apppw' in role rds_superuser; grant create on database postgres to app; alter table users owner to app; alter table orders owner to app; grant usage, create on schema public to app;"
  APP="postgresql://app:apppw@127.0.0.1:$(port src)/postgres"
  $B preflight postgres "$APP" --format json | grep -q '"name":"can create event trigger","state":"pass"'; check "preflight treats rds_superuser membership as privileged" "$?" 0
  $B sync postgres rds "$APP" 2>&1 | grep -q 'reconcile rds'; check "sync without superuser falls back and names reconcile" "$?" 0
  RDS=$($B url rds)
  waitfor "psql '$RDS' -Atc 'select count(*) from users'" "$(psql "$SRC" -Atc 'select count(*) from users')" "initial copy from the restricted role"
  $B status rds | grep -q 'not tracked'; check "status says schema changes are not tracked" "$?" 0
  psql "$APP" -Xqc "alter table users add column tier text"
  psql "$APP" -Xqc "insert into users(name, tier) values ('gold-user', 'gold')"
  waitfor "$B status rds | grep -c PAUSED" 1 "a row with an unknown column pauses the replica"
  $B repair rds | grep -q 'added 1 columns'; check "repair reconciles the missing column" "$?" 0
  waitfor "psql '$RDS' -Atc \"select tier from users where name = 'gold-user'\"" gold "the row arrives after reconcile"
  psql "$APP" -Xqc "create table invoices(id serial primary key, amt int); insert into invoices(amt) values (9);"
  $B reconcile rds | grep -q 'and 1 tables'; check "reconcile adds a new table to the publication and replica" "$?" 0
  waitfor "psql '$RDS' -Atc 'select count(*) from invoices'" 1 "new table copied after reconcile"
  $B rm rds
  check "no slot left on source" "$(psql "$SRC" -Atc 'select count(*) from pg_replication_slots')" 0
  check "no publication left on source" "$(psql "$SRC" -Atc 'select count(*) from pg_publication')" 0
  check "no event trigger left on source" "$(psql "$SRC" -Atc 'select count(*) from pg_event_trigger')" 0
  check "no snapshotdb schema left on source" "$(psql "$SRC" -Atc "select count(*) from pg_namespace where nspname like 'snapshotdb%'")" 0
  $B rm src
fi

if [ "${E2E_POSTGRES_ONLY:-0}" != 1 ] && command -v mysqld >/dev/null; then
  echo "mysql"
  mu() { local auth=${1#mysql://}; auth=${auth%%@*}; local user=${auth%%:*}; local hp=${auth#*:}; local port=${1##*:}; port=${port%%/*}; MYSQL_PWD="$hp" mysql --no-defaults -h 127.0.0.1 -P "$port" -u "$user" -N -B -e "$2"; }
  $B import mysql msrc --new >/dev/null; MS=$($B url msrc)
  echo "$MS" | grep -qE '^mysql://root:[0-9a-f]{32}@'; check "root has a generated password" "$?" 0
  check "TCP without the password is refused" "$(refused mysql --no-defaults -h 127.0.0.1 -P "$(port msrc)" -u root -e 'select 1')" refused
  mu "$MS" "create database app; create table app.t(id int auto_increment primary key, v int); insert into app.t(v) values (1),(2),(3)"
  $B preflight mysql "$MS" >/dev/null 2>&1; check "preflight passes" "$?" 0
  $B sync mysql mrep "$MS" >/dev/null 2>&1; MR=$($B url mrep)
  waitfor "mu '$MR' 'select count(*) from app.t'" 3 "initial copy"
  mu "$MS" "insert into app.t(v) values (4); alter table app.t add column note varchar(10); update app.t set note = 'n' where id = 1"
  waitfor "mu '$MR' 'select count(*) from app.t'" 4 "live change replicated"
  waitfor "mu '$MR' 'select note from app.t where id = 1'" n "schema change replicated"
  $B status mrep | grep -q 'caught up'; check "status reports caught up" "$?" 0
  # poisoned transaction
  mu "$MR" "insert into app.t(id, v) values (500, 0)"
  mu "$MS" "insert into app.t(id, v) values (500, 1)"
  waitfor "$B status mrep | grep -c PAUSED" 1 "conflict pauses the replica SQL thread"
  check "paused MySQL replica refuses branches" "$(refused $B create mysql-paused --from mrep)" refused
  $B repair mrep | grep -q skipped; check "repair skips the failing GTID" "$?" 0
  mu "$MS" "insert into app.t(v) values (5)"
  waitfor "mu '$MR' 'select count(*) from app.t where v = 5'" 1 "rows flow again after repair"
  $B create mdev --from mrep >/dev/null || exit 1; MD=$($B url mdev)
  check "branch has its own password" "$([ "$(pwof "$MD")" != "$(pwof "$MR")" ] && echo distinct)" distinct
  check "MySQL branch uses restricted agent account" "$(mu "$MD" 'SELECT CURRENT_USER()')" snapshotdb_agent@localhost
  check "MySQL agent cannot read host files" "$(mu "$MD" "SELECT LOAD_FILE('/etc/passwd') IS NULL")" 1
  check "MySQL agent cannot create administrators" "$(refused mu "$MD" "CREATE USER 'should_not_exist'@'localhost'")" refused
  check "branch has no replication channel" "$(MYSQL_PWD=$(cat "$SNAPSHOTDB_HOME/mdev/password") mysql --no-defaults -u root --socket="$SNAPSHOTDB_HOME/mdev/run/sock" -N -Be 'select count(*) from performance_schema.replication_connection_configuration')" 0
  mu "$MS" "insert into app.t(v) values (6)"
  waitfor "mu '$MR' 'select count(*) from app.t where v = 6'" 1 "replica keeps streaming after branch"
  check "branch isolated from later production writes" "$(mu "$MD" 'select count(*) from app.t where v = 6')" 0
  $B stop mdev; check "connecting to a suspended branch resumes it" "$(mu "$MD" 'select 1')" 1
  $B rm mdev; $B rm mrep; $B rm msrc
fi

if [ "${E2E_POSTGRES_ONLY:-0}" != 1 ] && command -v mongod >/dev/null && command -v mongosh >/dev/null; then
  echo "mongodb"
  mq() { mongosh --quiet "$1" --eval "$2"; }
  $B import mongodb mg --new >/dev/null; U=$($B url mg)
  echo "$U" | grep -qE '^mongodb://snapshotdb:[0-9a-f]{32}@'; check "root has generated credentials" "$?" 0
  check "unauthenticated access is refused" "$(refused mongosh --quiet "mongodb://127.0.0.1:$(port mg)/?directConnection=true" --eval 'db.getSiblingDB("app").t.countDocuments()')" refused
  mq "$U" 'db.getSiblingDB("app").t.insertMany([{_id:1},{_id:2},{_id:3}])' >/dev/null
  check "root is a writable single-node replica set" "$(mq "$U" 'print(db.hello().isWritablePrimary)')" true
  $B create mg2 --from mg >/dev/null || exit 1; U2=$($B url mg2)
  check "branch has its own password" "$([ "$(pwof "$U2")" != "$(pwof "$U")" ] && echo distinct)" distinct
  check "MongoDB agent cannot create administrators" "$(refused mq "$U2" 'db.getSiblingDB("admin").createUser({user:"should_not_exist",pwd:"test-only",roles:["root"]})')" refused
  check "clone reconfigured onto its own engine port" "$(mq "$U2" 'print(db.hello().me)')" "127.0.0.1:$(cat "$SNAPSHOTDB_HOME/mg2/run/eport")"
  mq "$U2" 'db.getSiblingDB("app").t.insertOne({_id:4})' >/dev/null
  check "branch has its own writes" "$(mq "$U2" 'print(db.getSiblingDB("app").t.countDocuments())')" 4
  check "parent unchanged" "$(mq "$U" 'print(db.getSiblingDB("app").t.countDocuments())')" 3
  $B stop mg2; check "connecting to a suspended branch resumes it" "$(mq "$U2" 'print(db.getSiblingDB("app").t.countDocuments())')" 4
  if command -v mongodump >/dev/null; then
    $B preflight mongodb "$U" >/dev/null 2>&1; check "preflight passes" "$?" 0
    $B sync mongodb mrep "$U" >/dev/null 2>&1; RU=$($B url mrep)
    waitfor "mq '$RU' 'print(db.getSiblingDB(\"app\").t.countDocuments())'" 3 "initial copy"
    mq "$U" 'const t = db.getSiblingDB("app").t; t.insertOne({_id:5}); t.updateOne({_id:1}, {$set: {v: 9}}); t.deleteOne({_id:2})' >/dev/null
    waitfor "mq '$RU' 'print(db.getSiblingDB(\"app\").t.findOne({_id:1}).v)'" 9 "update replicated"
    waitfor "mq '$RU' 'print(db.getSiblingDB(\"app\").t.countDocuments({_id:2}))'" 0 "delete replicated"
    check "insert replicated" "$(mq "$RU" 'print(db.getSiblingDB("app").t.countDocuments({_id:5}))')" 1
    mq "$U" 'db.getSiblingDB("app").t.createIndex({v: 1}, {name: "v_1"}); db.getSiblingDB("app").createCollection("logs", {capped: true, size: 1048576})' >/dev/null
    waitfor "mq '$RU' 'print(db.getSiblingDB(\"app\").t.getIndexes().some(i => i.name === \"v_1\"))'" true "index created on source appears on replica"
    waitfor "mq '$RU' 'print(db.getSiblingDB(\"app\").getCollectionInfos({name: \"logs\"})[0].options.capped)'" true "collection options replicated"
    mq "$U" 'db.getSiblingDB("app").audit_fail.insertOne({_id:1,v:1})' >/dev/null
    waitfor "mq '$RU' 'print(db.getSiblingDB(\"app\").audit_fail.countDocuments())'" 1 "fault fixture replicated"
    mq "$RU" 'db.getSiblingDB("app").audit_fail.createIndex({v:1},{unique:true})' >/dev/null
    mq "$U" 'db.getSiblingDB("app").audit_fail.insertOne({_id:2,v:1})' >/dev/null
    waitfor "test -f '$SNAPSHOTDB_HOME/mrep/run/tail.failed' && echo paused" paused "failed event pauses MongoDB replication"
    check "failed MongoDB replica refuses branches" "$(refused $B create mongo-paused --from mrep)" refused
    mq "$RU" 'db.getSiblingDB("app").audit_fail.dropIndex("v_1")' >/dev/null
    $B repair mrep >/dev/null
    waitfor "mq '$RU' 'print(db.getSiblingDB(\"app\").audit_fail.countDocuments())'" 2 "repair retries failed event without data loss"
    $B status mrep | grep -q tailing; check "status reports tailing" "$?" 0
    $B create mdev --from mrep >/dev/null; DU=$($B url mdev)
    mq "$U" 'db.getSiblingDB("app").t.insertOne({_id:6})' >/dev/null
    waitfor "mq '$RU' 'print(db.getSiblingDB(\"app\").t.countDocuments({_id:6}))'" 1 "replica keeps tailing after branch"
    check "branch isolated from later production writes" "$(mq "$DU" 'print(db.getSiblingDB("app").t.countDocuments({_id:6}))')" 0
    $B rm mdev; $B rm mrep
    check "tailer process gone" "$(pgrep -f 'tail.js' | wc -l | tr -d ' ')" 0
  fi
  $B rm mg2; $B rm mg
fi

check "no proxies left" "$(pgrep -f "^$B _proxy (src|prod|dev|dev2|msrc|mrep|mdev|mg|mg2)$" | wc -l | tr -d ' ')" 0
kill "$server_pid"; wait "$server_pid" 2>/dev/null
trap - EXIT
rm -rf "$SNAPSHOTDB_HOME"
[ $fail = 0 ] && echo "all passed" || { echo "FAILURES"; exit 1; }
