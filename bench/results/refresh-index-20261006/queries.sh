#!/usr/bin/env bash
# page_updated_since on room 1 for several `since`, as main runs it (no index on updated_at, ORDER BY created_at) and
# as this change does (index on (room_id, updated_at), ORDER BY +created_at), then the write cost of the index.
# Like the refresh controller, each query leaves out the 40 ids page_created_since finds. Every query runs 5 times.
#   bench/results/refresh-index-20261006/queries.sh <copy of the database>
# It drops and creates the index on the file it's given: use a copy.
set -euo pipefail
db=$1
cols='"messages"."id", "messages"."room_id", "messages"."creator_id", "messages"."client_message_id", "messages"."created_at", "messages"."updated_at"'

run() { # run <label> <order> <since>
  local since=$3 new without q n
  new=$(sqlite3 "$db" "SELECT group_concat(id) FROM (SELECT id FROM messages WHERE room_id = 1 AND (created_at > '$since') ORDER BY created_at ASC LIMIT 40)")
  without=""; [ -n "$new" ] && without=" AND \"messages\".\"id\" NOT IN ($new)"
  q="SELECT $cols FROM \"messages\" WHERE \"messages\".\"room_id\" = 1$without AND (updated_at > '$since') ORDER BY $2 DESC LIMIT 40"
  n=$(sqlite3 "$db" "SELECT count(*) FROM messages WHERE room_id = 1 AND updated_at > '$since'")
  echo "== $1, since $since ($n updated since)"
  sqlite3 "$db" "EXPLAIN QUERY PLAN $q" | tr '\n' ' '; echo
  echo "rows, sum of ids: $(sqlite3 "$db" "SELECT count(*), sum(id) FROM ($q)")"
  { echo .timer on; for _ in 1 2 3 4 5; do echo "SELECT count(*) FROM ($q);"; done; } | sqlite3 "$db" | grep -o 'real [0-9.]*' | tr '\n' ' '; echo
}

writes() { # 10,000 inserts and 10,000 touches, each in one transaction, rolled back
  { echo .timer on
    echo "BEGIN; WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 10000) INSERT INTO messages (room_id, creator_id, client_message_id, created_at, updated_at) SELECT 1 + i % 5, 1, 'probe-' || i, '2026-10-07 00:00:00', '2026-10-07 00:00:00' FROM n; ROLLBACK;"
    echo "BEGIN; UPDATE messages SET updated_at = '2026-10-08 00:00:00' WHERE id IN (SELECT id FROM messages WHERE id % 200 = 7); ROLLBACK;"
  } | sqlite3 "$db" | grep -o 'real [0-9.]*' | tr '\n' ' '; echo " <- $1: 10,000 inserts, 10,000 touches"
}

SINCES=('2026-10-06 18:00:00' '2026-10-04 00:00:00' '2020-01-24 03:00:02' '2020-01-21 20:00:02' '1970-01-01 00:00:00')
sqlite3 "$db" 'DROP INDEX IF EXISTS "index_messages_on_room_id_and_updated_at"'
sqlite3 "$db" 'CREATE INDEX IF NOT EXISTS "index_messages_on_room_id_and_created_at" ON "messages" ("room_id", "created_at")'
for s in "${SINCES[@]}"; do run main '"messages"."created_at"' "$s"; done
for _ in 1 2 3; do writes "without the index"; done
{ echo .timer on; echo 'CREATE INDEX IF NOT EXISTS "index_messages_on_room_id_and_updated_at" ON "messages" ("room_id", "updated_at");'; } |
  sqlite3 "$db" | grep -o 'real [0-9.]*' | sed 's/$/ <- creating the index/'
for s in "${SINCES[@]}"; do run pr '+"messages"."created_at"' "$s"; done
for _ in 1 2 3; do writes "with the index"; done
echo "index size: $(sqlite3 "$db" "SELECT round(sum(pgsize) / 1048576.0, 1) FROM dbstat WHERE name = 'index_messages_on_room_id_and_updated_at'") MB of $(sqlite3 "$db" "SELECT round(page_count * page_size / 1048576.0, 1) FROM pragma_page_count, pragma_page_size") MB"
