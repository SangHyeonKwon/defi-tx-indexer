//! S09 / M003: contract_label × failed_tx aggregate 통합 테스트.
//!
//! `#[ignore]`. 실행: docker PG 기동 후 `cargo test -p db -- --ignored`.
//! 픽스처는 바이너리별 disjoint 블록 밴드를 쓰고 끝에 자기 행만 명시 삭제한다
//! (병렬 실행 안전 — cargo-nextest 등).
//!
//! 블록 밴드 맵: 시드 ~18M / alert_rate 97.0M / **labels 97.5M** / alerts 98M /
//! rollback 99M(최상위 — `rollback_from_block`은 ≥N 전역 삭제라 그 파일 전용).

use db::models::{Block, ContractLabel, Transaction};

const DEFAULT_URL: &str = "postgres://defi:defi@localhost:5432/defi_analytics";
const BLOCK: i64 = 97_500_001; // labels 전용 밴드 — 파일 상단 밴드 맵 참조
const TXH_A: &str = "0xc40bec0000000000000000000000000000000000000000000000000000000001";
const TXH_B: &str = "0xc40bec0000000000000000000000000000000000000000000000000000000002";
const LABEL_PUBLIC_ADDR: &str = "0xaabb000000000000000000000000000000000000";
const LABEL_ALICE_ADDR: &str = "0xccdd000000000000000000000000000000000000";

fn db_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn tx_fixture(hash: &str, to: &str) -> Transaction {
    Transaction {
        tx_hash: hash.to_string(),
        from_addr: "0x01".to_string(),
        to_addr: Some(to.to_string()),
        block_number: BLOCK,
        gas_used: 1,
        gas_price: bigdecimal::BigDecimal::from(0),
        value: bigdecimal::BigDecimal::from(0),
        status: 0, // trigger creates failed_transaction(UNKNOWN)
        input_data: None,
    }
}

