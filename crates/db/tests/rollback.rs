//! `rollback_from_block` 통합 테스트 — 실제 PostgreSQL 필요.
//!
//! `#[ignore]`. 실행: docker PG 기동 후 `cargo test -p db -- --ignored`.
//! FORK는 시드 최대 블록(~18,000,002)보다 **확실히 위**여야 한다 — rollback은
//! `block_number >= FORK`를 지우므로 FORK가 시드보다 낮으면 시드까지 삭제된다.
//! 공유 dev DB 위생을 위해 체크포인트는 원복한다.
//!
//! 블록 밴드 맵: 시드 ~18M / alert_rate 97.0M / labels 97.5M / alerts 98M /
//! **rollback 99M**. 본 파일은 반드시 **최상위 밴드**를 유지해야 하며
//! `rollback_from_block`을 쓰는 유일한 테스트여야 한다 — ≥FORK 전역 삭제라
//! 아래 밴드에서 호출하면 병렬 실행 시 다른 바이너리 픽스처를 지운다.

use db::error::DbError;
use db::models::{Block, ErrorCategory, FailedTransaction, TraceLog, Transaction};

const DEFAULT_URL: &str = "postgres://defi:defi@localhost:5432/defi_analytics";
const FORK: i64 = 99_000_000; // above seed max (~18M) so rollback never hits seed
const TXH: &str = "0xfeed000000000000000000000000000000000000000000000000000000000001";

fn db_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn rollback_from_block_is_idempotent_and_scoped() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    let now = chrono::Utc::now();
    let prior = db::queries::get_last_checkpoint(&pool, 1)
        .await
        .expect("read checkpoint");

    // ── fixtures at a high block, disjoint from seed ──
    db::queries::insert_blocks(
        &pool,
        &[Block {
            block_number: FORK,
            timestamp: now,
            gas_used: 1,
            block_hash: Some("0xaaa".to_string()),
            parent_hash: Some("0xbbb".to_string()),
        }],
    )
    .await
    .expect("insert block");
    db::queries::insert_transactions(
        &pool,
        &[Transaction {
            tx_hash: TXH.to_string(),
            from_addr: "0x01".to_string(),
            to_addr: None,
            block_number: FORK,
            gas_used: 1,
            gas_price: bigdecimal::BigDecimal::from(0),
            value: bigdecimal::BigDecimal::from(0),
            status: 0,
            input_data: None,
        }],
    )
    .await
    .expect("insert tx");
    db::queries::insert_failed_transactions(
        &pool,
        &[FailedTransaction {
            tx_hash: TXH.to_string(),
            error_category: ErrorCategory::Unknown,
            revert_reason: None,
            failing_function: None,
            gas_used: 1,
            timestamp: now,
        }],
    )
    .await
    .expect("insert failed");
    db::queries::insert_trace_logs(
        &pool,
        &[TraceLog {
            tx_hash: TXH.to_string(),
            call_depth: 0,
            call_type: "CALL".to_string(),
            from_addr: "0x01".to_string(),
            to_addr: None,
            value: bigdecimal::BigDecimal::from(0),
            gas_used: 1,
            input: None,
            output: None,
            error: None,
            trace_id: 0,
        }],
    )
    .await
    .expect("insert trace");
    db::queries::update_checkpoint(&pool, 1, FORK)
        .await
        .expect("set checkpoint");

    assert!(
        db::queries::get_failed_transaction(&pool, TXH)
            .await
            .is_ok(),
        "fixture present before rollback"
    );

    // ── rollback ──
    db::queries::rollback_from_block(&pool, FORK)
        .await
        .expect("rollback");

    assert!(
        matches!(
            db::queries::get_failed_transaction(&pool, TXH).await,
            Err(DbError::NotFound(_))
        ),
        "failed_transaction removed"
    );
    assert!(
        db::queries::list_trace_logs_by_tx(&pool, TXH, i64::MAX)
            .await
            .expect("query")
            .is_empty(),
        "trace_log removed"
    );
    assert!(
        matches!(
            db::queries::get_block_by_number(&pool, FORK).await,
            Err(DbError::NotFound(_))
        ),
        "block removed"
    );
    assert_eq!(
        db::queries::get_last_checkpoint(&pool, 1)
            .await
            .expect("checkpoint"),
        Some(FORK - 1),
        "checkpoint rewound to fork-1"
    );
    assert!(
        db::queries::get_block_by_number(&pool, 18_000_000)
            .await
            .is_ok(),
        "seed block (< fork) untouched"
    );

    // ── idempotent: second rollback is a no-op, state stays consistent ──
    db::queries::rollback_from_block(&pool, FORK)
        .await
        .expect("rollback again");
    assert!(matches!(
        db::queries::get_block_by_number(&pool, FORK).await,
        Err(DbError::NotFound(_))
    ));
    assert_eq!(
        db::queries::get_last_checkpoint(&pool, 1)
            .await
            .expect("checkpoint"),
        Some(FORK - 1)
    );

    // restore prior checkpoint (shared dev DB hygiene)
    if let Some(p) = prior {
        db::queries::update_checkpoint(&pool, 1, p)
            .await
            .expect("restore checkpoint");
    }
}
