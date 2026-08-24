//! Alert 구독/매칭/멱등 통합 테스트 — 실제 PostgreSQL 필요.
//!
//! `#[ignore]`. 실행: docker PG 기동 후 `cargo test -p db -- --ignored`.
//! 픽스처는 바이너리별 disjoint 블록 밴드를 쓰고 끝에 자기 행만 명시 삭제한다
//! (병렬 실행 안전 — cargo-nextest 등).
//!
//! 블록 밴드 맵: 시드 ~18M / alert_rate 97.0M / labels 97.5M / **alerts 98M** /
//! rollback 99M(최상위 — `rollback_from_block`은 ≥N 전역 삭제라 그 파일 전용).

use db::models::{AlertMatch, Block, ErrorCategory, Transaction};

const DEFAULT_URL: &str = "postgres://defi:defi@localhost:5432/defi_analytics";
const BLOCK: i64 = 98_000_001; // alerts 전용 밴드 — 파일 상단 밴드 맵 참조
const TXH: &str = "0xa1e7000000000000000000000000000000000000000000000000000000000001";
const TO: &str = "0x00000000000000000000000000000000000000aa";
/// dispatcher의 운영 기본값과 일치(=60s).
const STALE_AFTER_SECS: i64 = 60;

fn db_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn has_match(v: &[AlertMatch], sub: i64, tx: &str) -> bool {
    v.iter()
        .any(|m| m.subscription_id == sub && m.tx_hash == tx)
}

#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn alert_match_is_idempotent_and_scoped() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    // 신규 alert 테이블 보장(멱등 — 이미 적용분은 sqlx가 스킵)
    db::run_migrations(&pool).await.expect("migrate");
    let now = chrono::Utc::now();
    let prior = db::queries::get_last_checkpoint(&pool, 1)
        .await
        .expect("read checkpoint");

    // ── 픽스처: 높은 분리 블록의 실패 tx 1건 ──
    // `trg_transaction_check_failed` 트리거가 status=0 transaction INSERT 직후
    // `failed_transaction` 행을 **자동 생성**(`error_category='UNKNOWN'`,
    // ON CONFLICT DO NOTHING). FK는 살아 있어 failed_transaction을 transaction
    // 보다 먼저 못 넣는다. → 트리거가 만든 UNKNOWN 행을 그대로 쓴다(매칭
    // 시맨틱 검증 목적엔 충분; sub_match는 `error_category=None`(=any)으로
    // UNKNOWN과 매칭, sub_other는 `DeadlineExpired`로 카테고리 불일치 검증).
    db::queries::insert_blocks(
        &pool,
        &[Block {
            block_number: BLOCK,
            timestamp: now,
            gas_used: 1,
            block_hash: Some("0xa11ce".to_string()),
            parent_hash: Some("0xb0b".to_string()),
        }],
    )
    .await
    .expect("insert block");
    db::queries::insert_transactions(
        &pool,
        &[Transaction {
            tx_hash: TXH.to_string(),
            from_addr: "0x01".to_string(),
            to_addr: Some(TO.to_string()),
            block_number: BLOCK,
            gas_used: 1,
            gas_price: bigdecimal::BigDecimal::from(0),
            value: bigdecimal::BigDecimal::from(0),
            status: 0, // 트리거가 자동으로 failed_transaction(UNKNOWN) 생성
            input_data: None,
        }],
    )
    .await
    .expect("insert tx");

    // ── 구독: 매칭(any-category + to_addr) / 카테고리 불일치 ──
    let sub_match = db::queries::insert_alert_subscription(
        &pool,
        None, // 모든 카테고리 — 트리거가 박은 UNKNOWN을 잡는다
        Some(TO),
        "https://example.test/alerts-itest",
        "secret-itest",
    )
    .await
    .expect("insert sub_match");
    let sub_other = db::queries::insert_alert_subscription(
        &pool,
        Some(&ErrorCategory::DeadlineExpired),
        None,
        "https://example.test/alerts-itest-other",
        "secret2",
    )
    .await
    .expect("insert sub_other");

    let m = db::queries::find_pending_alert_matches(&pool, 1000, STALE_AFTER_SECS)
        .await
        .expect("matches");
    assert!(
        has_match(&m, sub_match.subscription_id, TXH),
        "매칭 구독은 잡혀야"
    );
    assert!(
        !has_match(&m, sub_other.subscription_id, TXH),
        "카테고리 불일치 구독은 안 잡혀야"
    );

    // ── 전송 기록 → anti-join 멱등(다시 안 잡힘) ──
    db::queries::record_alert_delivery(&pool, sub_match.subscription_id, TXH, true, None)
        .await
        .expect("record delivered");
    let m2 = db::queries::find_pending_alert_matches(&pool, 1000, STALE_AFTER_SECS)
        .await
        .expect("matches2");
    assert!(
        !has_match(&m2, sub_match.subscription_id, TXH),
        "전송 완료분은 다시 안 잡혀야(멱등)"
    );
    // 같은 결과 재기록 = 멱등 upsert, 에러 없음
    db::queries::record_alert_delivery(&pool, sub_match.subscription_id, TXH, true, None)
        .await
        .expect("record delivered (idempotent)");

    // ── 실패 기록은 제외하지 않음(재시도 대상) ──
    let sub_retry = db::queries::insert_alert_subscription(
        &pool,
        None,
        Some(TO),
        "https://example.test/alerts-itest-retry",
        "secret3",
    )
    .await
    .expect("insert sub_retry");
    db::queries::record_alert_delivery(
        &pool,
        sub_retry.subscription_id,
        TXH,
        false,
        Some("connection refused"),
    )
    .await
    .expect("record failed");
    let m3 = db::queries::find_pending_alert_matches(&pool, 1000, STALE_AFTER_SECS)
        .await
        .expect("matches3");
    assert!(
        has_match(&m3, sub_retry.subscription_id, TXH),
        "전송 실패분은 계속 잡혀야(재시도)"
    );

    // ── 비활성 구독은 제외 ──
    db::queries::deactivate_alert_subscription(&pool, sub_retry.subscription_id)
        .await
        .expect("deactivate");
    let m4 = db::queries::find_pending_alert_matches(&pool, 1000, STALE_AFTER_SECS)
        .await
        .expect("matches4");
    assert!(
        !has_match(&m4, sub_retry.subscription_id, TXH),
        "비활성 구독은 안 잡혀야"
    );

    // ── teardown (공유 dev DB 위생) ──
    for s in [
        sub_match.subscription_id,
        sub_other.subscription_id,
        sub_retry.subscription_id,
    ] {
        db::queries::delete_alert_subscription(&pool, s)
            .await
            .expect("delete sub");
    }
    // 픽스처는 자기 행만 명시 삭제 — rollback_from_block(BLOCK)은 ≥BLOCK 전역
    // 삭제라 병렬 실행 시 상위 밴드(rollback 99M) 픽스처까지 지운다.
    // failed_transaction은 트리거가 만든 행이라 FK CASCADE 없음 — 먼저 삭제.
    sqlx::query("DELETE FROM failed_transaction WHERE tx_hash = $1")
        .bind(TXH)
        .execute(&pool)
        .await
        .expect("cleanup failed_tx");
    sqlx::query("DELETE FROM transaction WHERE tx_hash = $1")
        .bind(TXH)
        .execute(&pool)
        .await
        .expect("cleanup tx");
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

