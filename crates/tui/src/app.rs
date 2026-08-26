//! 애플리케이션 상태 머신 + 비동기 메인 루프.
//!
//! 렌더 루프는 절대 HTTP를 `.await`하지 않는다 — fetch는 `tokio::spawn`으로
//! 분리하고 결과를 [`mpsc`] 채널로 받는다. 화면별 [`Loadable`] 상태로 스피너/
//! 에러 배너를 비차단 렌더한다.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures_util::StreamExt;
use ratatui::widgets::TableState;
use tokio::sync::mpsc;

use crate::client::ApiClient;
use crate::config::TuiConfig;
use crate::dto::{
    FailedTransaction, FailedTxAnalysis, FailedTxDetail, TotalPaginated, UnknownRevertCluster,
};
use crate::event::{self, Action, DataMsg};
use crate::format;
use crate::terminal::Tui;
use crate::ui;

/// 화면(탭).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// KPI + 카테고리 분포.
    Overview,
    /// 실패 트랜잭션 목록.
    FailedTx,
    /// 단건 진단 상세.
    Detail,
    /// UNKNOWN revert 클러스터.
    Clusters,
}

impl Screen {
    /// 탭 인덱스 (0-based).
    pub fn index(self) -> usize {
        match self {
            Screen::Overview => 0,
            Screen::FailedTx => 1,
            Screen::Detail => 2,
            Screen::Clusters => 3,
        }
    }
}

/// 화면 순환 순서.
const SCREEN_ORDER: [Screen; 4] = [
    Screen::Overview,
    Screen::FailedTx,
    Screen::Detail,
    Screen::Clusters,
];

/// 시간 필터 윈도우.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeWindow {
    /// 전체 기간.
    All,
    /// 최근 24시간.
    H24,
    /// 최근 7일.
    D7,
    /// 최근 30일.
    D30,
}

impl TimeWindow {
    /// 표시 라벨.
    pub fn label(self) -> &'static str {
        match self {
            TimeWindow::All => "All",
            TimeWindow::H24 => "24h",
            TimeWindow::D7 => "7d",
            TimeWindow::D30 => "30d",
        }
    }

    /// 다음 윈도우 (순환).
    fn next(self) -> Self {
        match self {
            TimeWindow::All => TimeWindow::H24,
            TimeWindow::H24 => TimeWindow::D7,
            TimeWindow::D7 => TimeWindow::D30,
            TimeWindow::D30 => TimeWindow::All,
        }
    }

    /// `from` 쿼리 값(RFC3339). `All`은 None.
    fn start_iso(self) -> Option<String> {
        let dur = match self {
            TimeWindow::All => return None,
            TimeWindow::H24 => chrono::Duration::hours(24),
            TimeWindow::D7 => chrono::Duration::days(7),
            TimeWindow::D30 => chrono::Duration::days(30),
        };
        Some((Utc::now() - dur).to_rfc3339())
    }
}

/// 비동기 로딩 상태.
#[derive(Debug, Clone, Default)]
pub enum Loadable<T> {
    /// 아직 요청 전.
    #[default]
    Idle,
    /// 로딩 중 (표시할 이전 데이터 없음).
    Loading,
    /// 로드 완료.
    Loaded(T),
    /// 실패 (에러 메시지).
    Failed(String),
}

/// Failed-TX 화면의 필터 상태.
#[derive(Debug, Clone)]
pub struct FailedTxFilter {
    /// 카테고리 필터 (SCREAMING_SNAKE 와이어 값, None = 전체).
    pub category: Option<&'static str>,
    /// 기간 필터.
    pub window: TimeWindow,
    /// 페이지 크기.
    pub limit: i64,
    /// 오프셋.
    pub offset: i64,
}

impl FailedTxFilter {
    /// `GET /v1/failed-tx` 쿼리 페어로 변환.
    fn query_pairs(&self) -> Vec<(String, String)> {
        let mut v = vec![
            ("limit".to_string(), self.limit.to_string()),
            ("offset".to_string(), self.offset.to_string()),
        ];
        if let Some(c) = self.category {
            v.push(("category".to_string(), c.to_string()));
        }
        if let Some(from) = self.window.start_iso() {
            v.push(("from".to_string(), from));
        }
        v
    }
}

