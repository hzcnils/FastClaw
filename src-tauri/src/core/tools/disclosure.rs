// FastClaw · 渐进式工具披露（Progressive Tool Disclosure）
//
// ═══ 你问的「只加载工具名字和描述，用到再加载详细的」——这个机制现在才算真正存在 ═══
//
// 原有的 ToolSearch 看起来像这个机制，但它有三个致命缺陷，导致它从来没有真正работать：
//
//   ① **它不返回 JSON Schema**。ToolSearch 的输出是
//      `"Bash: Execute shell commands"` 这样的字符串。模型知道了工具存在，
//      但不知道参数长什么样 —— 依然没法调用。
//
//   ② **目录是硬编码的静态数组，且已经过期**。TOOL_CATALOG 里写的是
//      `Bash`/`Read`/`Write`，而注册表里实际注册的是 `bash`/`read_file`/`write_file`。
//      模型按 ToolSearch 返回的名字去调，会直接 NotFound。
//
//   ③ **它自己的注释就写着「In FastClaw all tools are always available」**。
//      也就是说这个工具从设计上就不是为了省 token，只是个"能力介绍"接口。
//
// 本模块提供真正的两阶段披露：
//
//   阶段一（常驻，便宜）：所有工具的 **名称 + 一行摘要**，约 40 字/个 × 40 个 ≈ 600 token
//   阶段二（按需，昂贵）：模型调用 `load_tool` 后，才注入完整 JSON Schema
//
// 与 selection.rs 的分工：
//   - selection.rs 做的是**静态裁剪**（按会话场景猜哪些工具可能用到）
//   - 本模块做的是**动态发现**（模型自己说"我要用这个"，然后才给详细定义）
//   二者互补：裁剪负责常见情况的零成本命中，披露负责长尾的兜底。

use crate::core::llm::ToolDefinition;
use std::collections::BTreeSet;
use std::sync::Mutex;

/// 阶段一：生成"工具清单"文本，注入 system prompt。
///
/// 只含名称 + 摘要（description 的第一句），不含 JSON Schema。
/// 这是常驻成本，必须尽可能小。
pub fn build_catalog_block(all: &[ToolDefinition], already_loaded: &[String]) -> String {
    // ★ 防御性兜底：这个函数只是在拼提示词，任何 panic 都不该让整个
    //   对话请求直接死掉（此前 first_sentence 的 UTF-8 越界就导致
    //   「生成一个工资发放的excel」整条请求无声消失，前端一直转圈）。
    //   拼不出清单最多是模型少看到几个可选工具，远好过请求崩溃。
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        build_catalog_block_inner(all, already_loaded)
    })) {
        Ok(s) => s,
        Err(_) => {
            tracing::error!("build_catalog_block 内部 panic，已降级为空清单");
            String::new()
        }
    }
}

fn build_catalog_block_inner(all: &[ToolDefinition], already_loaded: &[String]) -> String {
    if all.is_empty() {
        return String::new();
    }

    let mut s = String::with_capacity(2048);
    s.push_str("\n# 可按需加载的工具\n\n");
    s.push_str(
        "下面是当前**未**加载详细定义的工具。你只能看到名称和用途摘要。\n\
         如果确定需要其中某个，先调用 `load_tool`（参数 `names`，可一次传多个），\n\
         系统会把它们的完整参数定义注入后续对话，然后你才能正常调用。\n\
         不确定要不要用的，不要加载 —— 加载是有成本的。\n\n",
    );

    let loaded: BTreeSet<&str> = already_loaded.iter().map(|s| s.as_str()).collect();
    let mut count = 0;

    for def in all {
        let name = def.function.name.as_str();
        if loaded.contains(name) {
            continue;
        }
        // 取 description 第一句作为摘要，控制在 ~60 字符
        let summary = first_sentence(&def.function.description, 60);
        s.push_str(&format!("- `{name}` — {summary}\n"));
        count += 1;
    }

    if count == 0 {
        return String::new();
    }
    s.push('\n');
    s
}

