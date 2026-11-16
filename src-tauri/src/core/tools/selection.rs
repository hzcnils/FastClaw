// FastClaw · 工具集裁剪（Tool Scoping）
//
// 问题背景：
//   原实现把 ToolRegistry 里全部 40 个内置工具 + 全部 MCP 工具无差别塞给模型，
//   仅 schema 就约 8000-9000 token/轮，6 轮就是 5 万 token 的纯固定开销。
//   更严重的是认知负荷：模型面对 40 个工具做选择，注意力被稀释，
//   选错概率随工具数近似线性上升（"你底层 llm 模型是" → 去调 glob 翻文件）。
//
// 业界共识是单次注入 15-20 个工具是可用性拐点。本模块做两件事：
//   1. 分层（Tier）：核心工具常驻，长尾工具默认不注入
//   2. 按会话意图打分：只放出与当前问题相关的那一档
//
// 设计原则：宁可少放工具让模型说"我做不到"，也不要多放工具让模型乱调。

use crate::core::llm::ToolDefinition;

/// 工具分层。数字越小越常用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolTier {
    /// 核心：任何有工具的会话都注入。
    Core = 0,
    /// 场景：仅当会话意图命中对应场景时注入。
    Scoped = 1,
    /// 长尾：默认不注入，只能通过 tool_search 显式发现。
    Rare = 2,
}

/// 场景标签 —— 决定 Scoped 工具何时放出。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scenario {
    /// 读写本地文件 / 代码
    Filesystem,
    /// 联网检索
    Web,
    /// 执行命令 / 脚本 / skill
    Execution,
    /// 长任务、后台任务、定时
    TaskMgmt,
    /// 多智能体 / 远程 / 消息
    Orchestration,
}

/// 单个工具的路由元数据。
struct ToolMeta {
    tier: ToolTier,
    scenario: Option<Scenario>,
}

/// 工具名 → 分层 / 场景。未登记的工具（含未来新增）默认按 Scoped·Execution 处理，
/// 保证不会因为漏登记而"凭空消失"。
fn meta_for(name: &str) -> ToolMeta {
    use Scenario::*;
    use ToolTier::*;

    let (tier, scenario) = match name {
        // ── Core：几乎所有任务都可能用到 ──
        "read_file" => (Core, Some(Filesystem)),
        "write_file" => (Core, Some(Filesystem)),
        "edit_file" => (Core, Some(Filesystem)),
        "bash" => (Core, Some(Execution)),
        "web_search" => (Core, Some(Web)),
        // ★ get_weather 原本列为 Core（常驻），但实测有副作用：
        //   「帮我生成一个工资 excel」这类 Filesystem 任务里它也会被注入，
        //   模型看到上一轮遗留的天气话题就顺手调了一次，把两件事混着答。
        //   天气问题必然带"天气/气温/下雨"等词、必然命中 Web 场景，
        //   所以放 Scoped 完全够用，还能避免污染其他场景。
        "get_weather" => (Scoped, Some(Web)),
        // 其余结构化实时数据工具：同属 Web 场景，走 Scoped
        // （不占常驻额度，但只要探测到 Web 场景就会注入）
        "get_exchange_rate" => (Scoped, Some(Web)),
        "get_crypto_price" => (Scoped, Some(Web)),
        "get_location_info" => (Scoped, Some(Web)),
        "todo_write" => (Core, None),
        // tool_search 必须常驻：它是长尾工具的唯一入口
        "ToolSearch" => (Core, None),
        // load_tool 必须常驻：它是"渐进式披露清单"的唯一执行入口，
        // 缺了它，system prompt 里的工具清单就是画饼
        "load_tool" => (Core, None),

        // ── Scoped · Filesystem ──
        "glob" => (Scoped, Some(Filesystem)),
        "grep" => (Scoped, Some(Filesystem)),
        "apply_patch" => (Scoped, Some(Filesystem)),
        "batch_edit" => (Scoped, Some(Filesystem)),

        // ── Scoped · Web ──
        "web_fetch" => (Scoped, Some(Web)),
        "download_file" => (Scoped, Some(Web)),

        // ── Scoped · Execution ──
        "skill" => (Scoped, Some(Execution)),
        "run_skill" => (Scoped, Some(Execution)),
        "repl" => (Scoped, Some(Execution)),

        // ── Scoped · TaskMgmt ──
        "task_create" | "task_get" | "task_list" | "task_output" | "task_stop" | "task_update" => {
            (Scoped, Some(TaskMgmt))
        }
        "Sleep" => (Scoped, Some(TaskMgmt)),

        // ── Scoped · Orchestration ──
        "SendMessage" => (Scoped, Some(Orchestration)),

        // ── Rare：长尾，默认不注入 ──
        "Brief" | "StructuredOutput" | "config" | "goal_complete" | "ask_user" => (Rare, None),
        "enter_plan_mode" | "exit_plan_mode" => (Rare, None),
        "list_mcp_resources" | "read_mcp_resource" => (Rare, None),

        // 未登记 → 保守放行
        _ => (Scoped, Some(Execution)),
    };

    ToolMeta { tier, scenario }
}

