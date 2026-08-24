use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::error::DbError;
use crate::models::{
    AlertMatch, AlertSubscription, Block, CategoryDiagnosis, ContractLabel, DailySwapVolume,
    ErrorCategory, FailedTransaction, FailedTxAnalysis, FailedTxByLabelPoint, FailedTxTrendPoint,
    FunctionSignature, LiquidityEvent, LiquidityEventType, Pool, PoolStats, PriceSnapshot,
    RateAlertMatch, SnapshotInterval, SwapEvent, TimeBucket, Token, TokenTransfer, TopTrader,
    TraceLog, Transaction, UserProfile,
};

/// PostgreSQL enum 값으로 변환하는 헬퍼.
fn liquidity_type_to_sql(t: &LiquidityEventType) -> &'static str {
    match t {
        LiquidityEventType::Mint => "MINT",
        LiquidityEventType::Burn => "BURN",
    }
}

/// PostgreSQL enum 값으로 변환하는 헬퍼. ErrorCategory의 SCREAMING_SNAKE 와이어
/// 표현과 1:1 매핑되며, S12.1에서 가산된 4 세부 변형도 포함한다 (D028 일관).
fn error_category_to_sql(c: &ErrorCategory) -> &'static str {
    match c {
        ErrorCategory::InsufficientBalance => "INSUFFICIENT_BALANCE",
        ErrorCategory::InsufficientAllowance => "INSUFFICIENT_ALLOWANCE",
        ErrorCategory::SlippageExceeded => "SLIPPAGE_EXCEEDED",
        ErrorCategory::SlippageAmountOut => "SLIPPAGE_AMOUNT_OUT",
        ErrorCategory::SlippageAmountIn => "SLIPPAGE_AMOUNT_IN",
        ErrorCategory::SlippagePriceImpact => "SLIPPAGE_PRICE_IMPACT",
        ErrorCategory::DeadlineExpired => "DEADLINE_EXPIRED",
        ErrorCategory::Unauthorized => "UNAUTHORIZED",
        ErrorCategory::TransferFailed => "TRANSFER_FAILED",
        ErrorCategory::Unknown => "UNKNOWN",
    }
}

/// PostgreSQL enum 값으로 변환하는 헬퍼.
fn snapshot_interval_to_sql(i: &SnapshotInterval) -> &'static str {
    match i {
        SnapshotInterval::OneMinute => "1m",
        SnapshotInterval::FiveMinutes => "5m",
        SnapshotInterval::FifteenMinutes => "15m",
        SnapshotInterval::OneHour => "1h",
        SnapshotInterval::FourHours => "4h",
        SnapshotInterval::OneDay => "1d",
    }
}

/// 블록을 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, blocks))]
pub async fn insert_blocks(pool: &PgPool, blocks: &[Block]) -> Result<u64, DbError> {
    if blocks.is_empty() {
        return Ok(0);
    }

    let block_numbers: Vec<i64> = blocks.iter().map(|b| b.block_number).collect();
    let timestamps: Vec<DateTime<Utc>> = blocks.iter().map(|b| b.timestamp).collect();
    let gas_useds: Vec<i64> = blocks.iter().map(|b| b.gas_used).collect();
    let block_hashes: Vec<Option<&str>> = blocks.iter().map(|b| b.block_hash.as_deref()).collect();
    let parent_hashes: Vec<Option<&str>> =
        blocks.iter().map(|b| b.parent_hash.as_deref()).collect();

    let result = sqlx::query(
        "INSERT INTO block (block_number, timestamp, gas_used, block_hash, parent_hash)
         SELECT * FROM UNNEST($1::BIGINT[], $2::TIMESTAMPTZ[], $3::BIGINT[], $4::TEXT[], $5::TEXT[])
         ON CONFLICT (block_number) DO NOTHING",
    )
    .bind(&block_numbers)
    .bind(&timestamps)
    .bind(&gas_useds)
    .bind(&block_hashes)
    .bind(&parent_hashes)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 트랜잭션을 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, transactions))]
pub async fn insert_transactions(
    pool: &PgPool,
    transactions: &[Transaction],
) -> Result<u64, DbError> {
    if transactions.is_empty() {
        return Ok(0);
    }

    let tx_hashes: Vec<&str> = transactions.iter().map(|t| t.tx_hash.as_str()).collect();
    let from_addrs: Vec<&str> = transactions.iter().map(|t| t.from_addr.as_str()).collect();
    let to_addrs: Vec<Option<&str>> = transactions.iter().map(|t| t.to_addr.as_deref()).collect();
    let block_numbers: Vec<i64> = transactions.iter().map(|t| t.block_number).collect();
    let gas_useds: Vec<i64> = transactions.iter().map(|t| t.gas_used).collect();
    let gas_prices: Vec<&bigdecimal::BigDecimal> =
        transactions.iter().map(|t| &t.gas_price).collect();
    let values: Vec<&bigdecimal::BigDecimal> = transactions.iter().map(|t| &t.value).collect();
    let statuses: Vec<i16> = transactions.iter().map(|t| t.status).collect();
    let input_datas: Vec<Option<&str>> = transactions
        .iter()
        .map(|t| t.input_data.as_deref())
        .collect();

    let result = sqlx::query(
        "INSERT INTO transaction (tx_hash, from_addr, to_addr, block_number, gas_used, gas_price, value, status, input_data)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::BIGINT[], $5::BIGINT[], $6::NUMERIC[], $7::NUMERIC[], $8::SMALLINT[], $9::TEXT[])
         ON CONFLICT (tx_hash) DO NOTHING",
    )
    .bind(&tx_hashes)
    .bind(&from_addrs)
    .bind(&to_addrs)
    .bind(&block_numbers)
    .bind(&gas_useds)
    .bind(&gas_prices)
    .bind(&values)
    .bind(&statuses)
    .bind(&input_datas)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 토큰 메타데이터를 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, tokens))]
pub async fn insert_tokens(pool: &PgPool, tokens: &[Token]) -> Result<u64, DbError> {
    if tokens.is_empty() {
        return Ok(0);
    }

    let addresses: Vec<&str> = tokens.iter().map(|t| t.token_address.as_str()).collect();
    let symbols: Vec<&str> = tokens.iter().map(|t| t.symbol.as_str()).collect();
    let names: Vec<&str> = tokens.iter().map(|t| t.name.as_str()).collect();
    let decimals: Vec<i16> = tokens.iter().map(|t| t.decimals).collect();

    let result = sqlx::query(
        "INSERT INTO token (token_address, symbol, name, decimals)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::SMALLINT[])
         ON CONFLICT (token_address) DO NOTHING",
    )
    .bind(&addresses)
    .bind(&symbols)
    .bind(&names)
    .bind(&decimals)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 풀 정보를 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool_conn, pools))]
