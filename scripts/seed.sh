#!/bin/sh
# 시드 SQL 적용 스크립트 — .github/workflows/ci.yml과 docker-compose.yml의
# seed 서비스가 공유한다 (적용 순서의 단일 출처).
#
# 사용법: scripts/seed.sh <conninfo>
#   conninfo: psql이 받는 접속 문자열 (예: postgres://user:pass@host:5432/db)
#
# 전제: 마이그레이션(migrations/)이 이미 적용된 DB. 파일 순서가 곧 의존
# 순서다: views → procedures → triggers → seed DML.
#
# compose seed 컨테이너는 postgres:16-alpine(busybox sh)에서 실행되므로
# POSIX sh 문법만 사용한다.
set -eu

if [ "$#" -ne 1 ]; then
    echo "usage: $0 <conninfo>" >&2
    exit 1
fi
conninfo=$1

# 스크립트 위치 기준으로 sql/ 해석 — 호출 위치(CWD)에 의존하지 않는다.
sql_dir="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)/sql"

for f in \
    "$sql_dir/views/001_views.sql" \
    "$sql_dir/procedures/001_procedures.sql" \
    "$sql_dir/triggers/001_triggers.sql" \
    "$sql_dir/dml/001_seed_data.sql"; do
    echo "== $f"
    psql "$conninfo" -v ON_ERROR_STOP=1 -q -f "$f"
done
echo "Done — seed data loaded successfully"
