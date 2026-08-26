//! 입력 이벤트 → [`Action`] 매핑 + 백그라운드 fetch 결과 메시지 [`DataMsg`].
//!
//! `map_key`는 순수 함수(부수효과 없음)라 단위테스트할 수 있다. 실제 비동기
//! 이벤트 소스(`EventStream`)와 틱은 [`crate::app::run`]이 `tokio::select!`로
//! 구동한다.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::Screen;
use crate::dto::{
    FailedTransaction, FailedTxAnalysis, FailedTxDetail, TotalPaginated, UnknownRevertCluster,
};

/// 사용자 입력에서 파생된 의도 (Elm 스타일 메시지).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// 종료.
    Quit,
    /// 다음 탭.
    NextTab,
    /// 이전 탭.
    PrevTab,
    /// 특정 화면으로 점프.
    SelectScreen(Screen),
    /// 위로 (선택 이동 / 스크롤).
    Up,
    /// 아래로 (선택 이동 / 스크롤).
    Down,
    /// 선택 행으로 드릴인.
    Enter,
    /// 뒤로 / 오버레이 닫기.
    Back,
    /// 카테고리 필터 순환.
    CycleCategory,
    /// 기간 필터 순환.
    CycleWindow,
    /// 다음 페이지.
    NextPage,
    /// 이전 페이지.
    PrevPage,
    /// 지금 새로고침.
    RefreshNow,
    /// 자동 폴링 토글.
    ToggleAutoPoll,
    /// 도움말 오버레이 토글.
    ToggleHelp,
    /// 무시.
    None,
}

/// 백그라운드 fetch 태스크가 보내는 결과. 에러는 `String`으로 운반(태스크
/// 경계를 넘기 쉽게). 큰 페이로드는 enum 크기 균형을 위해 `Box`.
#[derive(Debug)]
pub enum DataMsg {
    /// `latest_block` 결과.
    LatestBlock(Result<Option<i64>, String>),
    /// `failed_tx_analysis` 결과.
    Analysis(Result<Vec<FailedTxAnalysis>, String>),
    /// `list_failed_tx` 결과.
    FailedList(Result<Box<TotalPaginated<FailedTransaction>>, String>),
    /// `failed_tx_detail` 결과.
    Detail(Result<Box<FailedTxDetail>, String>),
    /// `unknown_clusters` 결과.
    Clusters(Result<Vec<UnknownRevertCluster>, String>),
}

/// crossterm 키 이벤트를 [`Action`]으로 변환한다.
///
/// `show_help`가 true면 도움말 오버레이가 떠 있으므로 닫기 키만 처리한다.
pub fn map_key(key: KeyEvent, show_help: bool) -> Action {
    // Ctrl+C는 어디서나 종료.
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
        return Action::Quit;
    }

    if show_help {
        return match key.code {
            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => Action::ToggleHelp,
            _ => Action::None,
        };
    }

    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('?') => Action::ToggleHelp,
        KeyCode::Char('r') => Action::RefreshNow,
        KeyCode::Char('p') => Action::ToggleAutoPoll,
        KeyCode::Tab => Action::NextTab,
        KeyCode::BackTab => Action::PrevTab,
        KeyCode::Char('1') => Action::SelectScreen(Screen::Overview),
        KeyCode::Char('2') => Action::SelectScreen(Screen::FailedTx),
        KeyCode::Char('3') => Action::SelectScreen(Screen::Detail),
        KeyCode::Char('4') => Action::SelectScreen(Screen::Clusters),
        KeyCode::Esc => Action::Back,
        KeyCode::Enter => Action::Enter,
        KeyCode::Char('j') | KeyCode::Down => Action::Down,
        KeyCode::Char('k') | KeyCode::Up => Action::Up,
        KeyCode::Char('c') => Action::CycleCategory,
        KeyCode::Char('t') => Action::CycleWindow,
        KeyCode::Char('n') | KeyCode::PageDown => Action::NextPage,
        KeyCode::Char('b') | KeyCode::PageUp => Action::PrevPage,
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn ctrl_c_quits() {
        let ev = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(map_key(ev, false), Action::Quit);
    }

    #[test]
    fn basic_keys_map() {
        assert_eq!(map_key(key(KeyCode::Char('q')), false), Action::Quit);
        assert_eq!(map_key(key(KeyCode::Tab), false), Action::NextTab);
        assert_eq!(map_key(key(KeyCode::Char('j')), false), Action::Down);
        assert_eq!(map_key(key(KeyCode::Enter), false), Action::Enter);
        assert_eq!(
            map_key(key(KeyCode::Char('2')), false),
            Action::SelectScreen(Screen::FailedTx)
        );
        assert_eq!(
            map_key(key(KeyCode::Char('4')), false),
            Action::SelectScreen(Screen::Clusters)
        );
    }

    #[test]
    fn help_overlay_swallows_other_keys() {
        assert_eq!(map_key(key(KeyCode::Char('j')), true), Action::None);
        assert_eq!(map_key(key(KeyCode::Char('?')), true), Action::ToggleHelp);
        assert_eq!(map_key(key(KeyCode::Esc), true), Action::ToggleHelp);
    }
}