// ── 场景探测 ────────────────────────────────────────────────────────

/// 从用户消息里探测涉及哪些场景。
///
/// 刻意用关键词而非 embedding：这一层只做"放宽"判断（宁可多放一档，
/// 也不要漏掉用户真正需要的工具），精度要求不高，但必须零延迟、零依赖。
/// 真正的精排交给模型自己在候选集里选。
pub fn detect_scenarios(user_message: &str) -> Vec<Scenario> {
    let m = user_message.to_lowercase();
    let mut out = Vec::new();

    let has = |kws: &[&str]| kws.iter().any(|k| m.contains(k));

    if has(&[
        "文件", "目录", "路径", "代码", "项目", "仓库", "读取", "写入", "修改", "重构", "报错",
        "日志", "配置", "excel", "文档", "表格", "csv", "pdf", "ppt", "file", "dir", "path",
        "code", "repo", "refactor", "read", "write", "edit", ".rs", ".ts", ".js", ".py", ".vue",
        ".json", ".md", ".xlsx",
    ]) {
        out.push(Scenario::Filesystem);
    }

    if has(&[
        "搜索",
        "查一下",
        "查查",
        "最新",
        "新闻",
        "天气",
        "股价",
        "汇率",
        "官网",
        "网页",
        "链接",
        "资料",
        "行情",
        "今天",
        "现在",
        "多少钱",
        "发布",
        "实时",
        // 结构化实时数据的触发词（对应 get_weather / get_exchange_rate /
        // get_crypto_price / get_location_info 四个专用工具）
        "汇率",
        "换算",
        "兑换",
        "美元",
        "人民币",
        "欧元",
        "日元",
        "港币",
        "币价",
        "比特币",
        "以太坊",
        "加密货币",
        "虚拟货币",
        "coin",
        "btc",
        "eth",
        "几点",
        "时区",
        "经纬度",
        "坐标",
        "当地时间",
        "时差",
        "气温",
        "温度",
        "下雨",
        "降水",
        "预报",
        // ★ 打探某个实体口碑/情况的疑问句式 —— 「汇智信科咋样」属于真实检索需求，
        //   不加这些会被误判为闲聊而不给工具
        "咋样",
        "靠谱",
        "口碑",
        "评价",
        "是什么公司",
        "什么来头",
        "search",
        "latest",
        "news",
        "weather",
        "http://",
        "https://",
        "www.",
    ]) {
        out.push(Scenario::Web);
    }

    if has(&[
        "运行", "执行", "安装", "编译", "构建", "启动", "部署", "命令", "脚本", "生成", "打包",
        "测试", "npm", "cargo", "pip", "docker", "git ", "run", "exec", "install", "build",
        "deploy", "script", "generate",
    ]) {
        out.push(Scenario::Execution);
    }

    if has(&[
        "定时",
        "每天",
        "每周",
        "后台",
        "任务",
        "监控",
        "调度",
        "计划",
        "提醒",
        "cron",
        "schedule",
        "background",
        "task",
        "monitor",
    ]) {
        out.push(Scenario::TaskMgmt);
    }

    if has(&[
        "多智能体",
        "子agent",
        "协作",
        "远程",
        "通知",
        "发消息",
        "团队",
        "agent",
        "team",
        "remote",
        "notify",
    ]) {
        out.push(Scenario::Orchestration);
    }

    // ★「X 怎么样 / X 如何」只有在 X 像专有名词（公司/产品/地名）时才算检索需求。
    //   直接把"怎么样""如何"当关键词会误伤「如何学好英语」这类纯知识问答，
    //   让它们白白失去流式输出。这里要求：疑问词在**句尾**，且前面的部分
    //   不含常见的泛化动词。
    if !out.contains(&Scenario::Web) {
        let trimmed = m.trim_end_matches(['?', '？', '。', '!', '！', ' ']);
        for suffix in ["怎么样", "如何", "怎样"] {
            if trimmed.ends_with(suffix) {
                let head = &trimmed[..trimmed.len() - suffix.len()];
                let generic = [
                    "学", "做", "写", "说", "看", "用", "才能", "可以", "应该", "能够", "怎么",
                    "才", "去",
                ];
                let head_len = head.chars().count();
                // 主体 2-12 字且不含泛化动词 → 判定为实体打探
                if head_len >= 2 && head_len <= 12 && !generic.iter().any(|g| head.contains(g)) {
                    out.push(Scenario::Web);
                }
                break;
            }
        }
    }

    out.sort_by_key(|s| format!("{s:?}"));
    out.dedup();
    out
}

