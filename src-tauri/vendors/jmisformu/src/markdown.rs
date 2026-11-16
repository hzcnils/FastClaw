//! 从纯文本片段中提取 LaTeX 公式。
//!
//! 识别：
//!   - 块级公式 `$$ ... $$` 与 `\[ ... \]`
//!   - 行内公式 `$ ... $`   与 `\( ... \)`
//!
//! 设计要点：
//!   - 处理转义的 `\$`（不视为公式边界）。
//!   - **单 `$` 遵循 CommonMark math 扩展的邻接规则**：开分隔符右侧、闭分隔符左侧
//!     都必须是非空白字符。没有这条规则时，正文里的
//!     `我花了 $5 买书，又花了 $10 吃饭` 会被吃成公式（见 R5）。
//!   - `$$` 不受邻接规则约束（块级公式允许换行排版）。

/// 文档被拆分成的片段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// 普通文本。
    Text(String),
    /// 行内公式的 LaTeX 源码（不含分隔符）。
    InlineMath(String),
    /// 块级公式的 LaTeX 源码（不含分隔符）。
    DisplayMath(String),
}

/// 在一段纯文本里扫描 `$...$` / `$$...$$` / `\(..\)` / `\[..\]`，产出有序片段。
#[must_use]
pub fn split_text_math(input: &str) -> Vec<Segment> {
    // 快速路径：绝大多数文本既没有 `$` 也没有 `\`，直接返回，避免 Vec<char> 分配。
    if !input.contains('$') && !input.contains('\\') {
        return if input.is_empty() {
            Vec::new()
        } else {
            vec![Segment::Text(input.to_string())]
        };
    }

    let mut segments = Vec::new();
    let mut buf = String::new();
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        let c = chars[i];

        // LaTeX 风格分隔符：\( ... \) 行内，\[ ... \] 块级。
        if c == '\\' && i + 1 < n {
            let is_display = match chars[i + 1] {
                '(' => Some(false),
                '[' => Some(true),
                _ => None,
            };
            if let Some(is_display) = is_display {
                let close_seq: [char; 2] = if is_display { ['\\', ']'] } else { ['\\', ')'] };
                if let Some(rel) = find_closing_seq(&chars, i + 2, &close_seq) {
                    let formula: String = chars[i + 2..rel].iter().collect();
                    let formula = formula.trim().to_string();
                    // 空公式（如 `\[ \]`）跳过,避免 DisplayMath("") 触发无谓导出降级
                    if !formula.is_empty() {
                        if !buf.is_empty() {
                            segments.push(Segment::Text(std::mem::take(&mut buf)));
                        }
                        segments.push(if is_display {
                            Segment::DisplayMath(formula)
                        } else {
                            Segment::InlineMath(formula)
                        });
                    }
                    i = rel + 2;
                    continue;
                }
            }
        }

        // 转义的 \$ —— 当作普通字符
        if c == '\\' && i + 1 < n && chars[i + 1] == '$' {
            buf.push('$');
            i += 2;
            continue;
        }

        if c == '$' {
            let is_display = i + 1 < n && chars[i + 1] == '$';
            let open_len = if is_display { 2 } else { 1 };

            // 单 $ 的开分隔符邻接规则：右侧必须存在且非空白。
            let open_ok = is_display
                || chars
                    .get(i + open_len)
                    .is_some_and(|c2| !c2.is_whitespace());

            if open_ok {
                if let Some(rel) = find_closing(&chars, i + open_len, is_display) {
                    let formula: String = chars[i + open_len..rel].iter().collect();
                    let formula = formula.trim().to_string();
                    // 空公式（如 `$$ $$` / `$$$$`）跳过,避免 DisplayMath("") 触发无谓导出降级
                    if !formula.is_empty() {
                        if !buf.is_empty() {
                            segments.push(Segment::Text(std::mem::take(&mut buf)));
                        }
                        segments.push(if is_display {
                            Segment::DisplayMath(formula)
                        } else {
                            Segment::InlineMath(formula)
                        });
                    }
                    i = rel + open_len;
                    continue;
                }
            }
        }

        buf.push(c);
        i += 1;
    }

    if !buf.is_empty() {
        segments.push(Segment::Text(buf));
    }
    segments
}

