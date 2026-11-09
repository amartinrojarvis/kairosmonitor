//! A reusable native HTTP session. Bounded streaming, no curl subprocesses or disk cache.
use crate::{config::Config, model::Sample};
use objc2::{
    AnyThread, DefinedClass, define_class, msg_send, rc::Retained, runtime::ProtocolObject,
};
use objc2_foundation::*;
use std::{
    cell::RefCell,
    sync::{Mutex, mpsc},
};

const MAX_BYTES: usize = 2 * 1024 * 1024;
struct Transfer {
    id: usize,
    limit: usize,
    bytes: Vec<u8>,
    error: Option<String>,
    sender: mpsc::Sender<Result<Vec<u8>, String>>,
}
#[derive(Default)]
struct NetworkIvars {
    transfer: Mutex<Option<Transfer>>,
}
define_class!(
    // NSURLSession invokes callbacks on its serial delegate queue. Shared state is locked.
    #[unsafe(super = NSObject)]
    #[ivars = NetworkIvars]
    struct NetworkDelegate;
    unsafe impl NSObjectProtocol for NetworkDelegate {}
    unsafe impl NSURLSessionDelegate for NetworkDelegate {}
    unsafe impl NSURLSessionTaskDelegate for NetworkDelegate {
        #[unsafe(method(URLSession:task:didCompleteWithError:))]
        fn complete(
            &self,
            _session: &NSURLSession,
            task: &NSURLSessionTask,
            error: Option<&NSError>,
        ) {
            let mut guard = self.ivars().transfer.lock().unwrap();
            if guard
                .as_ref()
                .is_some_and(|t| t.id == task.taskIdentifier())
            {
                let transfer = guard.take().unwrap();
                let error = transfer
                    .error
                    .or_else(|| error.map(|e| e.localizedDescription().to_string()));
                let result = error.map_or_else(|| Ok(transfer.bytes), Err);
                let _ = transfer.sender.send(result);
            }
        }
    }
    unsafe impl NSURLSessionDataDelegate for NetworkDelegate {
        #[unsafe(method(URLSession:dataTask:didReceiveResponse:completionHandler:))]
        unsafe fn response(
            &self,
            _session: &NSURLSession,
            task: &NSURLSessionDataTask,
            response: &NSURLResponse,
            handler: &block2::DynBlock<dyn Fn(NSURLSessionResponseDisposition)>,
        ) {
            let mut guard = self.ivars().transfer.lock().unwrap();
            let allow = if let Some(t) = guard.as_mut().filter(|t| t.id == task.taskIdentifier()) {
                if response.expectedContentLength() > t.limit as i64 {
                    t.error = Some(format!("La respuesta supera {} MiB", t.limit / 1024 / 1024));
                } else if let Some(http) = response.downcast_ref::<NSHTTPURLResponse>() {
                    if !(200..300).contains(&http.statusCode()) {
                        t.error = Some(format!("Error HTTP {}", http.statusCode()));
                    }
                } else {
                    t.error = Some("Respuesta HTTP no válida".into());
                }
                t.error.is_none()
            } else {
                false
            };
            drop(guard);
            handler.call((if allow {
                NSURLSessionResponseDisposition::Allow
            } else {
                NSURLSessionResponseDisposition::Cancel
            },));
        }
        #[unsafe(method(URLSession:dataTask:didReceiveData:))]
        fn data(&self, _session: &NSURLSession, task: &NSURLSessionDataTask, data: &NSData) {
            let mut guard = self.ivars().transfer.lock().unwrap();
            if let Some(t) = guard.as_mut().filter(|t| t.id == task.taskIdentifier()) {
                if t.bytes.len().saturating_add(data.length()) > t.limit {
                    t.error = Some(format!("La respuesta supera {} MiB", t.limit / 1024 / 1024));
                    task.cancel();
                } else if t.error.is_none() {
                    // The immutable NSData is alive for the full callback; bytes are copied now.
                    t.bytes
                        .extend_from_slice(unsafe { data.as_bytes_unchecked() });
                }
            }
        }
    }
);
pub struct Client {
    session: Retained<NSURLSession>,
    delegate: Retained<NetworkDelegate>,
    endpoint: RefCell<Option<(String, String)>>,
    usage_cache: RefCell<Option<(String, std::time::Instant, serde_json::Value)>>,
}
impl Client {
    pub fn new() -> Self {
        let delegate = NetworkDelegate::alloc().set_ivars(NetworkIvars::default());
        let delegate: Retained<NetworkDelegate> = unsafe { msg_send![super(delegate), init] };
        let config = NSURLSessionConfiguration::ephemeralSessionConfiguration();
        config.setURLCache(None);
        config.setHTTPCookieStorage(None);
        config.setHTTPShouldSetCookies(false);
        config.setHTTPMaximumConnectionsPerHost(1);
        config.setRequestCachePolicy(NSURLRequestCachePolicy::ReloadIgnoringLocalCacheData);
        let session = unsafe {
            NSURLSession::sessionWithConfiguration_delegate_delegateQueue(
                &config,
                Some(ProtocolObject::from_ref(&*delegate)),
                None,
            )
        };
        Self {
            session,
            delegate,
            endpoint: RefCell::new(None),
            usage_cache: RefCell::new(None),
        }
    }
    pub fn probe(&self, c: &Config) -> Result<Sample, String> {
        self.endpoint.borrow_mut().take();
        self.fetch(c)
    }
    pub fn fetch(&self, c: &Config) -> Result<Sample, String> {
        let cached = self
            .endpoint
            .borrow()
            .as_ref()
            .filter(|(input, _)| input == &c.url)
            .map(|(_, endpoint)| endpoint.clone());
        let endpoints = if let Some(endpoint) = cached {
            vec![endpoint]
        } else {
            endpoint_candidates(&c.url)?
        };
        let mut last_error = String::from("Sin respuesta compatible");
        for endpoint in endpoints {
            let bytes = match self.get(&endpoint, c.timeout) {
                Ok(bytes) => bytes,
                Err(e) if e.contains("HTTP 404") || e.contains("HTTP 405") => { last_error = e; continue; }
                Err(e) if e.contains("HTTP 401") || e.contains("HTTP 403") => return Err("El servidor requiere autorización (HTTP 401/403). Esta versión admite endpoints accesibles sin clave API.".into()),
                Err(e) => return Err(e),
            };
            let host = NSURL::URLWithString(&NSString::from_str(&endpoint))
                .and_then(|url| url.host())
                .map(|h| h.to_string())
                .unwrap_or_default();
            let sample = Sample::parse(&bytes, "auto").or_else(|kairos_error| {
                Sample::from_openai(&bytes, &host).map_err(|_| kairos_error)
            });
            match sample {
                Ok(mut sample) => {
                    if sample.kind == crate::model::ServerKind::Kairos
                        && let Some(base) = endpoint.strip_suffix("/api/metrics")
                    {
                        sample.data["chatgptUsage"] = self.fetch_usage(base, c.timeout);
                    }
                    sample.endpoint = endpoint.clone();
                    *self.endpoint.borrow_mut() = Some((c.url.clone(), endpoint));
                    return Ok(sample);
                }
                Err(e) => last_error = e,
            }
        }
        Err(format!(
            "No se detectó un endpoint de Kairos ni un catálogo /v1/models · {last_error}"
        ))
    }
    fn fetch_usage(&self, base: &str, timeout: u64) -> serde_json::Value {
        if let Some((key, fetched, data)) = self.usage_cache.borrow().as_ref()
            && key == base
            && fetched.elapsed().as_secs() < 15
        {
            return data.clone();
        }
        let data = self
            .get(&format!("{base}/api/chatgpt/usage-limits"), timeout)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .filter(|v| v.is_object())
            .unwrap_or_else(|| serde_json::json!({"ok":false}));
        *self.usage_cache.borrow_mut() =
            Some((base.into(), std::time::Instant::now(), data.clone()));
        data
    }
    fn get(&self, endpoint: &str, timeout: u64) -> Result<Vec<u8>, String> {
        self.get_bounded(endpoint, timeout, MAX_BYTES)
    }
    pub fn get_bounded(
        &self,
        endpoint: &str,
        timeout: u64,
        limit: usize,
    ) -> Result<Vec<u8>, String> {
        let url = NSURL::URLWithString(&NSString::from_str(endpoint)).ok_or("URL no válida")?;
        let request = NSURLRequest::requestWithURL_cachePolicy_timeoutInterval(
            &url,
            NSURLRequestCachePolicy::ReloadIgnoringLocalCacheData,
            timeout as f64,
        );
        let (sender, receiver) = mpsc::channel();
        let task = self.session.dataTaskWithRequest(&request);
        *self.delegate.ivars().transfer.lock().unwrap() = Some(Transfer {
            id: task.taskIdentifier(),
            limit,
            bytes: Vec::with_capacity(8192),
            error: None,
            sender,
        });
        task.resume();
        // A total deadline also covers a server that keeps dribbling partial data.
        let result = receiver.recv_timeout(std::time::Duration::from_secs(timeout));
        match result {
            Ok(Ok(bytes)) => Ok(bytes),
            Ok(Err(e)) => Err(format!("El servidor no responde · {e}")),
            Err(_) => {
                task.cancel();
                self.delegate.ivars().transfer.lock().unwrap().take();
                Err("El servidor no responde · tiempo de espera agotado".into())
            }
        }
    }
}
fn endpoint_candidates(input: &str) -> Result<Vec<String>, String> {
    let raw = input.trim();
    let url = NSURL::URLWithString(&NSString::from_str(raw)).ok_or("URL no válida")?;
    if url.host().is_none() {
        return Err("La URL debe incluir un servidor".into());
    }
    let path = url.path().map(|p| p.to_string()).unwrap_or_default();
    let base = raw.trim_end_matches('/');
    if url.query().is_some() || url.fragment().is_some() {
        return Ok(vec![raw.into()]);
    }
    if path.is_empty() || path == "/" {
        Ok(vec![
            format!("{base}/api/metrics"),
            format!("{base}/v1/models"),
        ])
    } else if path.trim_end_matches('/').ends_with("/v1") {
        Ok(vec![format!("{base}/models")])
    } else {
        Ok(vec![raw.into()])
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.session.invalidateAndCancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    fn server(response: Vec<u8>, delay: Duration) -> (Config, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let c = Config {
            url: format!("http://{}/metrics", listener.local_addr().unwrap()),
            timeout: 1,
            ..Config::default()
        };
        let thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                if stream.read_exact(&mut byte).is_err() {
                    return;
                }
                request.push(byte[0]);
            }
            thread::sleep(delay);
            let _ = stream.write_all(&response);
        });
        (c, thread)
    }

    #[test]
    fn native_http_rejects_errors_oversize_and_timeouts() {
        objc2::rc::autoreleasepool(|_| {
            let client = Client::new();
            for (bytes, delay, expected) in [
                (
                    b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n".to_vec(),
                    Duration::ZERO,
                    "503",
                ),
                (
                    b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nbad".to_vec(),
                    Duration::ZERO,
                    "JSON",
                ),
                (
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                        MAX_BYTES + 1,
                        "x".repeat(MAX_BYTES + 1)
                    )
                    .into_bytes(),
                    Duration::ZERO,
                    "2 MiB",
                ),
                (
                    Vec::new(),
                    Duration::from_millis(1200),
                    "El servidor no responde",
                ),
            ] {
                let (c, thread) = server(bytes, delay);
                let error = client.fetch(&c).unwrap_err();
                thread.join().unwrap();
                assert!(error.contains(expected), "expected {expected}: {error}");
            }
            // A chunked response with no declared size must also be bounded.
            let body = vec![b'x'; MAX_BYTES + 1];
            let mut response = format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n",
                body.len()
            )
            .into_bytes();
            response.extend(body);
            response.extend(b"\r\n0\r\n\r\n");
            let (c, thread) = server(response, Duration::ZERO);
            assert!(client.fetch(&c).unwrap_err().contains("2 MiB"));
            thread.join().unwrap();
        });
    }

    #[test]
    fn discovers_openai_from_base_url_and_reuses_resolved_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let c = Config {
            url: format!("http://{}", listener.local_addr().unwrap()),
            timeout: 2,
            ..Config::default()
        };
        let server = thread::spawn(move || {
            for expected in ["/api/metrics", "/v1/models", "/v1/models"] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                while !bytes.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                assert!(
                    String::from_utf8(bytes)
                        .unwrap()
                        .starts_with(&format!("GET {expected} "))
                );
                let (status, body) = if expected == "/api/metrics" {
                    ("404 Not Found", "")
                } else {
                    (
                        "200 OK",
                        r#"{"object":"list","data":[{"id":"detected-model"}]}"#,
                    )
                };
                write!(stream, "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        objc2::rc::autoreleasepool(|_| {
            let client = Client::new();
            for _ in 0..2 {
                let s = client.fetch(&c).unwrap();
                assert_eq!(s.models, ["detected-model"]);
                assert_eq!(s.kind, crate::model::ServerKind::OpenAI);
            }
        });
        server.join().unwrap();
    }
    #[test]
    fn weekly_usage_is_cached_and_failure_does_not_hide_metrics() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let c = Config {
            url: format!("{base}/api/metrics"),
            timeout: 1,
            ..Config::default()
        };
        let server = thread::spawn(move || {
            for expected in ["/api/metrics", "/api/chatgpt/usage-limits", "/api/metrics"] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                assert!(
                    String::from_utf8(request)
                        .unwrap()
                        .starts_with(&format!("GET {expected} "))
                );
                let body = if expected.ends_with("usage-limits") {
                    include_str!("../tests/fixtures/usage.json")
                } else {
                    r#"{"memory":{"percent":20}}"#
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        objc2::rc::autoreleasepool(|_| {
            let client = Client::new();
            for _ in 0..2 {
                let sample = client.fetch(&c).unwrap();
                assert_eq!(sample.n("/memory/percent"), 20.);
                assert_eq!(crate::usage::used(sample.usage()), 92.);
            }
            // Expire the cache. An unavailable auxiliary endpoint cannot invalidate telemetry.
            *client.usage_cache.borrow_mut() = None;
            assert_eq!(crate::usage::used(&client.fetch_usage(&base, 1)), -1.);
        });
        server.join().unwrap();
    }
    #[test]
    fn endpoint_resolution_preserves_explicit_urls_and_prefixes() {
        objc2::rc::autoreleasepool(|_| {
            assert_eq!(
                endpoint_candidates("http://host:8000/").unwrap(),
                ["http://host:8000/api/metrics", "http://host:8000/v1/models"]
            );
            assert_eq!(
                endpoint_candidates("https://host/proxy/v1/").unwrap(),
                ["https://host/proxy/v1/models"]
            );
            assert_eq!(
                endpoint_candidates("https://host/custom/metrics?key=value").unwrap(),
                ["https://host/custom/metrics?key=value"]
            );
        });
    }
    #[test]
    fn native_http_reuses_connection_and_reads_fresh_data() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let c = Config {
            url: format!("http://{}/metrics", listener.local_addr().unwrap()),
            timeout: 2,
            ..Config::default()
        };
        let server = thread::spawn(move || {
            // Both requests must arrive over the same TCP connection.
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            for value in [20, 30] {
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                let body = format!(r#"{{"memory":{{"percent":{value}}}}}"#);
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        objc2::rc::autoreleasepool(|_| {
            let client = Client::new();
            assert_eq!(client.fetch(&c).unwrap().n("/memory/percent"), 20.);
            assert_eq!(client.fetch(&c).unwrap().n("/memory/percent"), 30.);
        });
        server.join().unwrap();
    }
}
