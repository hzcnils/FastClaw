//! BERT 模型加载与推理
//!
//! 使用 candle 加载 DistilBERT 多语言模型，提取句子的 [CLS] 嵌入向量。

use anyhow::{Context, Result};
use candle_core::{Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config, DTYPE};
use std::path::Path;
use tokenizers::Tokenizer;

/// BERT 模型包装器 —— 负责加载模型和提取句子嵌入
pub struct IntentModel {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
    hidden_size: usize,
}

impl IntentModel {
    /// 从模型目录加载 BERT 模型
    ///
    /// 目录需包含:
    /// - config.json —— BERT 配置
    /// - model.safetensors —— 模型权重
    /// - tokenizer.json —— HuggingFace 分词器
    pub fn load(model_dir: &Path) -> Result<Self> {
        let device = Device::Cpu;

        // 1. 加载配置
        let config_path = model_dir.join("config.json");
        let config_data = std::fs::read_to_string(&config_path)
            .with_context(|| format!("无法读取配置文件: {}", config_path.display()))?;
        let config: Config =
            serde_json::from_str(&config_data).with_context(|| "BERT 配置解析失败")?;
        let hidden_size = config.hidden_size;

        // 2. 加载分词器
        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("分词器加载失败: {e}"))?;

        // 3. 加载模型权重 (safetensors)
        let weights_path = model_dir.join("model.safetensors");
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[&weights_path], DTYPE, &device)
                .with_context(|| format!("模型权重加载失败: {}", weights_path.display()))?
        };
        let model = BertModel::load(vb.pp("bert"), &config)?;

        tracing::info!(
            "BERT 意图模型加载成功: {} (hidden_size={})",
            model_dir.display(),
            hidden_size,
        );

        Ok(Self {
            model,
            tokenizer,
            device,
            hidden_size,
        })
    }

    /// 获取文本的句子嵌入向量（[CLS] token 的 last hidden state）
    ///
    /// 返回 hidden_size 维 f32 向量（L2 归一化后）
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // 1. Tokenize
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("分词失败: {e}"))?;

        let token_ids = encoding.get_ids().to_vec();
        let attention_mask = encoding.get_attention_mask().to_vec();
        let seq_len = token_ids.len();

        // 2. 转为 Tensor [1, seq_len]
        let input_ids = Tensor::from_slice(&token_ids, (1, seq_len), &self.device)?;
        let attn_mask = Tensor::from_slice(&attention_mask, (1, seq_len), &self.device)?;
        let token_type_ids = Tensor::zeros((1, seq_len), DTYPE, &self.device)?;

        // 3. 前向传播
        let output = self.model.forward(
            &input_ids,
            // attention_mask: &Tensor (直接引用)
            &attn_mask,
            // token_type_ids: Option<&Tensor> (借用，不是 &Option)
            Some(&token_type_ids),
        )?;

        // 4. 提取 [CLS] token（第一个 token）的隐藏状态
        // output shape: [1, seq_len, hidden_size]
        let cls_embedding = output.i((0, 0))?; // [batch=0, token=0, hidden]

        // 5. L2 归一化
        let sqr_sum = cls_embedding.sqr()?.sum_all()?;
        let norm = sqr_sum.sqrt()?;
        let norm_scalar: f32 = norm.to_scalar()?;
        let normalized = if norm_scalar > 0.0 {
            cls_embedding.broadcast_div(&norm)?
        } else {
            cls_embedding
        };

        // 6. 转为 Vec<f32>
        let data = normalized.to_vec1()?;
        Ok(data)
    }

    /// 批量获取文本嵌入
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// 返回嵌入向量的维度
    pub fn embedding_dim(&self) -> usize {
        self.hidden_size
    }
}