// ── 裁剪主入口 ──────────────────────────────────────────────────────

/// 单次请求注入的工具数上限。
///
/// 原值 18 定得太紧：v5 加了 4 个结构化 API 工具后，Core 从 7 涨到 9，
/// 「生成 excel」这类同时命中 Execution+Filesystem 的任务会到 19 个，
/// 恰好超限 1 个并丢掉 write_file。
///
/// 24 能覆盖绝大多数双场景组合而不触发截断，同时仍远低于全量 48 个
/// （schema 约 5.9k token vs 全量 ~15k）。真超限时也已改为按优先级丢弃。
pub const MAX_TOOLS_PER_REQUEST: usize = 24;

/// 按会话意图裁剪工具集。
///
/// - `all`：全量工具定义（内置 + MCP）
/// - `user_message`：本轮用户消息，用于场景探测
/// - `forced`：必须保留的工具名（如 BERT 路由命中的候选、用户显式指定的 MCP）
///
/// MCP 工具永远保留：它们是用户主动连接的，代表明确意图，不该被启发式裁掉。
pub fn scope_tools(
    all: Vec<ToolDefinition>,
    user_message: &str,
    forced: &[String],
) -> Vec<ToolDefinition> {
    let scenarios = detect_scenarios(user_message);

    // ★ 带优先级收集：超限时必须丢**最不重要**的，而不是按字母序尾部盲砍。
    //
    //   之前的实现直接 `kept.truncate(18)`，而 kept 是按注册表（BTreeMap）
    //   的字母序填充的。实测「帮我生成一个人员工资发放的excel」命中
    //   Execution+Filesystem 两个场景共 19 个工具，超限 1 个，被砍掉的正好是
    //   字母序最后的 **write_file** —— 一个要生成文件的任务，写文件工具没了。
    //
    //   优先级：MCP/forced(0) > Core(1) > 场景命中的 Scoped(2)。
    //   同优先级内保持注册表顺序，保证 prompt 前缀稳定（缓存友好）。
    let mut picked: Vec<(u8, ToolDefinition)> = Vec::new();
    let mut deferred: Vec<ToolDefinition> = Vec::new();

    for def in all {
        let name = def.function.name.as_str();

        // ① MCP 工具：用户主动连接的，一律保留，优先级最高
        if name.starts_with("mcp__") {
            picked.push((0, def));
            continue;
        }

        // ② 强制保留名单（BERT 候选 / 已 load_tool 的）
        if forced.iter().any(|f| f == name) {
            picked.push((0, def));
            continue;
        }

        let meta = meta_for(name);
        match meta.tier {
            ToolTier::Core => picked.push((1, def)),
            ToolTier::Scoped => {
                let hit = match meta.scenario {
                    Some(s) => scenarios.contains(&s),
                    None => true,
                };
                if hit {
                    picked.push((2, def));
                } else {
                    deferred.push(def);
                }
            }
            // Rare 直接丢弃：模型需要时可通过 load_tool 找回
            ToolTier::Rare => {}
        }
    }

    // 若场景探测没命中任何东西（纯闲聊/知识问答），只剩 Core，
    // 这正是我们想要的 —— 不给模型无关的锤子。
    //
    // 超限时按优先级稳定排序后截断：先丢场景 Scoped，绝不丢 Core / MCP。
    if picked.len() > MAX_TOOLS_PER_REQUEST {
        let dropped: Vec<String> = {
            let mut idx: Vec<usize> = (0..picked.len()).collect();
            idx.sort_by_key(|&i| picked[i].0); // 稳定排序，同级保持原序
            idx.iter()
                .skip(MAX_TOOLS_PER_REQUEST)
                .map(|&i| picked[i].1.function.name.clone())
                .collect()
        };
        tracing::warn!(
            "🔧 工具超过单次上限 {}，已丢弃低优先级工具: [{}]（模型仍可用 load_tool 找回）",
            MAX_TOOLS_PER_REQUEST,
            dropped.join(", ")
        );
        picked.sort_by_key(|(p, _)| *p);
        picked.truncate(MAX_TOOLS_PER_REQUEST);
    }

    let _ = deferred; // 保留变量以便将来做"可发现工具提示"
    picked.into_iter().map(|(_, d)| d).collect()
}