pub async fn insert_pools(pool_conn: &PgPool, pools: &[Pool]) -> Result<u64, DbError> {
    if pools.is_empty() {
        return Ok(0);
    }

    let addresses: Vec<&str> = pools.iter().map(|p| p.pool_address.as_str()).collect();
    let pair_names: Vec<&str> = pools.iter().map(|p| p.pair_name.as_str()).collect();
    let token0s: Vec<&str> = pools.iter().map(|p| p.token0_address.as_str()).collect();
    let token1s: Vec<&str> = pools.iter().map(|p| p.token1_address.as_str()).collect();
    let fee_tiers: Vec<i32> = pools.iter().map(|p| p.fee_tier).collect();
    let created_ats: Vec<DateTime<Utc>> = pools.iter().map(|p| p.created_at).collect();

    let result = sqlx::query(
        "INSERT INTO pool (pool_address, pair_name, token0_address, token1_address, fee_tier, created_at)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TEXT[], $5::INT[], $6::TIMESTAMPTZ[])
         ON CONFLICT (pool_address) DO NOTHING",
    )
    .bind(&addresses)
    .bind(&pair_names)
    .bind(&token0s)
    .bind(&token1s)
    .bind(&fee_tiers)
    .bind(&created_ats)
    .execute(pool_conn)
    .await?;

    Ok(result.rows_affected())
}

/// 스왑 이벤트를 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, events))]
pub async fn insert_swap_events(pool: &PgPool, events: &[SwapEvent]) -> Result<u64, DbError> {
    if events.is_empty() {
        return Ok(0);
    }

    let pool_addresses: Vec<&str> = events.iter().map(|e| e.pool_address.as_str()).collect();
    let tx_hashes: Vec<&str> = events.iter().map(|e| e.tx_hash.as_str()).collect();
    let senders: Vec<&str> = events.iter().map(|e| e.sender.as_str()).collect();
    let recipients: Vec<&str> = events.iter().map(|e| e.recipient.as_str()).collect();
    let amount0s: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.amount0).collect();
    let amount1s: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.amount1).collect();
    let amount_ins: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.amount_in).collect();
    let amount_outs: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.amount_out).collect();
    let sqrt_prices: Vec<&bigdecimal::BigDecimal> =
        events.iter().map(|e| &e.sqrt_price_x96).collect();
    let liquidities: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.liquidity).collect();
    let ticks: Vec<i32> = events.iter().map(|e| e.tick).collect();
    let log_indices: Vec<i32> = events.iter().map(|e| e.log_index).collect();
    let timestamps: Vec<DateTime<Utc>> = events.iter().map(|e| e.timestamp).collect();

    let result = sqlx::query(
        "INSERT INTO swap_event (pool_address, tx_hash, sender, recipient, amount0, amount1, amount_in, amount_out, sqrt_price_x96, liquidity, tick, log_index, timestamp)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TEXT[], $5::NUMERIC[], $6::NUMERIC[], $7::NUMERIC[], $8::NUMERIC[], $9::NUMERIC[], $10::NUMERIC[], $11::INT[], $12::INT[], $13::TIMESTAMPTZ[])
         ON CONFLICT (tx_hash, log_index) DO NOTHING",
    )
    .bind(&pool_addresses)
    .bind(&tx_hashes)
    .bind(&senders)
    .bind(&recipients)
    .bind(&amount0s)
    .bind(&amount1s)
    .bind(&amount_ins)
    .bind(&amount_outs)
    .bind(&sqrt_prices)
    .bind(&liquidities)
    .bind(&ticks)
    .bind(&log_indices)
    .bind(&timestamps)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 유동성 이벤트를 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, events))]
pub async fn insert_liquidity_events(
    pool: &PgPool,
    events: &[LiquidityEvent],
) -> Result<u64, DbError> {
    if events.is_empty() {
        return Ok(0);
    }

    let event_types: Vec<&str> = events
        .iter()
        .map(|e| liquidity_type_to_sql(&e.event_type))
        .collect();
    let pool_addresses: Vec<&str> = events.iter().map(|e| e.pool_address.as_str()).collect();
    let tx_hashes: Vec<&str> = events.iter().map(|e| e.tx_hash.as_str()).collect();
    let providers: Vec<&str> = events.iter().map(|e| e.provider.as_str()).collect();
    let token0_amounts: Vec<&bigdecimal::BigDecimal> =
        events.iter().map(|e| &e.token0_amount).collect();
    let token1_amounts: Vec<&bigdecimal::BigDecimal> =
        events.iter().map(|e| &e.token1_amount).collect();
    let tick_lowers: Vec<i32> = events.iter().map(|e| e.tick_lower).collect();
    let tick_uppers: Vec<i32> = events.iter().map(|e| e.tick_upper).collect();
    let liquidities: Vec<&bigdecimal::BigDecimal> = events.iter().map(|e| &e.liquidity).collect();
    let log_indices: Vec<i32> = events.iter().map(|e| e.log_index).collect();
    let timestamps: Vec<DateTime<Utc>> = events.iter().map(|e| e.timestamp).collect();

    let result = sqlx::query(
        "INSERT INTO liquidity_event (event_type, pool_address, tx_hash, provider, token0_amount, token1_amount, tick_lower, tick_upper, liquidity, log_index, timestamp)
         SELECT t.event_type::liquidity_event_type, t.pool_address, t.tx_hash, t.provider, t.token0_amount, t.token1_amount, t.tick_lower, t.tick_upper, t.liquidity, t.log_index, t.ts
         FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TEXT[], $5::NUMERIC[], $6::NUMERIC[], $7::INT[], $8::INT[], $9::NUMERIC[], $10::INT[], $11::TIMESTAMPTZ[])
         AS t(event_type, pool_address, tx_hash, provider, token0_amount, token1_amount, tick_lower, tick_upper, liquidity, log_index, ts)
         ON CONFLICT (tx_hash, log_index) DO NOTHING",
    )
    .bind(&event_types)
    .bind(&pool_addresses)
    .bind(&tx_hashes)
    .bind(&providers)
    .bind(&token0_amounts)
    .bind(&token1_amounts)
    .bind(&tick_lowers)
    .bind(&tick_uppers)
    .bind(&liquidities)
    .bind(&log_indices)
    .bind(&timestamps)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 토큰 전송을 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, transfers))]
pub async fn insert_token_transfers(
    pool: &PgPool,
    transfers: &[TokenTransfer],
) -> Result<u64, DbError> {
    if transfers.is_empty() {
        return Ok(0);
    }

    let tx_hashes: Vec<&str> = transfers.iter().map(|t| t.tx_hash.as_str()).collect();
    let token_addresses: Vec<&str> = transfers.iter().map(|t| t.token_address.as_str()).collect();
    let from_addrs: Vec<&str> = transfers.iter().map(|t| t.from_addr.as_str()).collect();
    let to_addrs: Vec<&str> = transfers.iter().map(|t| t.to_addr.as_str()).collect();
    let amounts: Vec<&bigdecimal::BigDecimal> = transfers.iter().map(|t| &t.amount).collect();
    let log_indices: Vec<i32> = transfers.iter().map(|t| t.log_index).collect();
    let timestamps: Vec<DateTime<Utc>> = transfers.iter().map(|t| t.timestamp).collect();

    let result = sqlx::query(
        "INSERT INTO token_transfer (tx_hash, token_address, from_addr, to_addr, amount, log_index, timestamp)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TEXT[], $5::NUMERIC[], $6::INT[], $7::TIMESTAMPTZ[])
         ON CONFLICT (tx_hash, log_index) DO NOTHING",
    )
    .bind(&tx_hashes)
    .bind(&token_addresses)
    .bind(&from_addrs)
    .bind(&to_addrs)
    .bind(&amounts)
    .bind(&log_indices)
    .bind(&timestamps)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 실패한 트랜잭션을 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, failed))]
