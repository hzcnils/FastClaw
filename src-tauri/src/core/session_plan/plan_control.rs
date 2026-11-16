//! 每个会话一个「规划等待槽」：规划确认（confirm / resume / cancel）经此投递，
//! 并保证同一个等待只被接收一次。
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanControl {
    Confirm,
    Cancel,
    Resume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitTarget {
    pub tree_id: String,
    pub node_id: Option<String>,
    pub step: bool,
}

pub struct PlanWait {
    id: String,
    target: WaitTarget,
    sender: Option<oneshot::Sender<PlanControl>>,
    accepted: Option<PlanControl>,
}
pub type PlanWaitSlot = Arc<Mutex<Option<PlanWait>>>;

/// None means a legacy desktop-owned conversation may handle the command instead.
/// Some(false/true) means newly delivered/already accepted. Errors MUST NOT be forwarded.
pub fn send_plan_control(
    slot: Option<&PlanWaitSlot>,
    ctrl: PlanControl,
    expected: Option<&str>,
) -> Result<Option<bool>, String> {
    let Some(slot) = slot else {
        return if expected.is_some() {
            Err("规划等待已失效，请刷新状态".into())
        } else {
            Ok(None)
        };
    };
    let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    let Some(wait) = guard.as_mut() else {
        return if expected.is_some() {
            Err("规划等待已失效，请刷新状态".into())
        } else {
            Ok(None)
        };
    };
    if expected.is_some_and(|id| id != wait.id) {
        return Err("规划步骤已变化，请使用当前确认栏".into());
    }
    if let Some(accepted) = wait.accepted {
        return if expected.is_some() && accepted == ctrl {
            Ok(Some(true))
        } else {
            Err("该规划步骤已接收操作，请等待状态更新".into())
        };
    }
    let valid = ctrl == PlanControl::Cancel
        || (wait.target.step && ctrl == PlanControl::Resume)
        || (!wait.target.step && ctrl == PlanControl::Confirm)
        || (expected.is_none() && wait.target.step && ctrl == PlanControl::Confirm);
    if !valid {
        return Err("操作与当前规划等待类型不匹配".into());
    }
    let sender = wait.sender.take().ok_or("规划确认已失效")?;
    sender
        .send(ctrl)
        .map_err(|_| "规划确认已失效".to_string())?;
    wait.accepted = Some(ctrl);
    Ok(Some(false))
}