/// 앱 상태.
pub struct App {
    /// 런타임 설정.
    pub config: TuiConfig,
    /// API 클라이언트 (fetch 태스크에 clone).
    pub client: Arc<ApiClient>,
    /// fetch 결과 송신 단.
    pub tx: mpsc::UnboundedSender<DataMsg>,
    /// 현재 화면.
    pub screen: Screen,
    /// 이전 화면 (Detail → 뒤로용).
    pub prev_screen: Screen,
    /// 종료 플래그.
    pub should_quit: bool,
    /// 도움말 오버레이 표시 여부.
    pub show_help: bool,
    /// 자동 폴링 on/off.
    pub auto_poll: bool,
    /// 스피너 애니메이션 카운터.
    pub tick: u64,
    /// 진행 중인 fetch 수 (스피너 표시용).
    pub inflight: u32,
    /// 마지막 갱신 시각.
    pub last_updated: Option<Instant>,
    /// Overview: 최신 블록.
    pub latest_block: Loadable<Option<i64>>,
    /// Overview: 카테고리 분석.
    pub analysis: Loadable<Vec<FailedTxAnalysis>>,
    /// Failed-TX: 목록(`total` 포함).
    pub failed: Loadable<TotalPaginated<FailedTransaction>>,
    /// Failed-TX: 테이블 선택 상태.
    pub table_state: TableState,
    /// Failed-TX: 필터.
    pub filter: FailedTxFilter,
    /// Detail: 진단 상세.
    pub detail: Loadable<FailedTxDetail>,
    /// Detail: 수직 스크롤.
    pub detail_scroll: u16,
    /// Detail 대상 tx 해시.
    pub selected_hash: Option<String>,
    /// Clusters: UNKNOWN revert 클러스터 목록.
    pub clusters: Loadable<Vec<UnknownRevertCluster>>,
    /// Clusters: 테이블 선택 상태.
    pub cluster_state: TableState,
}

impl App {
    /// 초기 상태를 만든다.
    pub fn new(
        config: TuiConfig,
        client: Arc<ApiClient>,
        tx: mpsc::UnboundedSender<DataMsg>,
    ) -> Self {
        Self {
            config,
            client,
            tx,
            screen: Screen::Overview,
            prev_screen: Screen::Overview,
            should_quit: false,
            show_help: false,
            auto_poll: true,
            tick: 0,
            inflight: 0,
            last_updated: None,
            latest_block: Loadable::Idle,
            analysis: Loadable::Idle,
            failed: Loadable::Idle,
            table_state: TableState::default(),
            filter: FailedTxFilter {
                category: None,
                window: TimeWindow::All,
                limit: 20,
                offset: 0,
            },
            detail: Loadable::Idle,
            detail_scroll: 0,
            selected_hash: None,
            clusters: Loadable::Idle,
            cluster_state: TableState::default(),
        }
    }