pub async fn insert_failed_transactions(
    pool: &PgPool,
    failed: &[FailedTransaction],
) -> Result<u64, DbError> {
    if failed.is_empty() {
        return Ok(0);
    }

    let tx_hashes: Vec<&str> = failed.iter().map(|f| f.tx_hash.as_str()).collect();
    let categories: Vec<&str> = failed
        .iter()
        .map(|f| error_category_to_sql(&f.error_category))
        .collect();
    let revert_reasons: Vec<Option<&str>> =
        failed.iter().map(|f| f.revert_reason.as_deref()).collect();
    let failing_fns: Vec<Option<&str>> = failed
        .iter()
        .map(|f| f.failing_function.as_deref())
        .collect();
    let gas_useds: Vec<i64> = failed.iter().map(|f| f.gas_used).collect();
    let timestamps: Vec<DateTime<Utc>> = failed.iter().map(|f| f.timestamp).collect();

    let result = sqlx::query(
        "INSERT INTO failed_transaction (tx_hash, error_category, revert_reason, failing_function, gas_used, timestamp)
         SELECT t.tx_hash, t.cat::error_category, t.revert_reason, t.failing_fn, t.gas_used, t.ts
         FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TEXT[], $4::TEXT[], $5::BIGINT[], $6::TIMESTAMPTZ[])
         AS t(tx_hash, cat, revert_reason, failing_fn, gas_used, ts)
         ON CONFLICT (tx_hash) DO NOTHING",
    )
    .bind(&tx_hashes)
    .bind(&categories)
    .bind(&revert_reasons)
    .bind(&failing_fns)
    .bind(&gas_useds)
    .bind(&timestamps)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 가격 스냅샷을 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, snapshots))]
pub async fn insert_price_snapshots(
    pool: &PgPool,
    snapshots: &[PriceSnapshot],
) -> Result<u64, DbError> {
    if snapshots.is_empty() {
        return Ok(0);
    }

    let pool_addresses: Vec<&str> = snapshots.iter().map(|s| s.pool_address.as_str()).collect();
    let prices: Vec<&bigdecimal::BigDecimal> = snapshots.iter().map(|s| &s.price).collect();
    let ticks: Vec<i32> = snapshots.iter().map(|s| s.tick).collect();
    let liquidities: Vec<&bigdecimal::BigDecimal> =
        snapshots.iter().map(|s| &s.liquidity).collect();
    let snapshot_tss: Vec<DateTime<Utc>> = snapshots.iter().map(|s| s.snapshot_ts).collect();
    let intervals: Vec<&str> = snapshots
        .iter()
        .map(|s| snapshot_interval_to_sql(&s.interval_type))
        .collect();

    let result = sqlx::query(
        "INSERT INTO price_snapshot (pool_address, price, tick, liquidity, snapshot_ts, interval_type)
         SELECT t.pool_address, t.price, t.tick, t.liquidity, t.snapshot_ts, t.interval::snapshot_interval
         FROM UNNEST($1::TEXT[], $2::NUMERIC[], $3::INT[], $4::NUMERIC[], $5::TIMESTAMPTZ[], $6::TEXT[])
         AS t(pool_address, price, tick, liquidity, snapshot_ts, interval)
         ON CONFLICT (pool_address, snapshot_ts, interval_type) DO NOTHING",
    )
    .bind(&pool_addresses)
    .bind(&prices)
    .bind(&ticks)
    .bind(&liquidities)
    .bind(&snapshot_tss)
    .bind(&intervals)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 유저 프로필을 UNNEST 배치 UPSERT한다.
#[tracing::instrument(skip(pool, profiles))]
pub async fn upsert_user_profiles(pool: &PgPool, profiles: &[UserProfile]) -> Result<u64, DbError> {
    if profiles.is_empty() {
        return Ok(0);
    }

    let addresses: Vec<&str> = profiles.iter().map(|u| u.user_address.as_str()).collect();
    let labels: Vec<Option<&str>> = profiles.iter().map(|u| u.label.as_deref()).collect();
    let first_seens: Vec<DateTime<Utc>> = profiles.iter().map(|u| u.first_seen).collect();
    let last_seens: Vec<DateTime<Utc>> = profiles.iter().map(|u| u.last_seen).collect();
    let total_swaps: Vec<i32> = profiles.iter().map(|u| u.total_swaps).collect();
    let total_volumes: Vec<&bigdecimal::BigDecimal> =
        profiles.iter().map(|u| &u.total_volume_usd).collect();

    let result = sqlx::query(
        "INSERT INTO user_profile (user_address, label, first_seen, last_seen, total_swaps, total_volume_usd)
         SELECT * FROM UNNEST($1::TEXT[], $2::TEXT[], $3::TIMESTAMPTZ[], $4::TIMESTAMPTZ[], $5::INT[], $6::NUMERIC[])
         ON CONFLICT (user_address) DO UPDATE SET
             last_seen = EXCLUDED.last_seen,
             total_swaps = user_profile.total_swaps + EXCLUDED.total_swaps,
             total_volume_usd = user_profile.total_volume_usd + EXCLUDED.total_volume_usd",
    )
    .bind(&addresses)
    .bind(&labels)
    .bind(&first_seens)
    .bind(&last_seens)
    .bind(&total_swaps)
    .bind(&total_volumes)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

/// 트레이스 로그를 UNNEST 배치 INSERT한다.
#[tracing::instrument(skip(pool, traces))]
pub async fn insert_trace_logs(pool: &PgPool, traces: &[TraceLog]) -> Result<u64, DbError> {
    if traces.is_empty() {
        return Ok(0);
    }

    let tx_hashes: Vec<&str> = traces.iter().map(|t| t.tx_hash.as_str()).collect();
    let call_depths: Vec<i32> = traces.iter().map(|t| t.call_depth).collect();
    let call_types: Vec<&str> = traces.iter().map(|t| t.call_type.as_str()).collect();
    let from_addrs: Vec<&str> = traces.iter().map(|t| t.from_addr.as_str()).collect();
    let to_addrs: Vec<Option<&str>> = traces.iter().map(|t| t.to_addr.as_deref()).collect();
    let values: Vec<&bigdecimal::BigDecimal> = traces.iter().map(|t| &t.value).collect();
    let gas_useds: Vec<i64> = traces.iter().map(|t| t.gas_used).collect();
    let inputs: Vec<Option<&str>> = traces.iter().map(|t| t.input.as_deref()).collect();
    let outputs: Vec<Option<&str>> = traces.iter().map(|t| t.output.as_deref()).collect();
    let errors: Vec<Option<&str>> = traces.iter().map(|t| t.error.as_deref()).collect();

    let result = sqlx::query(
        "INSERT INTO trace_log (tx_hash, call_depth, call_type, from_addr, to_addr, value, gas_used, input, output, error)
         SELECT * FROM UNNEST($1::TEXT[], $2::INT[], $3::TEXT[], $4::TEXT[], $5::TEXT[], $6::NUMERIC[], $7::BIGINT[], $8::TEXT[], $9::TEXT[], $10::TEXT[])",
    )
    .bind(&tx_hashes)
    .bind(&call_depths)
    .bind(&call_types)
    .bind(&from_addrs)
    .bind(&to_addrs)
    .bind(&values)
    .bind(&gas_useds)
    .bind(&inputs)
    .bind(&outputs)
    .bind(&errors)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

// ============================================
// API용 읽기 쿼리
// ============================================

/// 풀 목록을 페이지네이션하여 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn list_pools(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<Pool>, DbError> {
    let pools =
        sqlx::query_as::<_, Pool>("SELECT * FROM pool ORDER BY created_at DESC LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
    Ok(pools)
}

/// 주소로 단일 풀을 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_pool_by_address(pool: &PgPool, address: &str) -> Result<Pool, DbError> {
    sqlx::query_as::<_, Pool>("SELECT * FROM pool WHERE pool_address = $1")
        .bind(address)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("pool {address}")))
}