/// 从 `start` 开始，找到下一个 `\)` 或 `\]` 闭合序列（返回序列起始下标）。
fn find_closing_seq(chars: &[char], start: usize, close: &[char; 2]) -> Option<usize> {
    let n = chars.len();
    let mut j = start;
    while j + 1 < n {
        if chars[j] == close[0] && chars[j + 1] == close[1] {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// 从 `start` 开始，找到下一个未转义的 `$` / `$$`（返回分隔符起始下标）。
///
/// `is_display` 为 false 时施加闭分隔符邻接规则：其左侧必须是非空白字符，
/// 且公式体不能为空。
fn find_closing(chars: &[char], start: usize, is_display: bool) -> Option<usize> {
    let n = chars.len();
    let clen = if is_display { 2 } else { 1 };
    let mut j = start;
    while j < n {
        if chars[j] == '\\' {
            j += 2; // 跳过转义序列
            continue;
        }
        let matched = if is_display {
            j + 1 < n && chars[j] == '$' && chars[j + 1] == '$'
        } else {
            chars[j] == '$'
        };
        // display 允许闭 `$$` 紧贴开 `$$`（`$$$$` = 空公式,由上层跳过）;
        // inline 的紧贴情形由下面的左邻规则兜底。
        if matched && (is_display || j >= start + 1) {
            if is_display {
                return Some(j);
            }
            // 单 $：左邻非空白，且内容非空
            if j > start && !chars[j - 1].is_whitespace() {
                return Some(j);
            }
            // 不满足邻接规则：这个 $ 不能闭合，继续找下一个
        }
        j += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_and_display() {
        let segs = split_text_math(r"前文 $a+b$ 中间 $$\frac{1}{2}$$ 后文");
        assert_eq!(
            segs,
            vec![
                Segment::Text("前文 ".into()),
                Segment::InlineMath("a+b".into()),
                Segment::Text(" 中间 ".into()),
                Segment::DisplayMath(r"\frac{1}{2}".into()),
                Segment::Text(" 后文".into()),
            ]
        );
    }

    #[test]
    fn escaped_dollar() {
        let segs = split_text_math(r"价格 \$5 与 \$10");
        assert_eq!(segs, vec![Segment::Text("价格 $5 与 $10".into())]);
    }

    #[test]
    fn unclosed_is_text() {
        let segs = split_text_math(r"残缺 $a+b 没有闭合");
        assert_eq!(segs, vec![Segment::Text(r"残缺 $a+b 没有闭合".into())]);
    }

    #[test]
    fn latex_style_delimiters() {
        let segs = split_text_math(r"质量 \( m \) 与 \[ E=mc^2 \] 完");
        assert_eq!(
            segs,
            vec![
                Segment::Text("质量 ".into()),
                Segment::InlineMath("m".into()),
                Segment::Text(" 与 ".into()),
                Segment::DisplayMath("E=mc^2".into()),
                Segment::Text(" 完".into()),
            ]
        );
    }

    // ---- 回归：R5，正文里的美元金额不应被当成公式 ----

    #[test]
    fn currency_amounts_are_not_math() {
        let segs = split_text_math("我花了 $5 买书，又花了 $10 吃饭");
        assert_eq!(
            segs,
            vec![Segment::Text("我花了 $5 买书，又花了 $10 吃饭".into())],
            "闭分隔符左邻是空白，不应闭合"
        );
    }

    #[test]
    fn open_delimiter_followed_by_space_is_not_math() {
        let segs = split_text_math("总价 $ 100 和 $ 200");
        assert_eq!(segs, vec![Segment::Text("总价 $ 100 和 $ 200".into())]);
    }

    #[test]
    fn tight_inline_math_still_works() {
        let segs = split_text_math("设 $x$ 为变量");
        assert_eq!(
            segs,
            vec![
                Segment::Text("设 ".into()),
                Segment::InlineMath("x".into()),
                Segment::Text(" 为变量".into()),
            ]
        );
    }

    #[test]
    fn numeric_inline_math_still_works() {
        let segs = split_text_math("常数 $5$ 结束");
        assert_eq!(segs[1], Segment::InlineMath("5".into()));
    }

    #[test]
    fn no_panic_on_adversarial_input() {
        for s in ["$", "$$", "$$$", r"\(", r"\[", r"\)", "$\\", "$$\\$$", ""] {
            let _ = split_text_math(s);
        }
    }

    #[test]
    fn empty_math_is_skipped() {
        // 空定界符被吞掉，两侧文本留在同一 buffer 中合并
        assert_eq!(
            split_text_math(r"前 $$ $$ 后"),
            vec![Segment::Text("前  后".into())]
        );
        assert_eq!(
            split_text_math(r"前 $$$$ 后"),
            vec![Segment::Text("前  后".into())]
        );
        assert_eq!(
            split_text_math(r"前 \[ \] 后"),
            vec![Segment::Text("前  后".into())]
        );
        assert!(
            !split_text_math(r"$$ $$")
                .iter()
                .any(|s| matches!(s, Segment::DisplayMath(_) | Segment::InlineMath(_)))
        );
        // 有效公式不受影响
        assert!(split_text_math(r"$$a$$")
            .iter()
            .any(|s| matches!(s, Segment::DisplayMath(m) if m == "a")));
    }
}
