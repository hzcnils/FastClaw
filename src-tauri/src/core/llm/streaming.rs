/// 可选的流式观察者；不影响工具参数累积或工具执行。
#[derive(Debug, PartialEq)]
pub enum StreamUpdate {
    Reset,
    Token(String),
    Retry { attempt: u32, max_retries: u32, reason: String },
}

pub type StreamCallback<'a> = dyn Fn(StreamUpdate) + Send + Sync + 'a;

/// reasoning_content 与正文共用现有 <think> 传输协议。
#[derive(Default)]
pub(super) struct StreamContent {
    pub text: String,
    in_reasoning: bool,
}

impl StreamContent {
    fn push(&mut self, text: &str, callback: Option<&StreamCallback<'_>>) {
        self.text.push_str(text);
        if let Some(callback) = callback {
            callback(StreamUpdate::Token(text.to_owned()));
        }
    }

    pub fn delta(&mut self, delta: &serde_json::Value, callback: Option<&StreamCallback<'_>>) {
        if let Some(text) = delta.get("reasoning_content").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            if !self.in_reasoning {
                self.in_reasoning = true;
                self.push("<think>", callback);
            }
            self.push(text, callback);
        }
        if let Some(text) = delta.get("content").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
            self.finish(callback);
            self.push(text, callback);
        }
    }

    pub fn finish(&mut self, callback: Option<&StreamCallback<'_>>) {
        if self.in_reasoning {
            self.in_reasoning = false;
            self.push("</think>", callback);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    #[test]
    fn forwards_text_before_stream_finishes_without_empty_reasoning() {
        let events = Mutex::new(Vec::new());
        let callback = |event| events.lock().unwrap().push(event);
        let mut stream = StreamContent::default();
        stream.delta(&json!({"content":"", "reasoning_content":""}), Some(&callback));
        assert!(events.lock().unwrap().is_empty());
        stream.delta(&json!({"content":"## 雨夜"}), Some(&callback));
        assert_eq!(*events.lock().unwrap(), vec![StreamUpdate::Token("## 雨夜".into())]);
        stream.delta(&json!({"content":"琴声"}), Some(&callback));
        stream.finish(Some(&callback));
        assert_eq!(stream.text, "## 雨夜琴声");
    }

    #[test]
    fn streams_reasoning_and_text_without_exposing_tool_arguments() {
        let events = Mutex::new(Vec::new());
        let callback = |event| events.lock().unwrap().push(event);
        let mut stream = StreamContent::default();
        stream.delta(&json!({"reasoning_content":"分析"}), Some(&callback));
        stream.delta(&json!({"tool_calls":[{"function":{"arguments":"partial"}}]}), Some(&callback));
        stream.delta(&json!({"content":"回答"}), Some(&callback));
        stream.finish(Some(&callback));
        let tokens: String = events.lock().unwrap().iter().filter_map(|e| match e {
            StreamUpdate::Token(s) => Some(s.as_str()), _ => None,
        }).collect();
        assert_eq!(tokens, "<think>分析</think>回答");
        assert_eq!(stream.text, tokens);
    }

    #[test]
    fn closes_reasoning_only_response_and_supports_non_streaming_callers() {
        let mut stream = StreamContent::default();
        stream.delta(&json!({"reasoning_content":"分析"}), None);
        stream.finish(None);
        stream.finish(None);
        assert_eq!(stream.text, "<think>分析</think>");
    }
}