    /// [`Action`]을 상태 전이로 적용한다.
    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Quit => self.should_quit = true,
            Action::ToggleHelp => self.show_help = !self.show_help,
            Action::ToggleAutoPoll => self.auto_poll = !self.auto_poll,
            Action::RefreshNow => self.spawn_fetch_active(),
            Action::NextTab => self.cycle_screen(1),
            Action::PrevTab => self.cycle_screen(-1),
            Action::SelectScreen(s) => self.goto_screen(s),
            Action::Up => self.on_up(),
            Action::Down => self.on_down(),
            Action::Enter => self.on_enter(),
            Action::Back => self.on_back(),
            Action::CycleCategory => {
                if matches!(self.screen, Screen::FailedTx) {
                    self.cycle_category();
                }
            }
            Action::CycleWindow => {
                if matches!(self.screen, Screen::FailedTx) {
                    self.cycle_window();
                }
            }
            Action::NextPage => {
                if matches!(self.screen, Screen::FailedTx) {
                    self.next_page();
                }
            }
            Action::PrevPage => {
                if matches!(self.screen, Screen::FailedTx) {
                    self.prev_page();
                }
            }
            Action::None => {}
        }
    }

    /// fetch 결과를 상태에 반영한다.
    pub fn ingest(&mut self, msg: DataMsg) {
        self.inflight = self.inflight.saturating_sub(1);
        self.last_updated = Some(Instant::now());
        match msg {
            DataMsg::LatestBlock(r) => self.latest_block = settle(r),
            DataMsg::Analysis(r) => self.analysis = settle(r),
            DataMsg::FailedList(r) => {
                self.failed = settle(r.map(|b| *b));
                self.clamp_selection();
            }
            DataMsg::Detail(r) => self.detail = settle(r.map(|b| *b)),
            DataMsg::Clusters(r) => {
                self.clusters = settle(r);
                self.clamp_cluster_selection();
            }
        }
    }

    /// 현재 화면에 필요한 데이터를 백그라운드로 가져온다.
    pub fn spawn_fetch_active(&mut self) {
        match self.screen {
            Screen::Overview => {
                begin(&mut self.latest_block);
                begin(&mut self.analysis);
                self.inflight += 2;
                let (c, t) = (self.client.clone(), self.tx.clone());
                tokio::spawn(async move {
                    let r = c.latest_block().await.map_err(|e| e.to_string());
                    let _ = t.send(DataMsg::LatestBlock(r));
                });
                let (c, t) = (self.client.clone(), self.tx.clone());
                tokio::spawn(async move {
                    let r = c.failed_tx_analysis().await.map_err(|e| e.to_string());
                    let _ = t.send(DataMsg::Analysis(r));
                });
            }
            Screen::FailedTx => {
                begin(&mut self.failed);
                self.inflight += 1;
                let (c, t) = (self.client.clone(), self.tx.clone());
                let query = self.filter.query_pairs();
                tokio::spawn(async move {
                    let r = c
                        .list_failed_tx(&query)
                        .await
                        .map(Box::new)
                        .map_err(|e| e.to_string());
                    let _ = t.send(DataMsg::FailedList(r));
                });
            }
            Screen::Detail => {
                if let Some(hash) = self.selected_hash.clone() {
                    begin(&mut self.detail);
                    self.inflight += 1;
                    let (c, t) = (self.client.clone(), self.tx.clone());
                    tokio::spawn(async move {
                        let r = c
                            .failed_tx_detail(&hash)
                            .await
                            .map(Box::new)
                            .map_err(|e| e.to_string());
                        let _ = t.send(DataMsg::Detail(r));
                    });
                }
            }
            Screen::Clusters => {
                begin(&mut self.clusters);
                self.inflight += 1;
                let (c, t) = (self.client.clone(), self.tx.clone());
                tokio::spawn(async move {
                    let r = c.unknown_clusters().await.map_err(|e| e.to_string());
                    let _ = t.send(DataMsg::Clusters(r));
                });
            }
        }
    }

    /// 스피너를 표시할지 — 진행 중 fetch가 있으면 true.
    pub fn is_busy(&self) -> bool {
        self.inflight > 0
    }

    /// 마지막 갱신 이후 경과 초.
    pub fn updated_secs_ago(&self) -> Option<u64> {
        self.last_updated.map(|t| t.elapsed().as_secs())
    }

    // ── 네비게이션 helpers ──────────────────────────────────

    fn cycle_screen(&mut self, dir: i32) {
        let idx = SCREEN_ORDER
            .iter()
            .position(|s| *s == self.screen)
            .unwrap_or(0) as i32;
        let next = SCREEN_ORDER[(idx + dir).rem_euclid(SCREEN_ORDER.len() as i32) as usize];
        self.goto_screen(next);
    }

    fn goto_screen(&mut self, s: Screen) {
        if self.screen != s {
            self.prev_screen = self.screen;
            self.screen = s;
            self.spawn_fetch_active();
        }
    }

    fn on_enter(&mut self) {
        if !matches!(self.screen, Screen::FailedTx) {
            return;
        }
        let hash = match &self.failed {
            Loadable::Loaded(p) => self
                .table_state
                .selected()
                .and_then(|i| p.data.get(i))
                .map(|r| r.tx_hash.clone()),
            _ => None,
        };
        if let Some(h) = hash {
            self.selected_hash = Some(h);
            self.detail_scroll = 0;
            self.goto_screen(Screen::Detail);
        }
    }

    fn on_back(&mut self) {
        if matches!(self.screen, Screen::Detail) {
            self.goto_screen(Screen::FailedTx);
        }
    }

    fn on_down(&mut self) {
        match self.screen {
            Screen::FailedTx => self.move_selection(1),
            Screen::Detail => self.detail_scroll = self.detail_scroll.saturating_add(1),
            Screen::Clusters => self.move_cluster_selection(1),
            Screen::Overview => {}
        }
    }

    fn on_up(&mut self) {
        match self.screen {
            Screen::FailedTx => self.move_selection(-1),
            Screen::Detail => self.detail_scroll = self.detail_scroll.saturating_sub(1),
            Screen::Clusters => self.move_cluster_selection(-1),
            Screen::Overview => {}
        }
    }

    fn move_selection(&mut self, dir: i32) {
        let len = match &self.failed {
            Loadable::Loaded(p) => p.data.len(),
            _ => 0,
        };
        if len == 0 {
            return;
        }
        let cur = self.table_state.selected().unwrap_or(0) as i32;
        let next = (cur + dir).clamp(0, len as i32 - 1) as usize;
        self.table_state.select(Some(next));
    }

    fn clamp_selection(&mut self) {
        let len = match &self.failed {
            Loadable::Loaded(p) => p.data.len(),
            _ => 0,
        };
        let new = match self.table_state.selected() {
            _ if len == 0 => None,
            Some(i) if i >= len => Some(len - 1),
            Some(i) => Some(i),
            None => Some(0),
        };
        self.table_state.select(new);
    }

    fn move_cluster_selection(&mut self, dir: i32) {
        let len = match &self.clusters {
            Loadable::Loaded(rows) => rows.len(),
            _ => 0,
        };
        if len == 0 {
            return;
        }
        let cur = self.cluster_state.selected().unwrap_or(0) as i32;
        let next = (cur + dir).clamp(0, len as i32 - 1) as usize;
        self.cluster_state.select(Some(next));
    }

    fn clamp_cluster_selection(&mut self) {
        let len = match &self.clusters {
            Loadable::Loaded(rows) => rows.len(),
            _ => 0,
        };
        let new = match self.cluster_state.selected() {
            _ if len == 0 => None,
            Some(i) if i >= len => Some(len - 1),
            Some(i) => Some(i),
            None => Some(0),
        };
        self.cluster_state.select(new);
    }

    fn cycle_category(&mut self) {
        let next = match self.filter.category {
            None => Some(format::CATEGORIES[0]),
            Some(cur) => {
                let idx = format::CATEGORIES
                    .iter()
                    .position(|c| *c == cur)
                    .unwrap_or(0);
                format::CATEGORIES.get(idx + 1).copied()
            }
        };
        self.filter.category = next;
        self.filter.offset = 0;
        self.spawn_fetch_active();
    }

    fn cycle_window(&mut self) {
        self.filter.window = self.filter.window.next();
        self.filter.offset = 0;
        self.spawn_fetch_active();
    }

    fn next_page(&mut self) {
        if let Loadable::Loaded(p) = &self.failed {
            if self.filter.offset + self.filter.limit < p.pagination.total {
                self.filter.offset += self.filter.limit;
                self.spawn_fetch_active();
            }
        }
    }

    fn prev_page(&mut self) {
        if self.filter.offset > 0 {
            self.filter.offset = (self.filter.offset - self.filter.limit).max(0);
            self.spawn_fetch_active();
        }
    }
}

