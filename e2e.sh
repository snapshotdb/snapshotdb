#!/usr/bin/env bash
# End-to-end check: a fake "production" made by anybranch itself, preflight, a synced
# replica, schema changes, a poisoned transaction repaired, detached branches, suspend and
# resume through the proxy, settings, and a clean teardown. Needs the engine binaries on
# PATH; engines that are not installed are skipped. Uses a throwaway ANYBRANCH_HOME unless
# one is exported (e.g. a Btrfs mount on Linux).
set -u
cd "$(dirname "$0")"
cargo build --release -q || exit 1
B=$PWD/target/release/anybranch
export ANYBRANCH_HOME=${ANYBRANCH_HOME:-$(mktemp -d /tmp/anybranch-e2e.XXXX)}
export ANYBRANCH_IDLE_MINUTES=0
fail=0
check() { if [ "$2" = "$3" ]; then echo "  ok   $1 = $2"; else echo "  FAIL $1: got '$2' want '$3'"; fail=1; fi; }
waitfor() { for _ in $(seq 1 120); do v=$(eval "$1" 2>/dev/null); [ "$v" = "$2" ] && break; sleep 0.3; done; check "$3" "$v" "$2"; }
port() { local u; u=$($B url "$1"); u=${u##*:}; echo "${u%%/*}"; }
st() { $B list | sed 's/^[* ] //' | awk -F'\t' -v n="$1" '$1==n{print $4}'; }

if command -v pg_ctl >/dev/null; then
  echo "postgres"
  $B import postgres src --new >/dev/null; SRC=$($B url src)
  psql "$SRC" -Xq -c "create table users(id serial primary key, name text); insert into users(name) select 'u'||g from generate_series(1,1000) g; create table nopk(x int);"
  $B preflight postgres "$SRC" >/dev/null 2>&1; check "preflight passes on a good source" "$?" 0
  $B preflight postgres "postgresql://nobody@127.0.0.1:1/x" >/dev/null 2>&1; check "preflight exits 2 on a bad source" "$?" 2
  $B preflight postgres "$SRC" --format json | grep -q '"name":"replica identity","state":"warn"'; check "preflight warns about table without primary key" "$?" 0
  $B sync postgres prod "$SRC" >/dev/null 2>&1; PROD=$($B url prod)
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
  $B status prod | grep -q 'retaining'; check "status shows WAL retained on source" "$?" 0
  # a poisoned transaction: the replica already has id 5000, production inserts it too
  psql "$PROD" -Atqc "insert into users(id, name) values (5000, 'local')"
  psql "$SRC" -Atqc "insert into users(id, name) values (5000, 'remote')"
  waitfor "$B status prod | grep -c PAUSED" 1 "conflict pauses replication instead of retrying forever"
  $B repair prod | grep -q skipped; check "repair skips the poisoned transaction" "$?" 0
  psql "$SRC" -Atqc "insert into users(name) values ('after-repair')"
  waitfor "psql '$PROD' -Atc \"select count(*) from users where name = 'after-repair'\"" 1 "rows flow again after repair"
  check "replica has no event trigger" "$(psql "$PROD" -Atc 'select count(*) from pg_event_trigger')" 0
  # branches
  $B settings prod set branch_sql "insert into users(name) values ('seeded')"
  $B settings prod set branch_sql "insert into users(name) values ('fixtures')" --hook 20-fixtures
  $B create dev --from prod --print-url >/dev/null; DEV=$($B url dev)
  check "create --print-url made dev current" "$($B info --print-url)" "$DEV"
  check "re-running create returns the same URL" "$($B create dev --from prod --print-url)" "$DEV"
  $B create dev >/dev/null 2>&1; check "usage error exits 2" "$?" 2
  $B create dev --from nope --format json | grep -q '"error"'; check "json error output" "$?" 0
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
  $B lock prod; $B rm prod >/dev/null 2>&1; check "locked root refuses rm" "$?" 1
  $B unlock prod
  # simulate a reboot: kill every proxy and engine, then `up`
  for b in src prod dev dev2; do kill -TERM "$(cat "$ANYBRANCH_HOME/$b/run/proxypid")" 2>/dev/null; $B stop "$b" 2>/dev/null; done; sleep 1
  check "after 'reboot' URLs are dead" "$(st dev)" stopped
  $B up >/dev/null
  check "up restores the synced root" "$(st prod)" syncing
  check "up restores branch proxies (suspended, resume on connect)" "$(st dev)" suspended
  check "URL unchanged across the reboot" "$($B url dev)" "$DEV"
  check "branch resumes on first connection after reboot" "$(psql "$DEV" -Atc 'select 1')" 1
  $B start src >/dev/null
  $B reset dev >/dev/null
  check "reset re-clones from replica (no seeded row twice)" "$(psql "$($B url dev)" -Atc "select count(*) from users where name = 'seeded'")" 1
  $B rm dev; $B rm dev2; $B rm prod
  check "no slot left on source" "$(psql "$SRC" -Atc 'select count(*) from pg_replication_slots')" 0
  check "no publication left on source" "$(psql "$SRC" -Atc 'select count(*) from pg_publication')" 0
  check "no event trigger left on source" "$(psql "$SRC" -Atc 'select count(*) from pg_event_trigger')" 0
  check "no anybranch schema left on source" "$(psql "$SRC" -Atc "select count(*) from pg_namespace where nspname like 'anybranch%'")" 0
  $B rm src
fi

if command -v mysqld >/dev/null; then
  echo "mysql"
  m() { mysql --no-defaults -h 127.0.0.1 -P "$1" -u root -N -B -e "$2"; }
  $B import mysql msrc --new >/dev/null; P=$(port msrc)
  m "$P" "create database app; create table app.t(id int auto_increment primary key, v int); insert into app.t(v) values (1),(2),(3)"
  $B preflight mysql "mysql://root@127.0.0.1:$P/" >/dev/null 2>&1; check "preflight passes" "$?" 0
  $B sync mysql mrep "mysql://root@127.0.0.1:$P/" >/dev/null 2>&1; R=$(port mrep)
  waitfor "m $R 'select count(*) from app.t'" 3 "initial copy"
  m "$P" "insert into app.t(v) values (4); alter table app.t add column note varchar(10); update app.t set note = 'n' where id = 1"
  waitfor "m $R 'select count(*) from app.t'" 4 "live change replicated"
  waitfor "m $R 'select note from app.t where id = 1'" n "schema change replicated"
  $B status mrep | grep -q 'caught up'; check "status reports caught up" "$?" 0
  # poisoned transaction
  m "$R" "insert into app.t(id, v) values (500, 0)"
  m "$P" "insert into app.t(id, v) values (500, 1)"
  waitfor "$B status mrep | grep -c PAUSED" 1 "conflict pauses the replica SQL thread"
  $B repair mrep | grep -q skipped; check "repair skips the failing GTID" "$?" 0
  m "$P" "insert into app.t(v) values (5)"
  waitfor "m $R 'select count(*) from app.t where v = 5'" 1 "rows flow again after repair"
  $B create mdev --from mrep >/dev/null; D=$(port mdev)
  check "branch has no replication channel" "$(m "$D" 'select count(*) from performance_schema.replication_connection_configuration')" 0
  m "$P" "insert into app.t(v) values (6)"
  waitfor "m $R 'select count(*) from app.t where v = 6'" 1 "replica keeps streaming after branch"
  check "branch isolated from later production writes" "$(m "$D" 'select count(*) from app.t where v = 6')" 0
  $B stop mdev; check "connecting to a suspended branch resumes it" "$(m "$D" 'select 1')" 1
  $B rm mdev; $B rm mrep; $B rm msrc
fi

if command -v mongod >/dev/null && command -v mongosh >/dev/null; then
  echo "mongodb"
  mq() { mongosh --quiet "$1" --eval "$2"; }
  $B import mongodb mg --new >/dev/null; U=$($B url mg)
  mq "$U" 'db.getSiblingDB("app").t.insertMany([{_id:1},{_id:2},{_id:3}])' >/dev/null
  check "root is a writable single-node replica set" "$(mq "$U" 'print(db.hello().isWritablePrimary)')" true
  $B create mg2 --from mg >/dev/null; U2=$($B url mg2)
  check "clone reconfigured onto its own engine port" "$(mq "$U2" 'print(rs.conf().members[0].host)')" "127.0.0.1:$(cat "$ANYBRANCH_HOME/mg2/run/eport")"
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

check "no proxies left" "$(pgrep -f 'anybranch _proxy (src|prod|dev|dev2|msrc|mrep|mdev|mg|mg2)$' | wc -l | tr -d ' ')" 0
rm -rf "$ANYBRANCH_HOME"
[ $fail = 0 ] && echo "all passed" || { echo "FAILURES"; exit 1; }
