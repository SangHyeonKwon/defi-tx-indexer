//! `crates/db` 통합 테스트 — 실제 PostgreSQL이 필요하다.
//!
//! 전부 `#[ignore]`이므로 `cargo test`(기본)에서는 실행되지 않는다(CI는 green 유지).
//! 실행: docker PG 기동 후 `cargo test -p db -- --ignored`.
//! `DATABASE_URL` 미설정 시 docker-compose 기본값을 사용한다.

use db::error::DbError;
use db::models::{ErrorCategory, TimeBucket};

const DEFAULT_URL: &str = "postgres://defi:defi@localhost:5432/defi_analytics";

/// 시드 데이터의 알려진 실패 tx (`failed_transaction` + `trace_log` 프레임 보유).
const GOOD: &str = "0xdead000000000000000000000000000000000000000000000000000000000001";
/// 어떤 테이블에도 없는 해시.
const BAD: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";

fn db_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn get_failed_transaction_known_hash_ok() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    let ftx = db::queries::get_failed_transaction(&pool, GOOD)
        .await
        .expect("seeded failed tx should exist");
    assert_eq!(ftx.tx_hash, GOOD);
    assert!(ftx.gas_used > 0, "gas_used should be positive");
}

#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn get_failed_transaction_unknown_hash_not_found() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    let err = db::queries::get_failed_transaction(&pool, BAD)
        .await
        .expect_err("unknown hash must be NotFound");
    assert!(matches!(err, DbError::NotFound(_)), "got {err:?}");
}

/// S01 H1 회귀 가드: 콜트리는 pre-order DFS여야 한다
/// (root 프레임이 첫번째 + `trace_id` strictly ascending).
/// 이 테스트는 `ORDER BY call_depth ...`였던 옛 구현에서 실패한다.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn trace_logs_preserve_pre_order_invariant() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    let frames = db::queries::list_trace_logs_by_tx(&pool, GOOD, i64::MAX)
        .await
        .expect("query ok");

    assert!(!frames.is_empty(), "seeded tx should have trace frames");
    assert_eq!(
        frames[0].call_depth, 0,
        "root frame must be first (pre-order)"
    );
    for w in frames.windows(2) {
        assert!(
            w[1].trace_id > w[0].trace_id,
            "trace_id must be strictly ascending (pre-order DFS); got {} then {}",
            w[0].trace_id,
            w[1].trace_id
        );
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn trace_logs_unknown_hash_is_empty_not_error() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    let frames = db::queries::list_trace_logs_by_tx(&pool, BAD, i64::MAX)
        .await
        .expect("empty result is not an error");
    assert!(frames.is_empty());
}

/// S02: 필터·페이지네이션 + total 불변식.
/// total(필터 전체) ≥ 반환 길이, limit이 페이지를 자른다, 필터가 실제로 좁힌다.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn list_failed_transactions_filter_and_total_invariants() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");

    let total_all = db::queries::count_failed_transactions(&pool, None, None, None)
        .await
        .expect("count all");
    let total_unknown =
        db::queries::count_failed_transactions(&pool, Some(&ErrorCategory::Unknown), None, None)
            .await
            .expect("count filtered");
    assert!(total_all >= total_unknown, "필터가 전체보다 많을 수 없다");
    assert!(total_unknown >= 1, "시드에 UNKNOWN 실패가 있어야 한다");

    let page = db::queries::list_failed_transactions(
        &pool,
        Some(&ErrorCategory::Unknown),
        None,
        None,
        2,
        0,
    )
    .await
    .expect("list filtered");
    assert!(page.len() as i64 <= 2, "limit이 페이지를 잘라야 한다");
    assert!(
        page.iter()
            .all(|f| f.error_category == ErrorCategory::Unknown),
        "필터된 행은 전부 UNKNOWN 이어야 한다"
    );
    assert!(
        total_unknown >= page.len() as i64,
        "total은 페이지 길이 이상이어야 한다"
    );
}

/// 미래 구간 필터는 0건(에러 아님).
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn list_failed_transactions_future_window_is_empty() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    let from = chrono::DateTime::parse_from_rfc3339("2999-01-01T00:00:00Z")
        .expect("valid ts")
        .with_timezone(&chrono::Utc);
    let rows = db::queries::list_failed_transactions(&pool, None, Some(from), None, 50, 0)
        .await
        .expect("query ok");
    assert!(rows.is_empty(), "미래 구간엔 실패가 없어야 한다");
}

