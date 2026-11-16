//! 证据台账（grounding）——本轮回答里的数字到底是哪来的。
//!
//! # 为什么有这个模块
//!
//! 2026-09-23 甲方现场：问「北京天气怎么样」和「陕西渭南蒲城县天气怎么样」，两次回答
//! 给出**逐字相同**的一组数据（多云转晴 / 18℃~26℃ / 湿度55% / 风速3-4级 / 降水概率10%）。
//! 两个城市相距 800 公里不可能同温同湿同风速——数据是编造的。
//!
//! 事后查明，问题不是工具坏了（`get_weather` 返回的是真实数据），而是**没有任何一层
//! 在检查"回答里的数字是否来自工具返回"**：提示词里写了"不要脑补"，但没人验证。
//! 本模块补的就是这一层。
//!
//! # 两条硬性约束
//!
//! 1. **判定必须是确定性的**——正则/字符串抽取 + 包含判定，**绝不叫 LLM 判断**。
//!    让一个可能编造的模型去检查另一个可能编造的模型，等于没检查。
//! 2. **判定必须与 FastPlan 共用同一套词汇与口径**。两条链（本地对话链 / 规划链）
//!    面向同一个用户、同一套前端组件；若一边说 grounded 一边说"有来源"，前端就没法
//!    用同一个渲染器，用户看到的就是自相矛盾的两个系统。
//!    状态串直接沿用 FastPlan `executor::grounding_flow::GroundingState` 的
//!    `grounded` / `partial` / `ungrounded`（另加 FastClaw 需要的 `skipped`）。
//!
//! # 三个文件的分工
//!
//! - `extract`：文本 → 具体值（纯函数，可单测）
//! - `verify`：具体值 + 证据 → 判定与 claims
//! - 本文件：两端共享的数据契约
//!
//! # 判定与放行是两件事（容易混，先说清楚）
//!
//! - **值级判定**（本模块）：回答里的数字能不能在工具返回里找到。用于**提示**。
//! - **放行判定**（调用方）：声明的工具究竟有没有成功返回数据。用于**拦截**。
//!
//! 拦截不看值级判定，因为值级判定有误伤的可能（正则抽取天然保守），拿它做硬拦截会
//! 把正确回答也拦掉；而"工具到底调没调成功"是布尔事实，没有误判空间。

pub mod extract;
pub mod verify;

// 只对外暴露 `build` 与数据契约：抽取/判定原语是模块内部实现，
// 调用方要的是「结论 + 台账」，不该自己拿正则去判一遍（两条判据一漂移就会打架）。
pub use verify::build;

use serde::{Deserialize, Serialize};

/// 一次工具调用的证据条目。
///
/// 与 FastPlan `executor::provenance::EvidenceSource` 同构；FastClaw 额外需要
/// [`success`](Self::success)——FastPlan 的证据清单只收录成功的调用，而 FastClaw 要
/// 靠"声明的工具到底成没成"做放行判定，失败记录必须留在台账里。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSource {
    /// 序号（本轮内稳定，供 [`ClaimTrace::sources`] 引用）
    pub index: usize,
    /// 工具名
    pub tool: String,
    /// 调用入参摘要（截断）
    pub query: String,
    /// 返回内容摘要（截断）
    pub excerpt: String,
    /// 本次调用是否成功
    pub success: bool,
    /// 失败原因（成功时为 `None`）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 一条具体值的溯源结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimTrace {
    /// 具体值本身（数字/日期/编号）
    pub value: String,
    /// 命中的证据序号；为空表示**无出处**
    pub sources: Vec<usize>,
}

impl ClaimTrace {
    pub fn is_grounded(&self) -> bool {
        !self.sources.is_empty()
    }
}

/// 本轮接地的总判定。
///
/// 取值串与 FastPlan 对齐（见模块头约束 2）。`Skipped` 是 FastClaw 新增的：
/// FastPlan 的节点一定产出了结论、一定要判真假；FastClaw 的对话可能压根没出现
/// 可校验的具体值，这时"没判定"和"判定通过"必须区分开——否则前端会把一次
/// 未经检查的回答显示成绿色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroundingState {
    /// 校验通过：回答里的具体值基本都能在证据里找到。
    Grounded,
    /// 部分有据、部分对不上。
    Partial,
    /// 有具体值，但一个都对不上——典型的编造形态。
    Ungrounded,
    /// 回答里没有可校验的具体值，本轮未做判定。
    Skipped,
}