/// 토큰 목록을 페이지네이션하여 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn list_tokens(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<Token>, DbError> {
    let tokens =
        sqlx::query_as::<_, Token>("SELECT * FROM token ORDER BY symbol LIMIT $1 OFFSET $2")
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;
    Ok(tokens)
}

/// 주소로 단일 토큰을 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_token_by_address(pool: &PgPool, address: &str) -> Result<Token, DbError> {
    sqlx::query_as::<_, Token>("SELECT * FROM token WHERE token_address = $1")
        .bind(address)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("token {address}")))
}

/// 스왑 이벤트를 풀 필터 + 페이지네이션으로 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn list_swap_events(
    pool: &PgPool,
    pool_address: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<SwapEvent>, DbError> {
    let events = sqlx::query_as::<_, SwapEvent>(
        "SELECT * FROM swap_event
         WHERE ($1::TEXT IS NULL OR pool_address = $1)
         ORDER BY timestamp DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(pool_address)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(events)
}

/// 일별 스왑 볼륨을 조회한다 (vw_daily_swap_volume).
#[tracing::instrument(skip(pool))]
pub async fn get_daily_swap_volume(
    pool: &PgPool,
    pool_address: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<DailySwapVolume>, DbError> {
    let rows = sqlx::query_as::<_, DailySwapVolume>(
        "SELECT * FROM vw_daily_swap_volume
         WHERE ($1::TEXT IS NULL OR pool_address = $1)
         ORDER BY swap_date DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(pool_address)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 트레이더 랭킹을 조회한다 (vw_top_traders).
#[tracing::instrument(skip(pool))]
pub async fn get_top_traders(pool: &PgPool, limit: i64) -> Result<Vec<TopTrader>, DbError> {
    let traders = sqlx::query_as::<_, TopTrader>("SELECT * FROM vw_top_traders LIMIT $1")
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(traders)
}

/// 실패 TX 카테고리별 분석을 조회한다 (vw_failed_tx_analysis).
#[tracing::instrument(skip(pool))]
pub async fn get_failed_tx_analysis(pool: &PgPool) -> Result<Vec<FailedTxAnalysis>, DbError> {
    let rows = sqlx::query_as::<_, FailedTxAnalysis>("SELECT * FROM vw_failed_tx_analysis")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// tx 해시로 단건 실패 트랜잭션을 조회한다.
///
/// 해당 해시의 실패 기록이 없으면 [`DbError::NotFound`]를 반환한다.
#[tracing::instrument(skip(pool))]
pub async fn get_failed_transaction(
    pool: &PgPool,
    tx_hash: &str,
) -> Result<FailedTransaction, DbError> {
    sqlx::query_as::<_, FailedTransaction>(
        "SELECT tx_hash, error_category, revert_reason, failing_function, gas_used, timestamp
         FROM failed_transaction WHERE tx_hash = $1",
    )
    .bind(tx_hash)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::NotFound(format!("failed transaction {tx_hash}")))
}

/// tx 해시의 평탄화된 콜 트레이스를 호출 순서(pre-order DFS)대로, 최대 `limit`개 조회한다.
///
/// `trace_id`(BIGSERIAL)는 인덱서가 콜트리를 pre-order로 평탄화하며 삽입한
/// 순서를 그대로 보존하므로, `trace_id ASC` = 올바른 트리 선형순서다.
/// `call_depth`로 먼저 정렬하면 형제 서브트리가 섞여 트리 복원이 불가능하다.
/// 호출자는 잘림 감지를 위해 `limit = 상한 + 1`로 조회할 수 있다.
/// 트레이스가 없으면 빈 `Vec`을 반환한다(에러 아님).
#[tracing::instrument(skip(pool))]
pub async fn list_trace_logs_by_tx(
    pool: &PgPool,
    tx_hash: &str,
    limit: i64,
) -> Result<Vec<TraceLog>, DbError> {
    let logs = sqlx::query_as::<_, TraceLog>(
        "SELECT tx_hash, call_depth, call_type, from_addr, to_addr, value,
                gas_used, input, output, error, trace_id
         FROM trace_log WHERE tx_hash = $1
         ORDER BY trace_id ASC
         LIMIT $2",
    )
    .bind(tx_hash)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(logs)
}

/// 트랜잭션의 **첫 error frame**을 반환한다 — 콜트리에서 실제로 revert가 발생한 위치 (S10 / M004).
///
/// `trace_id ASC` = 인덱서가 pre-order DFS로 평탄화하며 삽입한 순서이므로,
/// `error IS NOT NULL`인 가장 빠른 1행이 *맨 처음 발생한 revert frame*이다.
/// 매칭 frame 없음 / tx 없음 모두 `None`을 반환한다(에러 아님). API는 명시
/// `null`로 직렬화 — silent default 금지.
#[tracing::instrument(skip(pool))]
pub async fn get_first_error_frame(
    pool: &PgPool,
    tx_hash: &str,
) -> Result<Option<TraceLog>, DbError> {
    let row = sqlx::query_as::<_, TraceLog>(
        "SELECT tx_hash, call_depth, call_type, from_addr, to_addr, value,
                gas_used, input, output, error, trace_id
         FROM trace_log
         WHERE tx_hash = $1 AND error IS NOT NULL
         ORDER BY trace_id ASC
         LIMIT 1",
    )
    .bind(tx_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 실패 트랜잭션을 필터·페이지네이션하여 조회한다.
///
/// `category`/`from`/`to`는 모두 선택 — `None`이면 해당 필터를 적용하지 않는다
/// (단일 prepared statement, 동적 SQL 미사용). `timestamp DESC` 정렬.
/// 전체 건수는 [`count_failed_transactions`]로 별도 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn list_failed_transactions(
    pool: &PgPool,
    category: Option<&ErrorCategory>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> Result<Vec<FailedTransaction>, DbError> {
    let cat = category.map(error_category_to_sql);
    let rows = sqlx::query_as::<_, FailedTransaction>(
        "SELECT tx_hash, error_category, revert_reason, failing_function, gas_used, timestamp
         FROM failed_transaction
         WHERE ($1::TEXT IS NULL OR error_category = $1::error_category)
           AND ($2::TIMESTAMPTZ IS NULL OR timestamp >= $2)
           AND ($3::TIMESTAMPTZ IS NULL OR timestamp <= $3)
         ORDER BY timestamp DESC
         LIMIT $4 OFFSET $5",
    )
    .bind(cat)
    .bind(from)
    .bind(to)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// [`list_failed_transactions`]와 동일 필터의 전체 건수를 반환한다.
///
/// 페이지네이션 `total` 메타데이터 산출용 — `LIMIT`/`OFFSET`과 무관하다.
#[tracing::instrument(skip(pool))]
pub async fn count_failed_transactions(
    pool: &PgPool,
    category: Option<&ErrorCategory>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
) -> Result<i64, DbError> {
    let cat = category.map(error_category_to_sql);
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM failed_transaction
         WHERE ($1::TEXT IS NULL OR error_category = $1::error_category)
           AND ($2::TIMESTAMPTZ IS NULL OR timestamp >= $2)
           AND ($3::TIMESTAMPTZ IS NULL OR timestamp <= $3)",
    )
    .bind(cat)
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;
    Ok(total)
}

/// 실패 트랜잭션을 시간 버킷 × 카테고리로 집계한다.
///
/// `bucket`은 화이트리스트 [`TimeBucket`]만 받아 `date_trunc($1, timestamp)`에
/// **바인딩 파라미터**로 전달한다 (문자열 보간 금지 — SQL 인젝션 방지).
/// `from`/`to`는 선택. `bucket ASC, error_category ASC` 정렬.
#[tracing::instrument(skip(pool))]
pub async fn failed_tx_timeseries(
    pool: &PgPool,
    bucket: &TimeBucket,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
) -> Result<Vec<FailedTxTrendPoint>, DbError> {
    let rows = sqlx::query_as::<_, FailedTxTrendPoint>(
        "SELECT date_trunc($1, timestamp) AS bucket,
                error_category,
                COUNT(*) AS failure_count
         FROM failed_transaction
         WHERE ($2::TIMESTAMPTZ IS NULL OR timestamp >= $2)
           AND ($3::TIMESTAMPTZ IS NULL OR timestamp <= $3)
         GROUP BY 1, 2
         ORDER BY 1 ASC, 2 ASC",
    )
    .bind(bucket.as_pg())
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 풀 종합 통계를 조회한다 (fn_get_pool_stats).
#[tracing::instrument(skip(pool))]
pub async fn get_pool_stats(
    pool: &PgPool,
    pool_address: &str,
    from_date: DateTime<Utc>,
    to_date: DateTime<Utc>,
) -> Result<PoolStats, DbError> {
    sqlx::query_as::<_, PoolStats>("SELECT * FROM fn_get_pool_stats($1, $2, $3)")
        .bind(pool_address)
        .bind(from_date)
        .bind(to_date)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("pool stats for {pool_address}")))
}

// ============================================
// 인덱서용 체크포인트 쿼리
// ============================================

/// 특정 체인의 마지막 체크포인트를 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_last_checkpoint(pool: &PgPool, chain_id: i32) -> Result<Option<i64>, DbError> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT last_processed_block FROM indexer_checkpoint WHERE chain_id = $1")
            .bind(chain_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

/// 체크포인트를 갱신한다 (없으면 INSERT, 있으면 UPDATE).
#[tracing::instrument(skip(pool))]
pub async fn update_checkpoint(
    pool: &PgPool,
    chain_id: i32,
    last_processed_block: i64,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO indexer_checkpoint (chain_id, last_processed_block, updated_at)
         VALUES ($1, $2, NOW())
         ON CONFLICT (chain_id) DO UPDATE
         SET last_processed_block = EXCLUDED.last_processed_block,
             updated_at = NOW()",
    )
    .bind(chain_id)
    .bind(last_processed_block)
    .execute(pool)
    .await?;
    Ok(())
}

/// 블록 번호로 블록을 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_block_by_number(pool: &PgPool, block_number: i64) -> Result<Block, DbError> {
    sqlx::query_as::<_, Block>("SELECT * FROM block WHERE block_number = $1")
        .bind(block_number)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("block {block_number}")))
}

/// DB에 저장된 가장 최근 블록 번호를 반환한다.
#[tracing::instrument(skip(pool))]
pub async fn get_latest_block_number(pool: &PgPool) -> Result<Option<i64>, DbError> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT MAX(block_number) FROM block")
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

/// 최근 인덱싱된 블록의 `(번호, 해시)`를 번호 내림차순으로 최대 `limit`개 조회한다.
///
/// 해시가 없는 행(S06 이전 인덱싱)은 비교 불가이므로 제외한다.
/// reorg 감지(`detect_fork` → 순수 `classify_fork`)의 로컬 입력용.
#[tracing::instrument(skip(pool))]
pub async fn recent_block_hashes(pool: &PgPool, limit: i64) -> Result<Vec<(i64, String)>, DbError> {
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT block_number, block_hash
         FROM block
         WHERE block_hash IS NOT NULL
         ORDER BY block_number DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// reorg 정정: `from_block`부터(포함) 인덱싱 데이터를 모두 삭제하고 체크포인트를
/// 되감는다. 단일 트랜잭션·멱등.
///
/// tx_hash FK가 `ON DELETE CASCADE`가 아니므로 의존 행 → `transaction` → `block`
/// 순서로 삭제한다. `price_snapshot`/`user_profile`은 블록 스코프가 아니라 제외.
/// 재인덱싱은 하지 않는다(follow가 체크포인트에서 재개). 삭제된 block 수를 반환.
#[tracing::instrument(skip(pool))]
pub async fn rollback_from_block(pool: &PgPool, from_block: i64) -> Result<u64, DbError> {
    let resume = (from_block - 1).max(0);
    let mut tx = pool.begin().await?;

    // 1) tx_hash 의존 행 (transaction 삭제 전 — FK RESTRICT)
    sqlx::query(
        "DELETE FROM swap_event
         WHERE tx_hash IN (SELECT tx_hash FROM transaction WHERE block_number >= $1)",
    )
    .bind(from_block)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM liquidity_event
         WHERE tx_hash IN (SELECT tx_hash FROM transaction WHERE block_number >= $1)",
    )
    .bind(from_block)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM token_transfer
         WHERE tx_hash IN (SELECT tx_hash FROM transaction WHERE block_number >= $1)",
    )
    .bind(from_block)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM trace_log
         WHERE tx_hash IN (SELECT tx_hash FROM transaction WHERE block_number >= $1)",
    )
    .bind(from_block)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM failed_transaction
         WHERE tx_hash IN (SELECT tx_hash FROM transaction WHERE block_number >= $1)",
    )
    .bind(from_block)
    .execute(&mut *tx)
    .await?;

    // 2) transaction → block
    sqlx::query("DELETE FROM transaction WHERE block_number >= $1")
        .bind(from_block)
        .execute(&mut *tx)
        .await?;
    let blocks = sqlx::query("DELETE FROM block WHERE block_number >= $1")
        .bind(from_block)
        .execute(&mut *tx)
        .await?
        .rows_affected();

    // 3) 체크포인트 되감기 (follow가 from_block부터 재인덱싱)
    sqlx::query(
        "UPDATE indexer_checkpoint
         SET last_processed_block = $1, updated_at = NOW()
         WHERE chain_id = 1",
    )
    .bind(resume)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(blocks)
}