/// HARDEN-T02: outbox claim의 원자성·재시도·stale-회복 검증.
///
/// `try_claim_alert_match`의 4 시나리오 — new / fresh-claimed 충돌 /
/// stale-claimed 재claim / failed 재claim / delivered는 영구 차단. `find_pending`
/// 자체는 본 테스트와 결합 없이 검증되므로 여기선 claim semantics만 본다.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn alert_claim_is_atomic_and_handles_stale() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    // 시나리오별로 분리된 sub 행 — 한쪽 claim이 다른 쪽에 영향 안 줌.
    let s_new = db::queries::insert_alert_subscription(
        &pool,
        None,
        None,
        "https://example.test/claim-new",
        "secret-cnew",
    )
    .await
    .expect("insert s_new");
    let s_delivered = db::queries::insert_alert_subscription(
        &pool,
        None,
        None,
        "https://example.test/claim-delivered",
        "secret-cdel",
    )
    .await
    .expect("insert s_delivered");
    let s_failed = db::queries::insert_alert_subscription(
        &pool,
        None,
        None,
        "https://example.test/claim-failed",
        "secret-cfail",
    )
    .await
    .expect("insert s_failed");

    const TX: &str = "0xc1a1ec0000000000000000000000000000000000000000000000000000000001";

    // (1) New row: claim → true
    assert!(
        db::queries::try_claim_alert_match(&pool, s_new.subscription_id, TX, STALE_AFTER_SECS)
            .await
            .expect("claim new"),
        "신규 (sub, tx) claim은 true여야"
    );

    // (2) 같은 행 즉시 재시도(fresh): WHERE 미일치로 false
    assert!(
        !db::queries::try_claim_alert_match(&pool, s_new.subscription_id, TX, STALE_AFTER_SECS)
            .await
            .expect("re-claim fresh"),
        "fresh-claimed 재시도는 false여야(다른 워커 진행 중과 동치)"
    );

    // (3) stale_after=0 → 어떤 claimed도 즉시 stale, 재claim true (워커 crash 복구)
    assert!(
        db::queries::try_claim_alert_match(&pool, s_new.subscription_id, TX, 0)
            .await
            .expect("re-claim stale"),
        "stale_after=0이면 fresh-claimed도 재claim 가능(crash 복구)"
    );

    // (4) delivered → 영구 차단 (stale=0 이어도 false)
    db::queries::try_claim_alert_match(&pool, s_delivered.subscription_id, TX, STALE_AFTER_SECS)
        .await
        .expect("initial claim s_delivered");
    db::queries::record_alert_delivery(&pool, s_delivered.subscription_id, TX, true, None)
        .await
        .expect("mark delivered");
    assert!(
        !db::queries::try_claim_alert_match(&pool, s_delivered.subscription_id, TX, 0)
            .await
            .expect("post-delivered claim"),
        "delivered는 stale 무관 영구 차단"
    );

    // (5) failed → 재claim 항상 true (재시도 트리거)
    db::queries::try_claim_alert_match(&pool, s_failed.subscription_id, TX, STALE_AFTER_SECS)
        .await
        .expect("initial claim s_failed");
    db::queries::record_alert_delivery(
        &pool,
        s_failed.subscription_id,
        TX,
        false,
        Some("connection refused"),
    )
    .await
    .expect("mark failed");
    assert!(
        db::queries::try_claim_alert_match(&pool, s_failed.subscription_id, TX, STALE_AFTER_SECS)
            .await
            .expect("post-failed claim"),
        "failed는 fresh stale 무관 재claim(재시도)"
    );

    // teardown — CASCADE가 alert_delivery 정리
    for s in [
        s_new.subscription_id,
        s_delivered.subscription_id,
        s_failed.subscription_id,
    ] {
        db::queries::delete_alert_subscription(&pool, s)
            .await
            .expect("delete sub");
    }
}

