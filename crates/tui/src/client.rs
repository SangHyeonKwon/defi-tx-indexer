//! amarillo REST API HTTP 클라이언트.
//!
//! `reqwest`로 axum API의 읽기 전용 엔드포인트를 호출하고 [`crate::dto`] 타입으로
//! 역직렬화한다. DB에 직접 붙지 않으므로 `AMARILLO_API_URL`만 알면 동작한다.

use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::config::TuiConfig;
use crate::dto::{
    ApiErrorBody, ApiResponse, FailedTransaction, FailedTxAnalysis, FailedTxDetail, TotalPaginated,
    UnknownRevertCluster,
};
use crate::error::TuiError;

/// API 클라이언트 — `reqwest::Client` + 베이스 URL + 선택적 admin key.
#[derive(Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    base_url: String,
    admin_api_key: Option<String>,
}

impl ApiClient {
    /// 설정에서 클라이언트를 만든다 (타임아웃 적용).
    pub fn new(cfg: &TuiConfig) -> Result<Self, TuiError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.request_timeout_secs))
            .build()?;
        Ok(Self {
            http,
            base_url: cfg.api_url.trim_end_matches('/').to_string(),
            admin_api_key: cfg.admin_api_key.clone(),
        })
    }

    /// 공통 GET 헬퍼 — 쿼리 빌드, (선택) Bearer 인증, 상태 검사, 역직렬화.
    ///
    /// 비성공 상태면 `{ "error": ... }`를 파싱해 [`TuiError::Api`]로, 파싱 실패 시
    /// raw 본문을 그대로 담는다.
    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<T, TuiError> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.http.get(&url).query(query);
        // 읽기 전용 MVP의 GET엔 불필요하지만, 향후 write 기능 대비해 배선.
        if let Some(key) = &self.admin_api_key {
            req = req.bearer_auth(key);
        }
        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let message = serde_json::from_str::<ApiErrorBody>(&body)
                .map(|b| b.error)
                .unwrap_or(body);
            return Err(TuiError::Api {
                status: status.as_u16(),
                message,
            });
        }
        resp.json::<T>()
            .await
            .map_err(|e| TuiError::Decode(e.to_string()))
    }

    /// `GET /v1/blocks/latest` — 최신 인덱싱 블록 번호 (빈 DB면 `None`).
    pub async fn latest_block(&self) -> Result<Option<i64>, TuiError> {
        let r: ApiResponse<Option<i64>> = self.get_json("/v1/blocks/latest", &[]).await?;
        Ok(r.data)
    }

    /// `GET /v1/analytics/failed-tx` — 카테고리별 실패 분석.
    pub async fn failed_tx_analysis(&self) -> Result<Vec<FailedTxAnalysis>, TuiError> {
        let r: ApiResponse<Vec<FailedTxAnalysis>> =
            self.get_json("/v1/analytics/failed-tx", &[]).await?;
        Ok(r.data)
    }

    /// `GET /v1/analytics/failed-tx/unknown-clusters` — UNKNOWN revert 클러스터.
    pub async fn unknown_clusters(&self) -> Result<Vec<UnknownRevertCluster>, TuiError> {
        let r: ApiResponse<Vec<UnknownRevertCluster>> = self
            .get_json("/v1/analytics/failed-tx/unknown-clusters", &[])
            .await?;
        Ok(r.data)
    }

    /// `GET /v1/failed-tx` — 필터·페이지네이션된 실패 트랜잭션 목록(`total` 포함).
    pub async fn list_failed_tx(
        &self,
        query: &[(String, String)],
    ) -> Result<TotalPaginated<FailedTransaction>, TuiError> {
        self.get_json("/v1/failed-tx", query).await
    }

    /// `GET /v1/failed-tx/{tx_hash}` — 단건 진단 상세.
    pub async fn failed_tx_detail(&self, tx_hash: &str) -> Result<FailedTxDetail, TuiError> {
        let path = format!("/v1/failed-tx/{tx_hash}");
        let r: ApiResponse<FailedTxDetail> = self.get_json(&path, &[]).await?;
        Ok(r.data)
    }
}
