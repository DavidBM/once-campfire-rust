#!/usr/bin/env bash
# search_reachable's query as main runs it (ORDER BY created_at) and as this change does (ORDER BY idx.rowid), for
# `coffee` (2% of the messages) and `message` (every message), searched by a user in every room with messages (id 1)
# and by one in none of them (bench-dm2k, in 2,000 empty direct rooms). Every query runs 5 times. Read-only.
#   bench/results/search-rowid-20261006/queries.sh <database>
set -euo pipefail
db=$1
cols='"messages"."id", "messages"."room_id", "messages"."creator_id", "messages"."client_message_id", "messages"."created_at", "messages"."updated_at"'
from='FROM "messages" INNER JOIN "rooms" ON "messages"."room_id" = "rooms"."id" INNER JOIN "memberships" ON "rooms"."id" = "memberships"."room_id" join message_search_index idx on messages.id = idx.rowid'
outsider=$(sqlite3 -readonly "$db" "SELECT id FROM users WHERE email_address = 'bench-dm2k@example.com'")

for word in coffee message; do
  echo "== \"$word\" matches $(sqlite3 -readonly "$db" "SELECT count(*) FROM message_search_index WHERE body MATCH '\"$word\"'") messages"
  for user in 1 "$outsider"; do
    for order in '"messages"."created_at" DESC' 'idx.rowid DESC'; do
      q="SELECT $cols $from WHERE \"memberships\".\"user_id\" = $user AND (idx.body match '\"$word\"') ORDER BY $order LIMIT 100"
      echo "-- user $user, ORDER BY $order"
      sqlite3 -readonly "$db" "EXPLAIN QUERY PLAN $q" | tr '\n' ' '; echo
      echo "rows, first id, last id, sum of ids: $(sqlite3 -readonly "$db" "SELECT count(*), min(id), max(id), sum(id) FROM ($q)")"
      { echo .timer on; for _ in 1 2 3 4 5; do echo "SELECT count(*) FROM ($q);"; done; } | sqlite3 -readonly "$db" | grep -o 'real [0-9.]*' | tr '\n' ' '; echo
    done
  done
done
