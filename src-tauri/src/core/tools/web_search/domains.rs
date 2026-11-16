//! 垂域站点元数据注册表。
//!
//! 数据源：`data/domains.txt`（垂域站点，域名|分类|权重）与
//! `data/deny.txt`（黑名单，类型|域名），纯文本一行一条，`#` 开头或空行忽略。
//! 编译期 `include_str!` 嵌入二进制：老梁改文本 → 重编译 → 生效，不碰 Rust 代码。
//!
//! 数据与逻辑分离：**站点列表**（会变、人工维护）放文本文件；
//! **规则/模式**（权威后缀、字典标题标记等稳定算法量）仍留在 fusion.rs 代码里。

use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct Site {
    pub domain: String,
    /// 分类标签（military / academic / enterprise / books / general ...）
    pub categories: Vec<String>,
    /// 权重 0~1，人物/垂域查询排序时加成。
    pub weight: f64,
}

#[derive(Debug, Default)]
pub struct Registry {
    /// 人物/实体结构化源（域名 → 权重），person 查询时排序加成。
    pub entity_sources: Vec<(String, f64)>,
    /// 垂域站点全集（含分类），供 LLM 搜索扩展按分类注入。
    pub sites: Vec<Site>,
    /// 字典/拆字站黑名单。
    pub dict_hosts: Vec<String>,
    /// 低质文档站（降权）。
    pub low_quality_hosts: Vec<String>,
    /// 需登录/JS 渲染、不可抓正文的站。
    pub unfetchable: Vec<String>,
}

pub fn registry() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(load)
}

const DOMAINS_TXT: &str = include_str!("../data/domains.txt");
const DENY_TXT: &str = include_str!("../data/deny.txt");

fn load() -> Registry {
    let mut r = Registry::default();

    for line in DOMAINS_TXT.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.splitn(4, '|');
        let domain = it.next().unwrap_or("").trim();
        let cats = it.next().unwrap_or("").trim();
        let weight = it.next().unwrap_or("").trim().parse::<f64>().unwrap_or(0.0);
        if domain.is_empty() {
            continue;
        }
        let categories: Vec<String> = cats
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        r.sites.push(Site {
            domain: domain.to_string(),
            categories,
            weight,
        });
        if weight > 0.0 {
            r.entity_sources.push((domain.to_string(), weight));
        }
    }

    for line in DENY_TXT.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (ty, domain) = match line.split_once('|') {
            Some((t, d)) => (t.trim(), d.trim().to_string()),
            None => ("", line.to_string()),
        };
        if domain.is_empty() {
            continue;
        }
        match ty {
            "dict" => r.dict_hosts.push(domain),
            "low" => r.low_quality_hosts.push(domain),
            "unfetchable" => r.unfetchable.push(domain),
            _ => {}
        }
    }

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 垂域站点已加载() {
        let r = registry();
        assert!(r.sites.iter().any(|s| s.domain == "aiqicha.baidu.com"));
        assert!(r.entity_sources.iter().any(|(d, _)| d == "cnki.net"));
        // 分类被解析
        let aiqicha = r.sites.iter().find(|s| s.domain == "aiqicha.baidu.com").unwrap();
        assert!(aiqicha.categories.contains(&"enterprise".to_string()));
    }

    #[test]
    fn 黑名单按类型归类() {
        let r = registry();
        assert!(r.dict_hosts.contains(&"hanyu.baidu.com".to_string()));
        assert!(r.low_quality_hosts.contains(&"wenku.baidu.com".to_string()));
        assert!(r.unfetchable.contains(&"tianyancha.com".to_string()));
        // 类型不串：字典站不该进 unfetchable
        assert!(!r.unfetchable.contains(&"hanyu.baidu.com".to_string()));
    }
}