/// 判断本轮是否**完全不需要**工具。
///
/// 命中时调用方应该直接不传 tools，让模型纯文本作答。这是对
/// "这个根本就不用调工具，直接大模型回答就行" 类问题最直接的修复，
/// 也顺带恢复了流式输出（有工具时流式被强制关闭）。
///
/// 判定保守：只在**明确是**闲聊/自我认知/纯知识问答时返回 true。
pub fn is_tool_free_question(user_message: &str) -> bool {
    let m = user_message.trim().to_lowercase();

    if m.is_empty() {
        return true;
    }

    // ★ 判定逻辑是**反过来**的：默认不需要工具，只有出现明确信号才需要。
    //
    //   最初的实现用白名单（列举"你是谁""你好"等），结果「你说」「写2000字小说」
    //   这类消息全都落到 else 分支 → 注入工具 → **流式被强制关闭**
    //   （core/llm/mod.rs 里 "stream": !has_tools）。
    //   用户看到的现象就是"突然不流式输出了"。
    //
    //   白名单永远列不全人类的说话方式。改成：只要没有任何"需要外部信息/
    //   需要操作本机"的信号，就一律不给工具。这样写作、闲聊、翻译、
    //   知识问答、代码生成全都能走流式。
    if !detect_scenarios(user_message).is_empty() {
        return false;
    }

    // detect_scenarios 主要覆盖中文关键词，这里补一些英文与符号信号
    let extra_signals = [
        // 明确的外部信息需求
        "http://",
        "https://",
        "www.",
        ".com",
        ".cn",
        ".org",
        // 明确的本机操作
        "c:\\",
        "d:\\",
        "/users/",
        "/home/",
        "./",
        "../",
        // 命令式动词（英文）
        "search for",
        "look up",
        "find out",
        "download",
        "install",
        "read the file",
        "write a file",
        "run the",
    ];
    if extra_signals.iter().any(|k| m.contains(k)) {
        return false;
    }

    // 走到这里 = 没有任何需要工具的信号 → 纯生成任务，可以流式
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_identity_needs_no_tools() {
        assert!(is_tool_free_question("你底层llm模型是  底层模型"));
        assert!(is_tool_free_question("你好"));
    }

    #[test]
    fn real_tasks_need_tools() {
        assert!(!is_tool_free_question("今天西安天气如何?"));
        assert!(!is_tool_free_question("生成一个工资发放的excel文件"));
    }

    #[test]
    fn weather_scopes_to_web_not_filesystem() {
        let s = detect_scenarios("今天西安天气如何?");
        assert!(s.contains(&Scenario::Web));
        assert!(!s.contains(&Scenario::Filesystem));
    }
}
