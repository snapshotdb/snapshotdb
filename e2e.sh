#!/usr/bin/env bash
# End-to-end check: a fake "production" made by anybranch itself, a synced replica,
# schema changes, a detached branch, and a clean teardown. Needs the engine binaries on
# PATH; engines that are not installed are skipped. Uses a throwaway ANYBRANCH_HOME
# unless one is exported (e.g. a Btrfs mount on Linux).
set -u
cd "$(dirname "$0")"
cargo build --release -q || exit 1
B=target/release/anybranch
export ANYBRANCH_HOME=${ANYBRANCH_HOME:-$(mktemp -d /tmp/anybranch-e2e.XXXX)}
fail=0
check() { if [ "$2" = "$3" ]; then echo "  ok   $1 = $2"; else echo "  FAIL $1: got '$2' want '$3'"; fail=1; fi; }
waitfor() { for _ in $(seq 1 100); do v=$(eval "$1" 2>/dev/null); [ "$v" = "$2" ] && break; sleep 0.3; done; check "$3" "$v" "$2"; }
port() { local u; u=$($B url "$1"); u=${u##*:}; echo "${u%%/*}"; }

if command -v pg_ctl >/dev/null; then
  echo "postgres"
  $B import postgres src --new >/dev/null; SRC=$($B url src)
  psql "$SRC" -Xq -c "create table users(id serial primary key, name text); insert into users(name) select 'u'||g from generate_series(1,1000) g; create table nopk(x int);"
  $B sync postgres prod "$SRC" 2>&1 | grep -q 'REPLICA IDENTITY FULL' ; check "warns about table without primary key" "$?" 0
  PROD=$($B url prod)
  waitfor "psql '$PROD' -Atc 'select count(*) from users'" 1000 "initial copy"
  psql "$SRC" -Atqc "insert into users(name) values ('live')"
  waitfor "psql '$PROD' -Atc 'select count(*) from users'" 1001 "live change replicated"
  # schema changes on production, one statement per query as migration tools send them
  psql "$SRC" -Xq <<'SQL'
alter table users add column email text;
update users set email = 'e' where id = 1;
create table orders(id serial primary key, amt int);
insert into orders(amt) values (7);
create index concurrently users_email on users(email);
SQL
  waitfor "psql '$PROD' -Atc \"select email from users where id = 1\"" e "column added on production appears on replica"
  waitfor "$B status prod >/dev/null; psql '$PROD' -Atc 'select count(*) from orders'" 1 "new table joins replication after refresh"
  $B status prod | grep -q '1 failed'; check "non-replayable DDL is recorded, not fatal" "$?" 0
  psql "$SRC" -Atqc "insert into users(name) values ('after-ddl')"
  waitfor "psql '$PROD' -Atc 'select count(*) from users'" 1002 "rows keep flowing after a failed DDL"
  check "replica has no event trigger" "$(psql "$PROD" -Atc 'select count(*) from pg_event_trigger')" 0
  # branch
  $B create dev --from prod >/dev/null; DEV=$($B url dev)
  check "branch has no subscription" "$(psql "$DEV" -Atc 'select count(*) from pg_subscription')" 0
  check "branch sequence advanced" "$(psql "$DEV" -Atqc "insert into users(name) values ('dev') returning id")" 1003
  psql "$SRC" -Atqc "insert into users(name) values ('after-branch')"
  waitfor "psql '$PROD' -Atc 'select count(*) from users'" 1003 "replica keeps streaming after branch"
  check "branch isolated from later production writes" "$(psql "$DEV" -Atc 'select count(*) from users')" 1003
  check "slot active on source" "$(psql "$SRC" -Atc 'select active from pg_replication_slots')" t
  $B reset dev >/dev/null
  check "reset re-clones from replica" "$(psql "$($B url dev)" -Atc 'select count(*) from users')" 1003
  $B rm dev; $B rm prod
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
  $B sync mysql mrep "mysql://root@127.0.0.1:$P/" >/dev/null 2>&1; R=$(port mrep)
  waitfor "m $R 'select count(*) from app.t'" 3 "initial copy"
  m "$P" "insert into app.t(v) values (4); alter table app.t add column note varchar(10); update app.t set note = 'n' where id = 1"
  waitfor "m $R 'select count(*) from app.t'" 4 "live change replicated"
  waitfor "m $R 'select note from app.t where id = 1'" n "schema change replicated"
  $B status mrep | grep -q 'caught up'; check "status reports caught up" "$?" 0
  $B create mdev --from mrep >/dev/null; D=$(port mdev)
  check "branch has no replication channel" "$(m "$D" 'select count(*) from performance_schema.replication_connection_configuration')" 0
  m "$P" "insert into app.t(v) values (5)"
  waitfor "m $R 'select count(*) from app.t'" 5 "replica keeps streaming after branch"
  check "branch isolated from later production writes" "$(m "$D" 'select count(*) from app.t')" 4
  $B rm mdev; $B rm mrep; $B rm msrc
fi

if command -v mongod >/dev/null && command -v mongosh >/dev/null; then
  echo "mongodb"
  mq() { mongosh --quiet "$1" --eval "$2"; }
  $B import mongodb mg --new >/dev/null; U=$($B url mg)
  mq "$U" 'db.getSiblingDB("app").t.insertMany([{_id:1},{_id:2},{_id:3}])' >/dev/null
  check "root is a writable single-node replica set" "$(mq "$U" 'print(db.hello().isWritablePrimary)')" true
  $B create mg2 --from mg >/dev/null; U2=$($B url mg2)
  check "clone reconfigured onto its own port" "$(mq "$U2" 'print(rs.conf().members[0].host)')" "127.0.0.1:$(port mg2)"
  mq "$U2" 'db.getSiblingDB("app").t.insertOne({_id:4})' >/dev/null
  check "branch has its own writes" "$(mq "$U2" 'print(db.getSiblingDB("app").t.countDocuments())')" 4
  check "parent unchanged" "$(mq "$U" 'print(db.getSiblingDB("app").t.countDocuments())')" 3
  if command -v mongodump >/dev/null; then
    $B sync mongodb mrep "$U" >/dev/null 2>&1; RU=$($B url mrep)
    waitfor "mq '$RU' 'print(db.getSiblingDB(\"app\").t.countDocuments())'" 3 "initial copy"
    mq "$U" 'const t = db.getSiblingDB("app").t; t.insertOne({_id:5}); t.updateOne({_id:1}, {$set: {v: 9}}); t.deleteOne({_id:2})' >/dev/null
    waitfor "mq '$RU' 'print(db.getSiblingDB(\"app\").t.findOne({_id:1}).v)'" 9 "update replicated"
    waitfor "mq '$RU' 'print(db.getSiblingDB(\"app\").t.countDocuments({_id:2}))'" 0 "delete replicated"
    check "insert replicated" "$(mq "$RU" 'print(db.getSiblingDB("app").t.countDocuments({_id:5}))')" 1
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

rm -rf "$ANYBRANCH_HOME"
[ $fail = 0 ] && echo "all passed" || { echo "FAILURES"; exit 1; }
