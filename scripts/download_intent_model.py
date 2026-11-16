#!/usr/bin/env python3
"""
下载 BERT 意图分类模型（DistilBERT 多语言）并保存为 FastClaw 可用格式。

用法:
    python scripts/download_intent_model.py

输出目录:
    {app_data_dir}/intent-model/
    - config.json         (~1 KB)   BERT 配置
    - model.safetensors   (~530 MB) 模型权重（DistilBERT base multilingual cased）
    - tokenizer.json      (~1 MB)   分词器

注意:
    - 需要安装: pip install transformers torch safetensors
    - 首次运行需联网下载
    - 输出约 530 MB，请确保磁盘空间充足
    - 如果磁盘空间受限，可改用 DistilBERT 小模型（见下方注释）
"""

import os
import sys
import json

MODEL_ID = "distilbert/distilbert-base-multilingual-cased"
# 轻量替代（约 70 MB）:
# MODEL_ID = "sentence-transformers/paraphrase-multilingual-MiniLM-L12-v2"
# 极小替代（约 17 MB）:
# MODEL_ID = "google/bert_uncased_L-2_H-128_A-2"

def get_target_dir():
    """获取模型目标目录"""
    # Windows: %APPDATA%/FastClaw/intent-model/
    # macOS/Linux: ~/.local/share/FastClaw/intent-model/ 或 $XDG_DATA_HOME
    if sys.platform == "win32":
        base = os.environ.get("APPDATA", os.path.expanduser("~"))
        return os.path.join(base, "FastClaw", "intent-model")
    else:
        data_home = os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
        return os.path.join(data_home, "FastClaw", "intent-model")


def download_model():
    """下载并保存模型"""
    target_dir = get_target_dir()
    os.makedirs(target_dir, exist_ok=True)

    config_path = os.path.join(target_dir, "config.json")
    weights_path = os.path.join(target_dir, "model.safetensors")
    tokenizer_path = os.path.join(target_dir, "tokenizer.json")

    # 检查是否已存在
    all_exist = all(os.path.exists(p) for p in [config_path, weights_path, tokenizer_path])
    if all_exist:
        print(f"✅ 模型文件已存在: {target_dir}")
        print("   如需重新下载，请删除目录后重试")
        return

    print(f"📦 下载模型: {MODEL_ID}")
    print(f"📁 目标目录: {target_dir}")

    try:
        from transformers import AutoModel, AutoTokenizer
    except ImportError:
        print("❌ 需要安装 transformers: pip install transformers torch safetensors")
        sys.exit(1)

    print("⏳ 加载模型（首次运行需要下载，预计 2-5 分钟）...")

    # 下载并加载
    model = AutoModel.from_pretrained(MODEL_ID)
    tokenizer = AutoTokenizer.from_pretrained(MODEL_ID)

    print("💾 保存 config.json ...")
    model.config.save_pretrained(target_dir)
    # 删除 pytorch_model.bin（如果有），只保留 safetensors
    bin_path = os.path.join(target_dir, "pytorch_model.bin")
    if os.path.exists(bin_path):
        os.remove(bin_path)

    print("💾 保存 model.safetensors ...")
    model.save_pretrained(target_dir, safe_serialization=True)

    print("💾 保存 tokenizer.json ...")
    tokenizer.save_pretrained(target_dir)

    # 验证
    files = os.listdir(target_dir)
    required = ["config.json", "model.safetensors", "tokenizer.json"]
    missing = [f for f in required if f not in files]

    if missing:
        print(f"⚠️ 缺少文件: {missing}")
        print(f"   已有文件: {files}")
    else:
        total_size = sum(
            os.path.getsize(os.path.join(target_dir, f)) for f in required
        )
        print(f"✅ 模型下载完成!")
        print(f"   目录: {target_dir}")
        print(f"   大小: {total_size / 1024 / 1024:.1f} MB")
        print(f"   重启 FastClaw 后 BERT 意图路由将自动生效")


if __name__ == "__main__":
    download_model()