/// 특정 블록의 스왑 이벤트를 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn get_swap_events_by_block(
    pool: &PgPool,
    block_number: i64,
) -> Result<Vec<SwapEvent>, DbError> {
    let events = sqlx::query_as::<_, SwapEvent>(
        "SELECT se.* FROM swap_event se
         JOIN transaction t ON se.tx_hash = t.tx_hash
         WHERE t.block_number = $1
         ORDER BY se.event_id",
    )
    .bind(block_number)
    .fetch_all(pool)
    .await?;
    Ok(events)
}

// ============================================
// Actionable Alerts (S08) — 구독/매칭/전송 기록
// ============================================

/// 실패 패턴 구독을 `per_event` 모드로 생성하고 생성된 행을 반환한다.
///
/// `error_category`는 enum 텍스트 바인딩(`$1::error_category`)으로 안전 전달.
/// `webhook_url`/`signing_secret`은 호출자(API)가 검증·생성해 넘긴다.
/// rate_threshold 모드는 [`insert_alert_subscription_rate`] (S14/M005) 참조.
#[tracing::instrument(skip(pool, signing_secret))]
pub async fn insert_alert_subscription(
    pool: &PgPool,
    error_category: Option<&ErrorCategory>,
    to_addr: Option<&str>,
    webhook_url: &str,
    signing_secret: &str,
) -> Result<AlertSubscription, DbError> {
    let cat = error_category.map(error_category_to_sql);
    let row = sqlx::query_as::<_, AlertSubscription>(
        "INSERT INTO alert_subscription
             (error_category, to_addr, webhook_url, signing_secret, sub_type)
         VALUES ($1::error_category, $2, $3, $4, 'per_event')
         RETURNING subscription_id, error_category, to_addr, webhook_url,
                   signing_secret, active, created_at,
                   sub_type, threshold_count, threshold_window_secs, debounce_secs",
    )
    .bind(cat)
    .bind(to_addr)
    .bind(webhook_url)
    .bind(signing_secret)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// 구독 목록을 최신순으로 최대 `limit`개 조회한다.
#[tracing::instrument(skip(pool))]
pub async fn list_alert_subscriptions(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<AlertSubscription>, DbError> {
    let rows = sqlx::query_as::<_, AlertSubscription>(
        "SELECT subscription_id, error_category, to_addr, webhook_url,
                signing_secret, active, created_at,
                sub_type, threshold_count, threshold_window_secs, debounce_secs
         FROM alert_subscription
         ORDER BY subscription_id DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 구독을 비활성화한다. 영향받은 행 수를 반환한다(0 = 미존재/이미 비활성).
#[tracing::instrument(skip(pool))]
pub async fn deactivate_alert_subscription(
    pool: &PgPool,
    subscription_id: i64,
) -> Result<u64, DbError> {
    let res = sqlx::query(
        "UPDATE alert_subscription SET active = FALSE
         WHERE subscription_id = $1 AND active = TRUE",
    )
    .bind(subscription_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// 아직 전송되지 않은 (활성 구독 × 매칭 실패 tx) 쌍을 최대 `limit`개 반환한다.
///
/// 매칭: `error_category`/`to_addr`가 NULL이면 해당 조건은 "모두 매칭".
/// **멱등 anti-join**: `alert_delivery`에서
/// (a) `status='delivered'` — 영구 제외, 또는
/// (b) `status='claimed'` 이고 `created_at`이 `stale_after_secs` 이내 — 다른
///     워커가 현재 처리 중이라 제외(HARDEN-T02). `stale_after_secs`가 지나면
///     워커가 죽은 것으로 간주, 다시 매칭에 포함 → 재claim 가능.
/// `status='failed'`는 항상 포함(재시도).
#[tracing::instrument(skip(pool))]
pub async fn find_pending_alert_matches(
    pool: &PgPool,
    limit: i64,
    stale_after_secs: i64,
) -> Result<Vec<AlertMatch>, DbError> {
    let rows = sqlx::query_as::<_, AlertMatch>(
        "SELECT s.subscription_id, f.tx_hash, s.webhook_url, s.signing_secret
         FROM alert_subscription s
         JOIN failed_transaction f
           ON (s.error_category IS NULL OR s.error_category = f.error_category)
         LEFT JOIN transaction t ON t.tx_hash = f.tx_hash
         WHERE s.active
           AND s.sub_type = 'per_event'
           AND (s.to_addr IS NULL OR s.to_addr = t.to_addr)
           AND NOT EXISTS (
                 SELECT 1 FROM alert_delivery d
                 WHERE d.subscription_id = s.subscription_id
                   AND d.tx_hash = f.tx_hash
                   AND (d.status = 'delivered'
                        OR (d.status = 'claimed'
                            AND d.created_at > NOW() - ($2::int * INTERVAL '1 second')))
               )
         ORDER BY f.tx_hash
         LIMIT $1",
    )
    .bind(limit)
    .bind(stale_after_secs)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 전송 결과를 멱등 기록한다 (PK `(subscription_id, tx_hash)` upsert).
///
/// 재시도 시 `attempts`가 누적되고, `delivered`면 `delivered_at`이 한 번 박힌다.
/// 이 멱등 기록 + [`find_pending_alert_matches`]의 anti-join이 "정확히 1회 전송"을
/// 보장한다.
#[tracing::instrument(skip(pool))]
pub async fn record_alert_delivery(
    pool: &PgPool,
    subscription_id: i64,
    tx_hash: &str,
    delivered: bool,
    last_error: Option<&str>,
) -> Result<(), DbError> {
    let status = if delivered { "delivered" } else { "failed" };
    sqlx::query(
        "INSERT INTO alert_delivery
             (subscription_id, tx_hash, status, attempts, last_error, delivered_at)
         VALUES ($1, $2, $3, 1, $4,
                 CASE WHEN $3 = 'delivered' THEN NOW() ELSE NULL END)
         ON CONFLICT (subscription_id, tx_hash) DO UPDATE
             SET status = EXCLUDED.status,
                 attempts = alert_delivery.attempts + 1,
                 last_error = EXCLUDED.last_error,
                 delivered_at = CASE
                     WHEN EXCLUDED.status = 'delivered' THEN NOW()
                     ELSE alert_delivery.delivered_at END",
    )
    .bind(subscription_id)
    .bind(tx_hash)
    .bind(status)
    .bind(last_error)
    .execute(pool)
    .await?;
    Ok(())
}

/// 구독을 영구 삭제한다(연관 `alert_delivery`는 FK `ON DELETE CASCADE`로 함께
/// 삭제). 영향받은 행 수를 반환한다. 보존정책상 완전 삭제·테스트 teardown용.
#[tracing::instrument(skip(pool))]
pub async fn delete_alert_subscription(
    pool: &PgPool,
    subscription_id: i64,
) -> Result<u64, DbError> {
    let res = sqlx::query("DELETE FROM alert_subscription WHERE subscription_id = $1")
        .bind(subscription_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

/// 활성 구독의 `signing_secret`을 회전한다(HARDEN2-T02). 호출자가 새 시크릿을
/// 생성해 넘긴다(API는 CSPRNG 32B hex). 미존재/비활성 구독 → `None` (API는
/// 404로 매핑). 멱등: 같은 시크릿 재호출도 동일 결과(`active=TRUE` 조건만 만족).
#[tracing::instrument(skip(pool, new_secret))]
pub async fn rotate_alert_subscription_secret(
    pool: &PgPool,
    subscription_id: i64,
    new_secret: &str,
) -> Result<Option<AlertSubscription>, DbError> {
    let row = sqlx::query_as::<_, AlertSubscription>(
        "UPDATE alert_subscription
            SET signing_secret = $2
          WHERE subscription_id = $1 AND active = TRUE
        RETURNING subscription_id, error_category, to_addr, webhook_url,
                  signing_secret, active, created_at,
                  sub_type, threshold_count, threshold_window_secs, debounce_secs",
    )
    .bind(subscription_id)
    .bind(new_secret)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

// ============================================
// Rate-threshold alerts (S14 / M005) — 임계율 집계 알림
// ============================================

/// `rate_threshold` 모드 구독을 생성하고 생성된 행을 반환한다 (S14/M005).
///
/// CHECK 제약(`alert_subscription_rate_fields_chk`)으로 threshold_count > 0,
/// threshold_window_secs > 0, debounce_secs >= 0 자동 검증 — 잘못된 값은
/// Postgres가 거부(API 계층이 사전 검증으로 400 매핑).
#[tracing::instrument(skip(pool, signing_secret))]
#[allow(clippy::too_many_arguments)]
pub async fn insert_alert_subscription_rate(
    pool: &PgPool,
    error_category: Option<&ErrorCategory>,
    to_addr: Option<&str>,
    webhook_url: &str,
    signing_secret: &str,
    threshold_count: i32,
    threshold_window_secs: i32,
    debounce_secs: i32,
) -> Result<AlertSubscription, DbError> {
    let cat = error_category.map(error_category_to_sql);
    let row = sqlx::query_as::<_, AlertSubscription>(
        "INSERT INTO alert_subscription
             (error_category, to_addr, webhook_url, signing_secret,
              sub_type, threshold_count, threshold_window_secs, debounce_secs)
         VALUES ($1::error_category, $2, $3, $4, 'rate_threshold', $5, $6, $7)
         RETURNING subscription_id, error_category, to_addr, webhook_url,
                   signing_secret, active, created_at,
                   sub_type, threshold_count, threshold_window_secs, debounce_secs",
    )
    .bind(cat)
    .bind(to_addr)
    .bind(webhook_url)
    .bind(signing_secret)
    .bind(threshold_count)
    .bind(threshold_window_secs)
    .bind(debounce_secs)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// 발송 자격 있는 rate_threshold 매칭을 최대 `limit`개 반환한다 (S14/M005).
///
/// 매칭 조건:
/// 1. sub.active AND sub_type='rate_threshold'
/// 2. category/to_addr 매칭 (NULL = 모두 매칭, S08 동일)
/// 3. f.timestamp >= NOW() - threshold_window_secs (시간 윈도우)
/// 4. NOT EXISTS (alert_rate_dispatch 중 debounce_secs 안에 발송된 행) — 디바운스
/// 5. COUNT(matches) >= threshold_count
///
/// 디바운스 검증이 SQL 안에서 race-safe하게 수행 — 두 worker가 같은 sub을 동시에
/// 매칭해도 *둘 다* INSERT 가능하지만, 다음 매칭 시점부터는 마지막 INSERT 시각
/// 기준으로 debounce_secs 동안 걸러짐. 1-2회의 짧은 race overlap만 허용.
#[tracing::instrument(skip(pool))]
pub async fn find_pending_rate_alert_matches(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<RateAlertMatch>, DbError> {
    let rows = sqlx::query_as::<_, RateAlertMatch>(
        "SELECT s.subscription_id, s.webhook_url, s.signing_secret,
                COUNT(f.tx_hash)::BIGINT AS match_count,
                s.threshold_count, s.threshold_window_secs
         FROM alert_subscription s
         JOIN failed_transaction f
           ON (s.error_category IS NULL OR s.error_category = f.error_category)
         LEFT JOIN transaction t ON t.tx_hash = f.tx_hash
         WHERE s.active
           AND s.sub_type = 'rate_threshold'
           AND (s.to_addr IS NULL OR s.to_addr = t.to_addr)
           AND f.timestamp >= NOW() - (s.threshold_window_secs * INTERVAL '1 second')
           AND NOT EXISTS (
             SELECT 1 FROM alert_rate_dispatch d
             WHERE d.subscription_id = s.subscription_id
               AND d.dispatched_at > NOW() - (s.debounce_secs * INTERVAL '1 second')
           )
         GROUP BY s.subscription_id, s.webhook_url, s.signing_secret,
                  s.threshold_count, s.threshold_window_secs
         HAVING COUNT(f.tx_hash) >= s.threshold_count
         ORDER BY s.subscription_id
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// rate_threshold 발송 결과를 기록한다 (디바운스 검증의 *마지막 발송 시각* 출처).
///
/// `match_count`는 발송 시점의 윈도우 매칭 수(payload에도 동일 값 포함).
/// `last_error`는 발송 실패 시 redacted 에러 메시지.
#[tracing::instrument(skip(pool, last_error))]
pub async fn record_rate_alert_dispatch(
    pool: &PgPool,
    subscription_id: i64,
    match_count: i32,
    delivered: bool,
    last_error: Option<&str>,
) -> Result<(), DbError> {
    let status = if delivered { "delivered" } else { "failed" };
    sqlx::query(
        "INSERT INTO alert_rate_dispatch
             (subscription_id, match_count, status, last_error)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(subscription_id)
    .bind(match_count)
    .bind(status)
    .bind(last_error)
    .execute(pool)
    .await?;
    Ok(())
}

// ============================================
// Contract labels (S09 / M003) — 온체인 × 비공개 라벨 조인
// ============================================

/// 컨트랙트 라벨을 등록한다(멱등 — 이미 있으면 no-op). `address`는 lowercased.
#[tracing::instrument(skip(pool))]
pub async fn insert_contract_label(
    pool: &PgPool,
    address: &str,
    label: &str,
    owner_id: Option<&str>,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO contract_label (address, label, owner_id)
         VALUES (LOWER($1), $2, $3)
         ON CONFLICT (address) DO NOTHING",
    )
    .bind(address)
    .bind(label)
    .bind(owner_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// 컨트랙트 라벨을 등록·갱신해 행을 반환한다 (S15 / M005 admin API).
///
/// `address` 충돌 시 `label`/`owner_id`를 EXCLUDED 값으로 덮어쓴다(UPSERT).
/// `insert_contract_label`이 ON CONFLICT DO NOTHING으로 시드 멱등에 쓰이는 반면,
/// 본 함수는 admin API의 "create or update" 시맨틱을 한 호출에 — 응답에 항상
/// 행을 돌려준다.
#[tracing::instrument(skip(pool))]
pub async fn upsert_contract_label(
    pool: &PgPool,
    address: &str,
    label: &str,
    owner_id: Option<&str>,
) -> Result<ContractLabel, DbError> {
    let row = sqlx::query_as::<_, ContractLabel>(
        "INSERT INTO contract_label (address, label, owner_id)
         VALUES (LOWER($1), $2, $3)
         ON CONFLICT (address) DO UPDATE
           SET label = EXCLUDED.label, owner_id = EXCLUDED.owner_id
         RETURNING address, label, owner_id, created_at",
    )
    .bind(address)
    .bind(label)
    .bind(owner_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// 컨트랙트 라벨을 영구 삭제한다(admin/test 용도). 영향 행 수를 반환한다.
#[tracing::instrument(skip(pool))]
pub async fn delete_contract_label(pool: &PgPool, address: &str) -> Result<u64, DbError> {
    let res = sqlx::query("DELETE FROM contract_label WHERE address = LOWER($1)")
        .bind(address)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

/// 등록된 라벨 목록(최신순) — admin/디버그용. `owner_id`로 필터.
#[tracing::instrument(skip(pool))]
pub async fn list_contract_labels(
    pool: &PgPool,
    owner_id: Option<&str>,
    limit: i64,
) -> Result<Vec<ContractLabel>, DbError> {
    let rows = sqlx::query_as::<_, ContractLabel>(
        "SELECT address, label, owner_id, created_at
         FROM contract_label
         WHERE ($1::TEXT IS NULL OR owner_id = $1)
         ORDER BY created_at DESC
         LIMIT $2",
    )
    .bind(owner_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 라벨된 컨트랙트별 실패 분포(S09 / M003).
///
/// `failed_transaction × transaction × contract_label` 조인 후 (라벨, 주소,
/// 카테고리) 그룹 카운트를 받아 Rust에서 (라벨, 주소)별로 카테고리 맵으로
/// **피벗**한다(sqlx `json` 피처 무도입을 위해). owner_id 필터로 공개(NULL)/
/// 테넌트 분리 가능. 시간 필터·limit 적용. 결과는 `total_failures` 내림차순.
#[tracing::instrument(skip(pool))]
pub async fn failed_tx_by_label_aggregate(
    pool: &PgPool,
    owner_id: Option<&str>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    limit: i64,
) -> Result<Vec<FailedTxByLabelPoint>, DbError> {
    use std::collections::HashMap;

    #[derive(sqlx::FromRow)]
    struct RawRow {
        label: String,
        address: String,
        error_category: String,
        cnt: i64,
    }

    let raw = sqlx::query_as::<_, RawRow>(
        "SELECT cl.label,
                cl.address,
                f.error_category::TEXT AS error_category,
                COUNT(*)::BIGINT AS cnt
         FROM contract_label cl
         JOIN transaction t ON t.to_addr = cl.address
         JOIN failed_transaction f ON f.tx_hash = t.tx_hash
         WHERE ($1::TEXT IS NULL OR cl.owner_id = $1)
           AND ($2::TIMESTAMPTZ IS NULL OR f.timestamp >= $2)
           AND ($3::TIMESTAMPTZ IS NULL OR f.timestamp <= $3)
         GROUP BY cl.label, cl.address, f.error_category",
    )
    .bind(owner_id)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let mut groups: HashMap<(String, String), (i64, HashMap<String, i64>)> = HashMap::new();
    for r in raw {
        let key = (r.label, r.address);
        let entry = groups.entry(key).or_insert_with(|| (0i64, HashMap::new()));
        entry.0 += r.cnt;
        entry.1.insert(r.error_category, r.cnt);
    }

    let mut out: Vec<FailedTxByLabelPoint> = groups
        .into_iter()
        .map(|((label, address), (total, by_cat))| FailedTxByLabelPoint {
            label,
            address,
            total_failures: total,
            by_category: by_cat,
        })
        .collect();

    out.sort_by_key(|p| std::cmp::Reverse(p.total_failures));
    if limit > 0 {
        out.truncate(limit as usize);
    }
    Ok(out)
}

// ============================================
// Category diagnosis (S12 / M004) — error_category → message + recommended_action
// ============================================

/// `error_category` (SCREAMING_SNAKE wire form) → 자기소유 진단 시드 lookup (S12 / M004).
///
/// `error_category_wire`는 `"SLIPPAGE_EXCEEDED"` 등 (`ErrorCategory` enum의 wire form).
/// 정확 매칭. 미존재 → `None` — silent default 금지(호출자가 명시 `null`로 직렬화, D014).
#[tracing::instrument(skip(pool))]
pub async fn get_category_diagnosis(
    pool: &PgPool,
    error_category_wire: &str,
) -> Result<Option<CategoryDiagnosis>, DbError> {
    let row = sqlx::query_as::<_, CategoryDiagnosis>(
        "SELECT error_category, message, recommended_action, source, created_at
         FROM category_diagnosis
         WHERE error_category = $1
         LIMIT 1",
    )
    .bind(error_category_wire)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

// ============================================
// Function signature decoding (S11 / M004) — selector → name/signature
// ============================================

/// 4-byte function selector로 자기소유 ABI 시드를 lookup한다 (S11 / M004).
///
/// `selector`는 `0x` + 8 hex (예: `0xa9059cbb`). 대소문자 무관(`LOWER($1)` lookup).
/// 매칭 없으면 `None` — silent default 금지(호출자가 명시 `null`로 직렬화, D014).
#[tracing::instrument(skip(pool))]
pub async fn get_function_signature(
    pool: &PgPool,
    selector: &str,
) -> Result<Option<FunctionSignature>, DbError> {
    let row = sqlx::query_as::<_, FunctionSignature>(
        "SELECT selector, name, signature, source, created_at
         FROM function_signature
         WHERE selector = LOWER($1)
         LIMIT 1",
    )
    .bind(selector)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 한 (구독 × tx) 쌍에 대한 전송 권리를 **원자적으로 claim** 한다 (HARDEN-T02).
///
/// `INSERT … ON CONFLICT DO UPDATE WHERE`로 락 없이 한 문장에서 보장한다:
/// - 새로운 쌍 → 새 'claimed' 행 INSERT → `true`
/// - 기존 `status='failed'` → 'claimed'로 다시 매겨 재시도 → `true`
/// - 기존 `status='claimed'` 이고 `created_at < NOW() - stale_after_secs`
///   (워커가 잡고서 죽은 상황) → 'claimed'로 갱신 → `true`
/// - 기존 `status='delivered'` 또는 fresh `status='claimed'` (다른 워커 진행 중)
///   → WHERE 미일치 → 0행 영향 → `false`
///
/// PK `(subscription_id, tx_hash)`가 원자성 보장 — 두 워커가 동시에 같은 쌍을
/// claim 시도해도 정확히 한쪽만 `true`를 받는다. HTTP POST는 락 밖에서 수행.
#[tracing::instrument(skip(pool))]
pub async fn try_claim_alert_match(
    pool: &PgPool,
    subscription_id: i64,
    tx_hash: &str,
    stale_after_secs: i64,
) -> Result<bool, DbError> {
    let res = sqlx::query(
        "INSERT INTO alert_delivery
             (subscription_id, tx_hash, status, attempts, last_error, delivered_at, created_at)
         VALUES ($1, $2, 'claimed', 1, NULL, NULL, NOW())
         ON CONFLICT (subscription_id, tx_hash) DO UPDATE
             SET status = 'claimed',
                 attempts = alert_delivery.attempts + 1,
                 last_error = NULL,
                 delivered_at = NULL,
                 created_at = NOW()
             WHERE alert_delivery.status = 'failed'
                OR (alert_delivery.status = 'claimed'
                    AND alert_delivery.created_at < NOW() - ($3::int * INTERVAL '1 second'))",
    )
    .bind(subscription_id)
    .bind(tx_hash)
    .bind(stale_after_secs)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() == 1)
}