impl GroundingState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Grounded => "grounded",
            Self::Partial => "partial",
            Self::Ungrounded => "ungrounded",
            Self::Skipped => "skipped",
        }
    }
}

/// 本轮对话的完整证据台账，落进 assistant 消息的 `extra_data.grounding`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingManifest {
    pub sources: Vec<EvidenceSource>,
    pub claims: Vec<ClaimTrace>,
    pub grounding: GroundingState,
}

impl GroundingManifest {
    /// 本轮是否有过**成功**的工具调用。
    ///
    /// 这是放行判定的唯一依据（见模块头"判定与放行是两件事"）。
    pub fn has_successful_source(&self) -> bool {
        self.sources.iter().any(|s| s.success)
    }

    /// 指定工具是否成功返回过数据。
    pub fn tool_succeeded(&self, tool: &str) -> bool {
        self.sources
            .iter()
            .any(|s| s.success && s.tool == tool)
    }

    /// 声明了却没成功调用的工具列表（用于"未获取到实时数据"的说明文案）。
    pub fn missing_tools<'a>(&self, required: &'a [String]) -> Vec<&'a str> {
        required
            .iter()
            .filter(|t| !self.tool_succeeded(t))
            .map(|t| t.as_str())
            .collect()
    }

    /// 没有出处的具体值（最多 [`extract::MAX_SAMPLES`] 条，避免界面被刷屏）。
    pub fn unsourced_values(&self) -> Vec<&str> {
        self.claims
            .iter()
            .filter(|c| !c.is_grounded())
            .map(|c| c.value.as_str())
            .take(extract::MAX_SAMPLES)
            .collect()
    }

    /// 追加到回答末尾的**系统生成**提示；`None` 表示本轮无需提示。
    ///
    /// 这段文字由系统拼接，不经过模型——模型无法通过任何措辞让提示消失或不出现。
    ///
    /// `expects_live_data`：本轮问题是否**本应**由工具回答（场景探测判定需要实时数据，
    /// 或选中的 skill 声明了 live-data）。这个闸门很关键——少了它，「帮我写首诗」
    /// 这类压根不需要检索的对话也会被挂上"未经工具检索"，用户很快就会学会无视所有
    /// 提示，这层防线就白做了。
    pub fn annotation(&self, expects_live_data: bool) -> Option<String> {
        if !self.has_successful_source() {
            // 一次成功的工具调用都没有：本该有数据却没有 → 提示；本来就不需要 → 沉默
            return expects_live_data
                .then(|| "⚠️ 本回答未经工具检索，内容可能不准确。".to_string());
        }

        // 有工具真实返回，但回答里的具体值**大面积**对不上——这才需要逐个列出来。
        //
        // 注意这里必须用判定的档位而不是"只要有一个值对不上就报"：模型基于证据做
        // 一步换算（两条数据相减得增幅）产生的派生值，本来就不会字面出现在证据里。
        // 按档位卡（超半数对不上）才不会把正常回答全打成可疑。
        if matches!(
            self.grounding,
            GroundingState::Ungrounded | GroundingState::Partial
        ) {
            let unsourced = self.unsourced_values();
            if !unsourced.is_empty() {
                return Some(format!(
                    "⚠️ 以下数值未能在工具返回中找到出处：{}",
                    unsourced.join("、")
                ));
            }
        }

        None
    }
}

/// 构建台账的输入：一次工具调用的完整记录。
///
/// `result` 保留**全文**——证据摘要是在 [`verify::build`] 里截断的，
/// 判定必须基于全文，否则被截断掉的那半段里的数值会被误判成"无出处"。
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub tool: String,
    /// 入参（JSON 文本或摘要）
    pub query: String,
    /// 返回全文
    pub result: String,
    pub success: bool,
    pub error: Option<String>,
}

impl ToolCall {
    /// 成功的调用（大多数场景）。
    pub fn ok(tool: impl Into<String>, query: impl Into<String>, result: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            query: query.into(),
            result: result.into(),
            success: true,
            error: None,
        }
    }

    /// 失败的调用——同样进台账，供放行判定与失败原因展示使用。
    pub fn failed(
        tool: impl Into<String>,
        query: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            tool: tool.into(),
            query: query.into(),
            result: String::new(),
            success: false,
            error: Some(error.into()),
        }
    }
}
