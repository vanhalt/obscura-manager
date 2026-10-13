use serde_json::{Value, json};
use std::time::Duration;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CdpTarget {
    pub target_id: String,
    pub kind: String,
    pub title: String,
    pub url: String,
}

pub const CDP_METHODS: &[(&str, &str)] = &[
    ("Target.getTargets", "List all targets"),
    ("Target.createTarget", "Create new target {url}"),
    ("Target.closeTarget", "Close target {targetId}"),
    ("Target.attachToTarget", "Attach {targetId, flatten}"),
    ("Target.createBrowserContext", "Create browser context"),
    (
        "Target.disposeBrowserContext",
        "Dispose context {browserContextId}",
    ),
    ("Page.navigate", "Navigate {url}"),
    ("Page.getFrameTree", "Frame tree"),
    ("Page.captureScreenshot", "Screenshot {format}"),
    ("Page.startScreencast", "Start screencast"),
    ("Page.stopScreencast", "Stop screencast"),
    ("Page.printToPDF", "Print to PDF"),
    ("Page.enable", "Enable Page domain"),
    ("Runtime.evaluate", "Evaluate {expression}"),
    ("Runtime.callFunctionOn", "Call function on object"),
    ("Runtime.getProperties", "Get properties {objectId}"),
    ("Runtime.enable", "Enable Runtime"),
    ("DOM.getDocument", "Get document"),
    ("DOM.querySelector", "Query selector {nodeId, selector}"),
    ("DOM.querySelectorAll", "Query all {nodeId, selector}"),
    ("DOM.getOuterHTML", "Outer HTML {nodeId}"),
    ("Network.enable", "Enable Network"),
    ("Network.setCookies", "Set cookies"),
    ("Network.getCookies", "Get cookies {urls}"),
    ("Network.setExtraHTTPHeaders", "Set headers {headers}"),
    ("Network.setUserAgentOverride", "UA override {userAgent}"),
    ("Fetch.enable", "Enable Fetch interception"),
    ("Fetch.continueRequest", "Continue {requestId}"),
    ("Fetch.fulfillRequest", "Fulfill {requestId, responseCode}"),
    ("Fetch.failRequest", "Fail {requestId}"),
    ("Fetch.takeResponseBodyAsStream", "Stream body {requestId}"),
    ("IO.read", "Read stream {handle}"),
    ("IO.close", "Close stream {handle}"),
    ("Storage.getCookies", "Storage cookies"),
    ("Storage.setCookies", "Storage set cookies"),
    ("Storage.deleteCookies", "Delete cookies"),
    ("Input.dispatchMouseEvent", "Mouse event"),
    ("Input.dispatchKeyEvent", "Key event"),
    ("LP.getMarkdown", "DOM to Markdown"),
];

pub fn default_params(method: &str) -> Value {
    match method {
        "Target.createTarget" => json!({"url": "about:blank"}),
        "Page.navigate" => json!({"url": "https://example.com"}),
        "Page.captureScreenshot" => json!({"format": "png"}),
        "Runtime.evaluate" => json!({"expression": "document.title", "returnByValue": true}),
        "DOM.querySelector" => json!({"nodeId": 1, "selector": "body"}),
        "DOM.querySelectorAll" => json!({"nodeId": 1, "selector": "a"}),
        "Network.getCookies" => json!({"urls": ["https://example.com"]}),
        _ => json!({}),
    }
}

// Synchronous CDP client using tungstenite
pub struct CdpClient {
    pub ws: tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    next_id: u64,
    pub session_id: Option<String>,
}