/// HARDEN2-T02: `rotate_alert_subscription_secret`의 happy/none/inactive 검증.
#[tokio::test]
#[ignore = "requires PostgreSQL: cargo test -p db -- --ignored"]
async fn alert_secret_rotation_happy_404_inactive() {
    let pool = db::create_pool(&db_url(), 2).await.expect("connect");
    db::run_migrations(&pool).await.expect("migrate");

    let sub = db::queries::insert_alert_subscription(
        &pool,
        None,
        None,
        "https://example.test/rotate-itest",
        "secret-original",
    )
    .await
    .expect("insert sub");

    // (1) happy: 활성 구독 회전 → 새 시크릿이 DB에 박힘
    let rotated = db::queries::rotate_alert_subscription_secret(
        &pool,
        sub.subscription_id,
        "secret-rotated-1",
    )
    .await
    .expect("rotate happy");
    let rotated = rotated.expect("active sub should rotate");
    assert_eq!(rotated.signing_secret, "secret-rotated-1");
    assert_eq!(rotated.subscription_id, sub.subscription_id);
    assert!(rotated.active);

    // (2) 멱등: 같은 시크릿 재호출도 정상 (DB 상태 동일)
    let again = db::queries::rotate_alert_subscription_secret(
        &pool,
        sub.subscription_id,
        "secret-rotated-1",
    )
    .await
    .expect("rotate idempotent");
    assert_eq!(
        again.expect("active sub").signing_secret,
        "secret-rotated-1"
    );

    // (3) 미존재 ID → None (API가 404로 매핑)
    let nope = db::queries::rotate_alert_subscription_secret(&pool, 999_999_999, "x")
        .await
        .expect("rotate nonexistent");
    assert!(nope.is_none(), "missing subscription must rotate to None");

    // (4) 비활성 → None (소프트-삭제된 구독엔 회전 금지)
    db::queries::deactivate_alert_subscription(&pool, sub.subscription_id)
        .await
        .expect("deactivate");
    let inactive = db::queries::rotate_alert_subscription_secret(
        &pool,
        sub.subscription_id,
        "secret-after-deactivate",
    )
    .await
    .expect("rotate inactive");
    assert!(
        inactive.is_none(),
        "inactive subscription must rotate to None"
    );

    // teardown
    db::queries::delete_alert_subscription(&pool, sub.subscription_id)
        .await
        .expect("delete sub");
}
