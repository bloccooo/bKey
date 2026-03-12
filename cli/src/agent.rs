use envilib::{error::{Error, Result}, store::cache_dir};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

const DEFAULT_TTL_SECS: u64 = 8 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentInfo {
    addr: String,
    token: String,
}

fn agent_file() -> PathBuf {
    cache_dir().join("agent.json")
}

fn read_agent_info() -> Option<AgentInfo> {
    let s = std::fs::read_to_string(agent_file()).ok()?;
    serde_json::from_str(&s).ok()
}

fn write_agent_file(info: &AgentInfo) -> std::io::Result<()> {
    let path = agent_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(info).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?
            .write_all(json.as_bytes())?;
    }
    #[cfg(not(unix))]
    std::fs::write(&path, json)?;
    Ok(())
}

// --- Protocol ---

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Request {
    Ping { token: String },
    GetKey { token: String, workspace_id: String },
    StoreKey { token: String, workspace_id: String, key: String },
    Kill { token: String },
}

#[derive(Serialize, Deserialize)]
struct Response {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl Response {
    fn ok() -> Self { Self { ok: true, key: None, error: None } }
    fn with_key(key: String) -> Self { Self { ok: true, key: Some(key), error: None } }
    fn err(msg: &str) -> Self { Self { ok: false, key: None, error: Some(msg.to_string()) } }
}

// --- Client ---

pub struct AgentClient {
    addr: String,
    token: String,
}

impl AgentClient {
    pub fn connect() -> Option<Self> {
        let info = read_agent_info()?;
        let client = Self { addr: info.addr, token: info.token };
        if client.ping() { Some(client) } else { None }
    }

    /// Connect to a running agent, starting one if necessary.
    pub fn connect_or_start() -> Option<Self> {
        if let Some(client) = Self::connect() {
            return Some(client);
        }
        let exe = std::env::current_exe().ok()?;
        std::process::Command::new(exe)
            .arg("agent")
            .arg("--serve")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(100));
            if let Some(client) = Self::connect() {
                return Some(client);
            }
        }
        None
    }

    fn request(&self, req: &Request) -> Option<Response> {
        let mut stream = TcpStream::connect(&self.addr).ok()?;
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
        let line = serde_json::to_string(req).ok()? + "\n";
        stream.write_all(line.as_bytes()).ok()?;
        let mut reader = BufReader::new(stream);
        let mut resp = String::new();
        reader.read_line(&mut resp).ok()?;
        serde_json::from_str(resp.trim()).ok()
    }

    fn ping(&self) -> bool {
        self.request(&Request::Ping { token: self.token.clone() })
            .map(|r| r.ok)
            .unwrap_or(false)
    }

    pub fn get_key(&self, workspace_id: &str) -> Option<[u8; 32]> {
        let resp = self.request(&Request::GetKey {
            token: self.token.clone(),
            workspace_id: workspace_id.to_string(),
        })?;
        if !resp.ok { return None; }
        let bytes = hex::decode(resp.key?).ok()?;
        bytes.try_into().ok()
    }

    pub fn store_key(&self, workspace_id: &str, key: &[u8; 32]) {
        let _ = self.request(&Request::StoreKey {
            token: self.token.clone(),
            workspace_id: workspace_id.to_string(),
            key: hex::encode(key),
        });
    }

    pub fn kill(&self) {
        let _ = self.request(&Request::Kill { token: self.token.clone() });
    }
}

// --- Command entry point ---

pub async fn run(serve: bool, kill: bool) -> Result<()> {
    if kill {
        match AgentClient::connect() {
            Some(client) => {
                client.kill();
                let _ = std::fs::remove_file(agent_file());
                println!("envi agent stopped");
            }
            None => eprintln!("no agent running"),
        }
        return Ok(());
    }

    if serve {
        return run_server().await;
    }

    // Check if already running
    if let Some(info) = read_agent_info() {
        let client = AgentClient { addr: info.addr, token: info.token };
        if client.ping() {
            println!("envi agent already running");
            return Ok(());
        }
    }

    // Spawn the server as a detached background process
    let exe = std::env::current_exe().map_err(|e| Error::Other(e.to_string()))?;
    std::process::Command::new(exe)
        .arg("agent")
        .arg("--serve")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| Error::Other(format!("failed to spawn agent: {e}")))?;

    // Wait for agent to write its socket file (up to 2s)
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(info) = read_agent_info() {
            let client = AgentClient { addr: info.addr, token: info.token };
            if client.ping() {
                println!("envi agent started");
                return Ok(());
            }
        }
    }

    Err(Error::Other("agent failed to start".to_string()))
}

async fn run_server() -> Result<()> {
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
    let addr = listener.local_addr()
        .map_err(|e| Error::Other(e.to_string()))?
        .to_string();

    let mut token_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut token_bytes);
    let token = hex::encode(token_bytes);

    write_agent_file(&AgentInfo { addr, token: token.clone() })
        .map_err(|e| Error::Other(e.to_string()))?;

    let keys: Arc<Mutex<HashMap<String, [u8; 32]>>> = Arc::new(Mutex::new(HashMap::new()));
    let last_activity: Arc<Mutex<std::time::Instant>> =
        Arc::new(Mutex::new(std::time::Instant::now()));

    // TTL watchdog
    let la = Arc::clone(&last_activity);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await;
            let elapsed = la.lock().unwrap().elapsed().as_secs();
            if elapsed > DEFAULT_TTL_SECS {
                let _ = std::fs::remove_file(agent_file());
                std::process::exit(0);
            }
        }
    });

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(handle_connection(
                    stream,
                    Arc::clone(&keys),
                    token.clone(),
                    Arc::clone(&last_activity),
                ));
            }
            Err(_) => continue,
        }
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    keys: Arc<Mutex<HashMap<String, [u8; 32]>>>,
    token: String,
    last_activity: Arc<Mutex<std::time::Instant>>,
) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    if reader.read_line(&mut line).await.is_err() {
        return;
    }

    let req: Request = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(_) => return,
    };

    let req_token = match &req {
        Request::Ping { token } | Request::GetKey { token, .. }
        | Request::StoreKey { token, .. } | Request::Kill { token } => token,
    };

    if req_token != &token {
        let _ = write_half.write_all(b"{\"ok\":false,\"error\":\"unauthorized\"}\n").await;
        return;
    }

    *last_activity.lock().unwrap() = std::time::Instant::now();

    let (resp, should_exit) = match req {
        Request::Ping { .. } => (Response::ok(), false),
        Request::GetKey { workspace_id, .. } => {
            let keys = keys.lock().unwrap();
            match keys.get(&workspace_id) {
                Some(key) => (Response::with_key(hex::encode(key)), false),
                None => (Response::err("not_found"), false),
            }
        }
        Request::StoreKey { workspace_id, key, .. } => {
            match hex::decode(&key).ok().and_then(|b| b.try_into().ok()) {
                Some(key_bytes) => {
                    keys.lock().unwrap().insert(workspace_id, key_bytes);
                    (Response::ok(), false)
                }
                None => (Response::err("invalid_key"), false),
            }
        }
        Request::Kill { .. } => (Response::ok(), true),
    };

    let resp_json = serde_json::to_string(&resp).unwrap() + "\n";
    let _ = write_half.write_all(resp_json.as_bytes()).await;

    if should_exit {
        let _ = std::fs::remove_file(agent_file());
        std::process::exit(0);
    }
}