impl CdpClient {
    pub fn connect(host: &str, port: u16, token: Option<&str>) -> anyhow::Result<Self> {
        let url = format!("ws://{host}:{port}/devtools/browser");
        use tungstenite::client::IntoClientRequest;
        let mut req = url.into_client_request()?;
        if let Some(t) = token
            && !t.is_empty()
        {
            req.headers_mut()
                .insert("Authorization", format!("Bearer {t}").parse()?);
        }
        let (ws, _) = tungstenite::connect(req)?;
        let mut client = Self {
            ws,
            next_id: 0,
            session_id: None,
        };
        client.ensure_session()?;
        Ok(client)
    }
    /// Establish a page session: reuse an existing page target via attach,
    /// otherwise create one and use its managed `{targetId}-session` id
    /// (verified against obscura 0.2.4: createTarget returns only targetId,
    /// navigate/evaluate/screenshot require that session id).
    pub fn ensure_session(&mut self) -> anyhow::Result<String> {
        if let Some(s) = &self.session_id {
            return Ok(s.clone());
        }
        let targets = self.get_targets()?;
        let session = if let Some(page) = targets.iter().find(|t| t.kind == "page") {
            let r = self.call_session(
                "Target.attachToTarget",
                json!({"targetId": page.target_id, "flatten": true}),
                None,
            )?;
            r.get("sessionId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}-session", page.target_id))
        } else {
            let r =
                self.call_session("Target.createTarget", json!({"url": "about:blank"}), None)?;
            let tid = r
                .get("targetId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if tid.is_empty() {
                anyhow::bail!("CDP createTarget returned no targetId");
            }
            format!("{tid}-session")
        };
        // Page.enable is best-effort on the new session.
        let _ = self.call_session("Page.enable", json!({}), Some(session.clone()));
        self.session_id = Some(session.clone());
        Ok(session)
    }
    pub fn call(&mut self, method: &str, params: Value) -> anyhow::Result<Value> {
        // Browser-level Target.* calls carry no session; page-scoped calls do.
        let session = if method.starts_with("Target.") {
            None
        } else {
            self.session_id.clone()
        };
        self.call_session(method, params, session)
    }
    pub fn call_session(
        &mut self,
        method: &str,
        params: Value,
        session: Option<String>,
    ) -> anyhow::Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let mut msg = json!({"id": id, "method": method, "params": params});
        if let Some(s) = session {
            msg["sessionId"] = json!(s);
        }
        self.ws.send(tungstenite::Message::Text(msg.to_string()))?;
        // wait for matching id, collect events in between (caller discards for now)
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if std::time::Instant::now() > deadline {
                anyhow::bail!("CDP timeout waiting for {method}");
            }
            match self.ws.read() {
                Ok(tungstenite::Message::Text(txt)) => {
                    let v: Value = serde_json::from_str(&txt)?;
                    if v.get("id").and_then(|x| x.as_u64()) == Some(id) {
                        if let Some(err) = v.get("error") {
                            anyhow::bail!("CDP error: {err}");
                        }
                        return Ok(v.get("result").cloned().unwrap_or(json!({})));
                    }
                    // event: ignore / could be buffered by caller in future
                    continue;
                }
                Ok(tungstenite::Message::Close(_)) => anyhow::bail!("CDP connection closed"),
                Ok(_) => continue,
                Err(tungstenite::Error::Io(e))
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
    pub fn get_targets(&mut self) -> anyhow::Result<Vec<CdpTarget>> {
        let r = self.call("Target.getTargets", json!({}))?;
        let mut out = Vec::new();
        if let Some(arr) = r.get("targetInfos").and_then(|v| v.as_array()) {
            for t in arr {
                out.push(CdpTarget {
                    target_id: t
                        .get("targetId")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    kind: t
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    title: t
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    url: t
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                });
            }
        }
        Ok(out)
    }
}

pub const MCP_TOOLS: &[(&str, &str)] = &[
    ("browser_navigate", "Navigate to URL"),
    ("browser_snapshot", "Page snapshot with element refs"),
    ("browser_screenshot", "Screenshot PNG (render build)"),
    ("browser_pdf", "PDF export (render build)"),
    ("browser_click", "Click by ref/selector"),
    ("browser_fill", "Fill input"),
    ("browser_type", "Type text"),
    ("browser_press_key", "Press key"),
    ("browser_select_option", "Select option"),
    ("browser_evaluate", "Evaluate JS"),
    ("browser_wait_for", "Wait for selector"),
    ("browser_network_requests", "Network requests"),
    ("browser_console_messages", "Console messages"),
    ("browser_close", "Close page"),
    ("browser_back", "Back"),
    ("browser_forward", "Forward"),
    ("browser_reload", "Reload"),
    ("browser_markdown", "Page as markdown"),
    ("browser_links", "Link list"),
    ("browser_extract", "Structured extract"),
    ("browser_get_cookies", "Get cookies"),
    ("browser_set_cookie", "Set cookie"),
    ("browser_clear_cookies", "Clear cookies"),
    ("browser_tab_new", "New tab"),
    ("browser_tab_list", "List tabs"),
    ("browser_tab_switch", "Switch tab"),
    ("browser_tab_close", "Close tab"),
];
