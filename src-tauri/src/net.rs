//! Tiny HTTPS GET over WinHTTP: uses the system's proxy settings and certificate store, and
//! adds no network stack of our own. Header values are never logged (one carries a token).

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Networking::WinHttp::*;

pub struct Response {
    pub status: u32,
    pub body: Vec<u8>,
}

struct Handle(*mut core::ffi::c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

fn err(what: &str) -> String {
    format!("{what} failed ({})", windows::core::Error::from_win32().code().0 & 0xFFFF)
}

/// GET `url` with extra `headers`, giving up after `timeout_ms` per step.
pub fn get(url: &str, headers: &[(&str, &str)], timeout_ms: i32) -> Result<Response, String> {
    let (secure, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err("unsupported URL".into());
    };
    let (hostport, path) = rest.split_once('/').map(|(h, p)| (h, format!("/{p}"))).unwrap_or((rest, "/".into()));
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|_| "bad port")?),
        None => (hostport, if secure { 443 } else { 80 }),
    };

    unsafe {
        let agent = HSTRING::from(format!("Covebar/{}", env!("CARGO_PKG_VERSION")));
        let session = Handle(WinHttpOpen(
            &agent,
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        ));
        if session.0.is_null() {
            return Err(err("open"));
        }
        let _ = WinHttpSetTimeouts(session.0, timeout_ms, timeout_ms, timeout_ms, timeout_ms);
        // Race IPv4 against IPv6 like browsers do. On networks where IPv6 is advertised but
        // goes nowhere, WinHTTP otherwise waits on it until the request times out (12002).
        let on: u32 = 1;
        let _ = WinHttpSetOption(Some(session.0), WINHTTP_OPTION_IPV6_FAST_FALLBACK, Some(std::slice::from_raw_parts(&on as *const u32 as *const u8, 4)));
        let conn = Handle(WinHttpConnect(session.0, &HSTRING::from(host), port, 0));
        if conn.0.is_null() {
            return Err(err("connect"));
        }
        let req = Handle(WinHttpOpenRequest(
            conn.0,
            &HSTRING::from("GET"),
            &HSTRING::from(path),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            if secure { WINHTTP_FLAG_SECURE } else { WINHTTP_OPEN_REQUEST_FLAGS(0) },
        ));
        if req.0.is_null() {
            return Err(err("request"));
        }
        let mut all = String::new();
        for (k, v) in headers {
            all.push_str(k);
            all.push_str(": ");
            all.push_str(v);
            all.push_str("\r\n");
        }
        let hdr: Vec<u16> = all.encode_utf16().collect();
        WinHttpSendRequest(req.0, (!hdr.is_empty()).then_some(&hdr[..]), None, 0, 0, 0).map_err(|_| err("send"))?;
        WinHttpReceiveResponse(req.0, std::ptr::null_mut()).map_err(|_| err("receive"))?;

        let mut status = 0u32;
        let mut len = 4u32;
        WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some(&mut status as *mut u32 as *mut _),
            &mut len,
            std::ptr::null_mut(),
        )
        .map_err(|_| err("status"))?;

        let mut body = Vec::new();
        loop {
            let mut avail = 0u32;
            WinHttpQueryDataAvailable(req.0, &mut avail).map_err(|_| err("read"))?;
            if avail == 0 {
                break;
            }
            let start = body.len();
            body.resize(start + avail as usize, 0);
            let mut read = 0u32;
            WinHttpReadData(req.0, body[start..].as_mut_ptr() as *mut _, avail, &mut read).map_err(|_| err("read"))?;
            body.truncate(start + read as usize);
            if body.len() > 4 << 20 {
                return Err("response too large".into());
            }
        }
        Ok(Response { status, body })
    }
}

pub fn get_json(url: &str, headers: &[(&str, &str)]) -> Result<(u32, serde_json::Value), String> {
    let r = get(url, headers, 10_000)?;
    let v = serde_json::from_slice(&r.body).map_err(|_| format!("bad response (HTTP {})", r.status))?;
    Ok((r.status, v))
}
