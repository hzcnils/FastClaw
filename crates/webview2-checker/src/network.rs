use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// 插件判断出的网络环境。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkEnvironment {
    /// 配置的内网探针可达。即使公网也可达，默认仍优先判为内网。
    Intranet,
    /// 内网探针不可达，但公网探针可达。
    Internet,
    /// 两类探针均不可达。
    Offline,
}

/// 单个网络探针。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkProbe {
    /// 建立 TCP 连接，例如 `192.168.1.10:7000` 或 `223.5.5.5:53`。
    Tcp(String),
    /// 仅验证主机名是否可以解析，例如 `packages.company.local`。
    Dns(String),
    /// 发送 HTTP HEAD 请求。为了保持零额外依赖，仅支持 `http://`。
    Http(String),
}

impl NetworkProbe {
    pub fn tcp(target: impl Into<String>) -> Self {
        Self::Tcp(target.into())
    }

    pub fn dns(host: impl Into<String>) -> Self {
        Self::Dns(host.into())
    }

    pub fn http(url: impl Into<String>) -> Self {
        Self::Http(url.into())
    }

    fn succeeds(&self, timeout: Duration) -> bool {
        match self {
            Self::Tcp(target) => connect_any(target, timeout).is_some(),
            Self::Dns(host) => (host.as_str(), 0)
                .to_socket_addrs()
                .map(|mut addresses| addresses.next().is_some())
                .unwrap_or(false),
            Self::Http(url) => http_head_succeeds(url, timeout),
        }
    }
}

/// 一组探针应如何得出“可达”。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbePolicy {
    /// 任一探针成功即可。
    Any,
    /// 所有探针都必须成功。
    All,
}

/// 可组合的内外网判断器。
#[derive(Clone, Debug)]
pub struct NetworkDetector {
    intranet_probes: Vec<NetworkProbe>,
    internet_probes: Vec<NetworkProbe>,
    intranet_policy: ProbePolicy,
    internet_policy: ProbePolicy,
    timeout: Duration,
}

impl Default for NetworkDetector {
    fn default() -> Self {
        Self {
            intranet_probes: Vec::new(),
            internet_probes: vec![
                NetworkProbe::tcp("8.8.8.8:53"),
                NetworkProbe::tcp("223.5.5.5:53"),
            ],
            intranet_policy: ProbePolicy::Any,
            internet_policy: ProbePolicy::Any,
            timeout: Duration::from_secs(2),
        }
    }
}

impl NetworkDetector {
    /// 创建一个没有预置探针的判断器。
    pub fn new() -> Self {
        Self {
            intranet_probes: Vec::new(),
            internet_probes: Vec::new(),
            intranet_policy: ProbePolicy::Any,
            internet_policy: ProbePolicy::Any,
            timeout: Duration::from_secs(2),
        }
    }

    pub fn intranet_probe(mut self, probe: NetworkProbe) -> Self {
        self.intranet_probes.push(probe);
        self
    }

    pub fn internet_probe(mut self, probe: NetworkProbe) -> Self {
        self.internet_probes.push(probe);
        self
    }

    pub fn intranet_policy(mut self, policy: ProbePolicy) -> Self {
        self.intranet_policy = policy;
        self
    }

    pub fn internet_policy(mut self, policy: ProbePolicy) -> Self {
        self.internet_policy = policy;
        self
    }

    pub fn timeout_secs(mut self, seconds: u64) -> Self {
        self.timeout = Duration::from_secs(seconds);
        self
    }

    /// 内网优先：内网探针命中时，即使公网可达也返回 `Intranet`。
    pub fn detect(&self) -> NetworkEnvironment {
        if probes_succeed(&self.intranet_probes, self.intranet_policy, self.timeout) {
            return NetworkEnvironment::Intranet;
        }
        if probes_succeed(&self.internet_probes, self.internet_policy, self.timeout) {
            return NetworkEnvironment::Internet;
        }
        NetworkEnvironment::Offline
    }
}

/// 兼容原有 API：使用默认公网探针判断是否联网。
pub fn is_online(timeout_secs: u64) -> bool {
    NetworkDetector::default()
        .timeout_secs(timeout_secs)
        .detect()
        == NetworkEnvironment::Internet
}

fn probes_succeed(probes: &[NetworkProbe], policy: ProbePolicy, timeout: Duration) -> bool {
    if probes.is_empty() {
        return false;
    }
    match policy {
        ProbePolicy::Any => probes.iter().any(|probe| probe.succeeds(timeout)),
        ProbePolicy::All => probes.iter().all(|probe| probe.succeeds(timeout)),
    }
}

fn connect_any(target: &str, timeout: Duration) -> Option<TcpStream> {
    let addresses = target.to_socket_addrs().ok()?;
    for address in addresses {
        if let Ok(stream) = TcpStream::connect_timeout(&address, timeout) {
            return Some(stream);
        }
    }
    None
}

fn http_head_succeeds(url: &str, timeout: Duration) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{path}")),
        None => (rest, "/".to_string()),
    };
    if authority.is_empty() || authority.contains('@') {
        return false;
    }

    let target = if authority.starts_with('[') {
        if authority.contains("]:") {
            authority.to_string()
        } else {
            format!("{authority}:80")
        }
    } else if authority.rsplit_once(':').is_some() {
        authority.to_string()
    } else {
        format!("{authority}:80")
    };

    let Some(mut stream) = connect_any(&target, timeout) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let request = format!("HEAD {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut response = [0_u8; 12];
    match stream.read(&mut response) {
        Ok(read) if read >= 5 => response.starts_with(b"HTTP/"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{NetworkDetector, NetworkEnvironment, NetworkProbe, ProbePolicy};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn empty_detector_is_offline() {
        assert_eq!(NetworkDetector::new().detect(), NetworkEnvironment::Offline);
    }

    #[test]
    fn invalid_http_scheme_is_rejected() {
        let detector = NetworkDetector::new()
            .intranet_probe(NetworkProbe::http("https://example.invalid"))
            .timeout_secs(0);
        assert_eq!(detector.detect(), NetworkEnvironment::Offline);
    }

    #[test]
    fn intranet_probe_has_priority() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let accept = thread::spawn(move || listener.accept().unwrap());

        let detector = NetworkDetector::new()
            .intranet_probe(NetworkProbe::tcp(address.to_string()))
            .internet_probe(NetworkProbe::tcp(address.to_string()));
        assert_eq!(detector.detect(), NetworkEnvironment::Intranet);
        accept.join().unwrap();
    }

    #[test]
    fn http_probe_accepts_an_http_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 256];
            let _ = stream.read(&mut request).unwrap();
            stream
                .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                .unwrap();
        });

        let detector = NetworkDetector::new()
            .intranet_probe(NetworkProbe::http(format!("http://{address}/health")))
            .intranet_policy(ProbePolicy::All);
        assert_eq!(detector.detect(), NetworkEnvironment::Intranet);
        server.join().unwrap();
    }
}