/// S03: 시계열 버킷 합 == 전체 카운트(재조정), 버킷 단조 증가.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn failed_tx_timeseries_reconciles_and_is_ordered() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    let pts = db::queries::failed_tx_timeseries(&pool, &TimeBucket::Day, None, None)
        .await
        .expect("timeseries ok");

    let sum: i64 = pts.iter().map(|p| p.failure_count).sum();
    let total = db::queries::count_failed_transactions(&pool, None, None, None)
        .await
        .expect("count all");
    assert_eq!(sum, total, "버킷 합은 전체 카운트와 일치해야 한다");

    for w in pts.windows(2) {
        assert!(w[1].bucket >= w[0].bucket, "버킷은 단조 증가해야 한다");
    }
}

/// S04 L3: `limit`이 콜트리 프레임 수를 자른다 (잘림 감지의 토대).
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn trace_logs_respects_limit() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    let one = db::queries::list_trace_logs_by_tx(&pool, GOOD, 1)
        .await
        .expect("query ok");
    assert_eq!(one.len(), 1, "limit=1 이면 정확히 1프레임이어야 한다");
}

/// S10-T01: `get_first_error_frame`이 콜트리의 첫 error frame과 정확히 일치 (자기일관성).
///
/// `trace_log`를 trace_id ASC로 받아 첫 `error IS NOT NULL`인 frame을 직접 찾아
/// `get_first_error_frame`이 반환한 값과 같은지 비교한다. KNOWLEDGE Lesson:
/// 시드 GOOD tx의 trace_log엔 슬리피지 error frame이 박혀 있다(S01 실측).
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn first_error_frame_matches_call_tree_first_error() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");

    let frames = db::queries::list_trace_logs_by_tx(&pool, GOOD, i64::MAX)
        .await
        .expect("trace ok");
    let expected = frames.iter().find(|t| t.error.is_some()).cloned();
    let root = db::queries::get_first_error_frame(&pool, GOOD)
        .await
        .expect("query ok");

    match (expected, root) {
        (Some(e), Some(r)) => {
            assert_eq!(
                r.trace_id, e.trace_id,
                "root_cause must equal the first error frame in trace_id ASC order"
            );
            assert!(
                r.error.is_some(),
                "root_cause.error must be Some by definition"
            );
        }
        (None, None) => {
            // 시드 변동으로 error frame이 없으면 root_cause도 None — 자기일관성 유지.
        }
        (e, r) => panic!("mismatch: expected_first_error={e:?}, got root_cause={r:?}"),
    }
}

/// S10-T01: 트레이스가 없는 해시 → `None` (에러 아님).
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn first_error_frame_unknown_hash_is_none() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    let root = db::queries::get_first_error_frame(&pool, BAD)
        .await
        .expect("query ok");
    assert!(root.is_none(), "BAD hash has no trace rows → None");
}

/// UNKNOWN revert 클러스터 (vw_unknown_revert_clusters) — 시드에 UNKNOWN 실패가
/// 있으므로 최소 1개의 `NO_DATA` 클러스터(`(no revert data)`)가 나와야 하고,
/// 뷰 정렬(`occurrences DESC`)과 집계 불변식(pct 합 ≤ 100, occurrences ≥ senders)이
/// 유지되어야 한다.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn unknown_revert_clusters_seed_shape_and_invariants() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    let rows = db::queries::get_unknown_revert_clusters(&pool)
        .await
        .expect("query ok");

    assert!(!rows.is_empty(), "seed has UNKNOWN failures → ≥1 cluster");

    let no_data = rows
        .iter()
        .find(|r| r.cluster_kind == "NO_DATA")
        .expect("seeded bare-revert failure must cluster as NO_DATA");
    assert_eq!(no_data.template, "(no revert data)");
    assert!(no_data.occurrences >= 1);
    assert!(
        no_data.sample_tx_hash.is_some(),
        "non-empty cluster carries a sample tx hash"
    );

    let mut pct_sum = 0.0_f64;
    for r in &rows {
        assert!(
            matches!(
                r.cluster_kind.as_str(),
                "NO_DATA" | "CUSTOM_ERROR" | "PANIC" | "TEXT"
            ),
            "unexpected cluster_kind: {}",
            r.cluster_kind
        );
        assert!(
            r.distinct_senders <= r.occurrences,
            "senders cannot exceed occurrences"
        );
        assert!(r.first_seen <= r.last_seen);
        pct_sum += r
            .pct_of_unknown
            .to_string()
            .parse::<f64>()
            .expect("pct is numeric");
    }
    assert!(
        pct_sum <= 100.0 + 0.5,
        "pct_of_unknown must sum to ≤ ~100 (rounding slack), got {pct_sum}"
    );

    for w in rows.windows(2) {
        assert!(
            w[0].occurrences >= w[1].occurrences,
            "view must be ordered occurrences DESC"
        );
    }
}