/// 取首句并截断。
///
/// ★ UTF-8 边界：`find()` 返回的是分隔符的**首字节**下标，
///   对多字节字符（'。' 占 3 字节）做 `i + 1` 会落在字符中间，
///   切片时 panic：
///     "end byte index 58 is not a char boundary; it is inside '。'"
///   这是我引入的 bug。改成用分隔符自身的字节长度推进。
fn first_sentence(desc: &str, max_chars: usize) -> String {
    let first = desc
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();

    // 找到第一个句子分隔符，并按该字符的真实字节长度推进到其后
    let cut = first
        .char_indices()
        .find(|(_, c)| matches!(c, '.' | '。' | '\n'))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(first.len());

    // cut 现在保证落在字符边界上；再按 char 截断，双重安全
    let s: String = first[..cut].chars().take(max_chars).collect();
    let s = s.trim().trim_end_matches(['.', '。']).to_string();
    if s.is_empty() {
        "（无描述）".to_string()
    } else {
        s
    }
}

/// 会话级"已加载工具"记录。
///
/// key 为 conversation_id。模型调用 load_tool 后，对应工具名进入这个集合，
/// 之后每一轮请求都会把它们的完整定义注入 tools 数组。
///
/// 用 Mutex + BTreeMap 而非 DashMap：不引入新依赖，且这里竞争极低
/// （每个会话串行执行）。
static LOADED_TOOLS: Mutex<Option<std::collections::BTreeMap<String, BTreeSet<String>>>> =
    Mutex::new(None);

fn with_map<R>(
    f: impl FnOnce(&mut std::collections::BTreeMap<String, BTreeSet<String>>) -> R,
) -> R {
    let mut g = LOADED_TOOLS.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        *g = Some(std::collections::BTreeMap::new());
    }
    f(g.as_mut().unwrap())
}

/// 记录某会话已加载的工具。
pub fn mark_loaded(conversation_id: &str, names: &[String]) {
    with_map(|m| {
        let e = m.entry(conversation_id.to_string()).or_default();
        for n in names {
            e.insert(n.clone());
        }
    });
}

/// 读取某会话已加载的工具列表。
pub fn get_loaded(conversation_id: &str) -> Vec<String> {
    with_map(|m| {
        m.get(conversation_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    })
}

/// 会话结束/清空上下文时调用，避免无限增长。
pub fn clear_session(conversation_id: &str) {
    with_map(|m| {
        m.remove(conversation_id);
    });
}

/// 校验请求加载的工具名是否真实存在，返回 (有效名, 无效名)。
pub fn validate_names(all: &[ToolDefinition], requested: &[String]) -> (Vec<String>, Vec<String>) {
    let known: BTreeSet<&str> = all.iter().map(|d| d.function.name.as_str()).collect();
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    for r in requested {
        // 容忍大小写差异 —— 代码库里工具名 PascalCase/snake_case 混用
        match known.iter().find(|k| k.eq_ignore_ascii_case(r)) {
            Some(k) => ok.push((*k).to_string()),
            None => bad.push(r.clone()),
        }
    }
    (ok, bad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::llm::{ToolDefinition, ToolFunctionDef};

    fn td(name: &str, desc: &str) -> ToolDefinition {
        ToolDefinition {
            r#type: "function".into(),
            function: ToolFunctionDef {
                name: name.into(),
                description: desc.into(),
                parameters: Some(serde_json::json!({"type":"object"})),
            },
        }
    }

    #[test]
    fn catalog_excludes_loaded() {
        let all = vec![
            td("bash", "Execute shell commands. Very powerful."),
            td("glob", "Find files by pattern."),
        ];
        let block = build_catalog_block(&all, &["bash".into()]);
        assert!(!block.contains("`bash`"));
        assert!(block.contains("`glob`"));
    }

    #[test]
    fn catalog_is_compact() {
        let all: Vec<_> = (0..40)
            .map(|i| td(&format!("tool{i}"), "Some fairly long description that goes on and on and should be truncated aggressively."))
            .collect();
        let block = build_catalog_block(&all, &[]);
        // 40 个工具的清单应该远小于它们的完整 schema（约 8-9k token）
        assert!(
            block.chars().count() < 4000,
            "catalog too big: {}",
            block.chars().count()
        );
    }

    #[test]
    fn name_validation_is_case_insensitive() {
        let all = vec![td("ToolSearch", "x"), td("bash", "y")];
        let (ok, bad) = validate_names(&all, &["toolsearch".into(), "nope".into()]);
        assert_eq!(ok, vec!["ToolSearch".to_string()]);
        assert_eq!(bad, vec!["nope".to_string()]);
    }

    #[test]
    fn session_isolation() {
        clear_session("c1");
        clear_session("c2");
        mark_loaded("c1", &["bash".into()]);
        assert_eq!(get_loaded("c1"), vec!["bash".to_string()]);
        assert!(get_loaded("c2").is_empty());
        clear_session("c1");
    }
}