#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn failed_tx_by_label_pivots_categories_and_filters_by_owner() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");
    let prior = db::queries::get_last_checkpoint(&pool, 1)
        .await
        .expect("read checkpoint");
    let now = chrono::Utc::now();

    // ── labels: one public + one tenant-owned ──
    db::queries::insert_contract_label(&pool, LABEL_PUBLIC_ADDR, "Public test", None)
        .await
        .expect("insert public label");
    db::queries::insert_contract_label(&pool, LABEL_ALICE_ADDR, "Alice test", Some("alice"))
        .await
        .expect("insert alice label");

    // ── fixture block + two failed txs targeting each label ──
    db::queries::insert_blocks(
        &pool,
        &[Block {
            block_number: BLOCK,
            timestamp: now,
            gas_used: 1,
            block_hash: Some("0xc40b".to_string()),
            parent_hash: Some("0xc40a".to_string()),
        }],
    )
    .await
    .expect("insert block");
    db::queries::insert_transactions(
        &pool,
        &[
            tx_fixture(TXH_A, LABEL_PUBLIC_ADDR),
            tx_fixture(TXH_B, LABEL_ALICE_ADDR),
        ],
    )
    .await
    .expect("insert tx");

    // (1) owner=None: both labels appear (plus any pre-existing seed labels —
    //     don't assert exact count; only that ours are present).
    let all = db::queries::failed_tx_by_label_aggregate(&pool, None, None, None, 1000)
        .await
        .expect("aggregate all");
    let public_row = all
        .iter()
        .find(|p| p.address == LABEL_PUBLIC_ADDR)
        .expect("public label in result");
    assert_eq!(public_row.total_failures, 1);
    assert_eq!(public_row.label, "Public test");
    assert_eq!(public_row.by_category.get("UNKNOWN").copied(), Some(1));
    let alice_in_all = all
        .iter()
        .find(|p| p.address == LABEL_ALICE_ADDR)
        .expect("alice label in result (owner=None matches everything)");
    assert_eq!(alice_in_all.total_failures, 1);

    // (2) owner=Some("alice"): only alice's label shows; public is excluded.
    let alice_only =
        db::queries::failed_tx_by_label_aggregate(&pool, Some("alice"), None, None, 1000)
            .await
            .expect("aggregate alice");
    assert!(
        alice_only.iter().all(|p| p.address == LABEL_ALICE_ADDR),
        "owner=alice must only return alice-owned labels"
    );
    assert_eq!(alice_only.len(), 1);
    assert_eq!(alice_only[0].total_failures, 1);

    // (3) owner=Some("nobody"): no matches → empty.
    let nobody = db::queries::failed_tx_by_label_aggregate(&pool, Some("nobody"), None, None, 1000)
        .await
        .expect("aggregate nobody");
    assert_eq!(nobody.len(), 0);

    // (4) future window: from > now → empty regardless of owner.
    let future_from = now + chrono::Duration::days(365);
    let future =
        db::queries::failed_tx_by_label_aggregate(&pool, None, Some(future_from), None, 1000)
            .await
            .expect("aggregate future");
    assert!(
        future
            .iter()
            .all(|p| { p.address != LABEL_PUBLIC_ADDR && p.address != LABEL_ALICE_ADDR }),
        "future window must exclude our just-inserted fixtures"
    );

    // ── teardown ──
    db::queries::delete_contract_label(&pool, LABEL_PUBLIC_ADDR)
        .await
        .expect("delete public label");
    db::queries::delete_contract_label(&pool, LABEL_ALICE_ADDR)
        .await
        .expect("delete alice label");
    // 픽스처는 자기 행만 명시 삭제 — rollback_from_block(BLOCK)은 ≥BLOCK 전역
    // 삭제라 병렬 실행 시 상위 밴드(alerts 98M, rollback 99M) 픽스처까지 지운다.
    // failed_transaction은 트리거가 만든 행이라 FK CASCADE 없음 — 먼저 삭제.
    for h in [TXH_A, TXH_B] {
        sqlx::query("DELETE FROM failed_transaction WHERE tx_hash = $1")
            .bind(h)
            .execute(&pool)
            .await
            .expect("cleanup failed_tx");
        sqlx::query("DELETE FROM transaction WHERE tx_hash = $1")
            .bind(h)
            .execute(&pool)
            .await
            .expect("cleanup tx");
    }
    sqlx::query("DELETE FROM block WHERE block_number = $1")
        .bind(BLOCK)
        .execute(&pool)
        .await
        .expect("cleanup block");
    // 체크포인트 원복 (이 테스트가 만진 게 없지만 안전망 — alert_rate와 동일 관례)
    if let Some(p) = prior {
        db::queries::update_checkpoint(&pool, 1, p)
            .await
            .expect("restore checkpoint");
    }
}

/// S15 (M005) — `upsert_contract_label` is the admin API's insert-or-update
/// primitive. Two calls with the same address overwrite label/owner_id, and
/// the second response carries the new values (not the old).
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn upsert_contract_label_creates_then_overwrites() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");
    let addr = "0xfeedbeef00000000000000000000000000000015";

    // (1) Initial create
    let first: ContractLabel = db::queries::upsert_contract_label(&pool, addr, "First label", None)
        .await
        .expect("upsert ok");
    assert_eq!(first.address, addr);
    assert_eq!(first.label, "First label");
    assert!(first.owner_id.is_none());

    // (2) Same address, different label + owner — UPSERT must overwrite.
    let second: ContractLabel =
        db::queries::upsert_contract_label(&pool, addr, "Second label", Some("alice"))
            .await
            .expect("upsert again ok");
    assert_eq!(second.address, addr);
    assert_eq!(second.label, "Second label");
    assert_eq!(second.owner_id.as_deref(), Some("alice"));

    // teardown
    let deleted = db::queries::delete_contract_label(&pool, addr)
        .await
        .expect("delete");
    assert_eq!(deleted, 1);
}

/// S15 — DELETE idempotency: a second DELETE on the same address returns 0
/// (no row affected). The admin API maps that to HTTP 404.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn delete_contract_label_is_idempotent() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");
    let addr = "0xdead00000000000000000000000000000000d015";

    db::queries::upsert_contract_label(&pool, addr, "Test", None)
        .await
        .expect("upsert");
    let first = db::queries::delete_contract_label(&pool, addr)
        .await
        .expect("delete");
    assert_eq!(first, 1, "first delete removes the row");
    let second = db::queries::delete_contract_label(&pool, addr)
        .await
        .expect("delete twice");
    assert_eq!(second, 0, "second delete is a no-op (handler returns 404)");
}