/// 로드 완료 상태가 아니면 Loading으로 — 이미 로드된 데이터는 폴링 중에도
/// 화면에 유지(깜빡임 방지).
fn begin<T>(field: &mut Loadable<T>) {
    if !matches!(field, Loadable::Loaded(_)) {
        *field = Loadable::Loading;
    }
}

/// `Result`를 [`Loadable`]로 정착.
fn settle<T>(r: Result<T, String>) -> Loadable<T> {
    match r {
        Ok(v) => Loadable::Loaded(v),
        Err(e) => Loadable::Failed(e),
    }
}

/// 메인 이벤트 루프 — 입력/틱/데이터를 `tokio::select!`로 비차단 처리한다.
pub async fn run(terminal: &mut Tui, config: TuiConfig) -> anyhow::Result<()> {
    let client = Arc::new(ApiClient::new(&config)?);
    let (tx, mut rx) = mpsc::unbounded_channel::<DataMsg>();
    let mut app = App::new(config, client, tx);

    let mut events = EventStream::new();
    let interval_secs = app.config.refresh_interval_secs.max(1);
    let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // tokio interval의 첫 tick은 즉시 완료된다 — 초기 fetch와의 중복을 피하려고
    // 미리 소비한다.
    ticker.tick().await;

    app.spawn_fetch_active();

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;
        if app.should_quit {
            break;
        }
        tokio::select! {
            maybe_event = events.next() => match maybe_event {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    let action = event::map_key(key, app.show_help);
                    app.apply(action);
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => tracing::warn!(error = %e, "event stream error"),
                None => app.should_quit = true,
            },
            _ = ticker.tick() => {
                app.tick = app.tick.wrapping_add(1);
                if app.auto_poll {
                    app.spawn_fetch_active();
                }
            }
            Some(msg) = rx.recv() => app.ingest(msg),
        }
    }
    Ok(())
}
