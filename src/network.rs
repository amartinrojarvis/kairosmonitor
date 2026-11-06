//! A reusable native HTTP session. Bounded streaming, no curl subprocesses or disk cache.
use crate::{config::Config, model::Sample};
use objc2::{
    AnyThread, DefinedClass, define_class, msg_send, rc::Retained, runtime::ProtocolObject,
};
use objc2_foundation::*;
use std::sync::{Mutex, mpsc};

const MAX_BYTES: usize = 2 * 1024 * 1024;
struct Transfer {
    id: usize,
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
                if response.expectedContentLength() > MAX_BYTES as i64 {
                    t.error = Some("La respuesta supera 2 MiB".into());
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
                if t.bytes.len().saturating_add(data.length()) > MAX_BYTES {
                    t.error = Some("La respuesta supera 2 MiB".into());
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
        Self { session, delegate }
    }
    pub fn fetch(&self, c: &Config) -> Result<Sample, String> {
        let url = NSURL::URLWithString(&NSString::from_str(&c.url)).ok_or("URL no válida")?;
        let request = NSURLRequest::requestWithURL_cachePolicy_timeoutInterval(
            &url,
            NSURLRequestCachePolicy::ReloadIgnoringLocalCacheData,
            c.timeout as f64,
        );
        let (sender, receiver) = mpsc::channel();
        let task = self.session.dataTaskWithRequest(&request);
        *self.delegate.ivars().transfer.lock().unwrap() = Some(Transfer {
            id: task.taskIdentifier(),
            bytes: Vec::with_capacity(8192),
            error: None,
            sender,
        });
        task.resume();
        // A total deadline also covers a server that keeps dribbling partial data.
        let result = receiver.recv_timeout(std::time::Duration::from_secs(c.timeout));
        match result {
            Ok(Ok(bytes)) => Sample::parse(&bytes, &c.service),
            Ok(Err(e)) => Err(format!("Kairos no responde · {e}")),
            Err(_) => {
                task.cancel();
                self.delegate.ivars().transfer.lock().unwrap().take();
                Err("Kairos no responde · tiempo de espera agotado".into())
            }
        }
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
                    "Kairos no responde",
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
