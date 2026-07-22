use std::{
    error::Error,
    fmt,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

type BackendResult<T> = Result<T, BackendError>;

#[derive(Debug)]
pub struct BackendError(String);

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for BackendError {}

impl From<&str> for BackendError {
    fn from(message: &str) -> Self {
        Self(message.to_owned())
    }
}

impl From<std::io::Error> for BackendError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<getrandom::Error> for BackendError {
    fn from(error: getrandom::Error) -> Self {
        Self(error.to_string())
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub data_root: PathBuf,
    pub port: u16,
    pub launch_secret: String,
}

impl RuntimeConfig {
    pub fn new(data_root: PathBuf) -> BackendResult<Self> {
        Ok(Self {
            data_root,
            port: available_loopback_port()?,
            launch_secret: random_launch_secret()?,
        })
    }

    pub fn launch_url(&self) -> String {
        format!(
            "http://127.0.0.1:{}/?launch_secret={}",
            self.port, self.launch_secret
        )
    }
}

pub struct BackendProcess {
    child: Mutex<Option<Child>>,
    runtime: RuntimeConfig,
}

impl BackendProcess {
    pub fn launch_development(runtime: RuntimeConfig) -> BackendResult<Self> {
        let command = development_rails_command(&runtime, project_root());
        Self::spawn(command, runtime)
    }

    fn spawn(mut command: Command, runtime: RuntimeConfig) -> BackendResult<Self> {
        command
            .env("LIBRAIRII_DATA_ROOT", &runtime.data_root)
            .env("LIBRAIRII_LAUNCH_SECRET", &runtime.launch_secret)
            .env("PORT", runtime.port.to_string())
            .env("RAILS_ENV", "development")
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let child = command.spawn()?;

        Ok(Self {
            child: Mutex::new(Some(child)),
            runtime,
        })
    }

    pub fn wait_until_healthy(&self, timeout: Duration) -> BackendResult<()> {
        let deadline = Instant::now() + timeout;

        loop {
            if self.has_exited()? {
                return Err("Rails exited before becoming healthy".into());
            }

            if health_request(&self.runtime)? {
                return Ok(());
            }

            if Instant::now() >= deadline {
                return Err("timed out waiting for the Rails health endpoint".into());
            }

            thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn stop(&self) {
        let Ok(mut child_slot) = self.child.lock() else {
            return;
        };
        let Some(mut child) = child_slot.take() else {
            return;
        };

        if child.try_wait().ok().flatten().is_none() {
            let _ = child.kill();
        }
        let _ = child.wait();
    }

    fn has_exited(&self) -> BackendResult<bool> {
        let mut child_slot = self
            .child
            .lock()
            .map_err(|_| "Rails child-process lock was poisoned")?;
        let child = child_slot
            .as_mut()
            .ok_or("Rails child process is not running")?;

        Ok(child.try_wait()?.is_some())
    }
}

impl Drop for BackendProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

fn project_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must be inside the project root")
}

fn development_rails_command(runtime: &RuntimeConfig, root: &Path) -> Command {
    let executable =
        std::env::var_os("LIBRAIRII_BUNDLE_EXECUTABLE").unwrap_or_else(|| "bundle".into());
    let mut command = Command::new(executable);
    command.current_dir(root).args([
        "exec",
        "rails",
        "server",
        "--environment",
        "development",
        "--binding",
        "127.0.0.1",
        "--port",
        &runtime.port.to_string(),
    ]);
    command
}

fn available_loopback_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

fn random_launch_secret() -> BackendResult<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn health_request(runtime: &RuntimeConfig) -> BackendResult<bool> {
    let address = SocketAddr::from(([127, 0, 0, 1], runtime.port));
    let mut stream = match TcpStream::connect_timeout(&address, Duration::from_millis(250)) {
        Ok(stream) => stream,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::TimedOut
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error.into()),
    };
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    write!(
        stream,
        "GET /health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",
        runtime.port, runtime.launch_secret
    )?;
    stream.flush()?;

    let mut status_line = String::new();
    BufReader::new(stream).read_line(&mut status_line)?;
    Ok(status_line.contains(" 200 "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::mpsc};

    fn runtime(port: u16) -> RuntimeConfig {
        RuntimeConfig {
            data_root: PathBuf::from("/tmp/librairii-test"),
            port,
            launch_secret: "test-secret".into(),
        }
    }

    #[test]
    fn reserves_an_available_loopback_port() {
        let port = available_loopback_port().unwrap();
        let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();

        assert_eq!(listener.local_addr().unwrap().ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn health_request_sends_the_launch_secret() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (request_sender, request_receiver) = mpsc::channel();

        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();

            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                request.push_str(&line);
            }

            request_sender.send(request).unwrap();
            let mut stream = stream;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .unwrap();
        });

        assert!(health_request(&runtime(port)).unwrap());
        assert!(
            request_receiver
                .recv()
                .unwrap()
                .contains("Authorization: Bearer test-secret")
        );
        server.join().unwrap();
    }

    #[test]
    fn stop_terminates_the_managed_child() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30"]);
        let process =
            BackendProcess::spawn(command, runtime(available_loopback_port().unwrap())).unwrap();

        process.stop();

        assert!(process.child.lock().unwrap().is_none());
    }

    #[test]
    fn development_command_targets_the_project_and_loopback_port() {
        let runtime = runtime(43210);
        let command = development_rails_command(&runtime, Path::new("/tmp/project"));
        let arguments: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy())
            .collect();

        assert_eq!(command.get_current_dir(), Some(Path::new("/tmp/project")));
        assert_eq!(
            arguments,
            [
                "exec",
                "rails",
                "server",
                "--environment",
                "development",
                "--binding",
                "127.0.0.1",
                "--port",
                "43210"
            ]
        );
    }

    #[test]
    fn development_backend_reaches_health_and_stops() {
        let data_root = std::env::temp_dir().join(format!(
            "librairii-tauri-smoke-{}",
            random_launch_secret().unwrap()
        ));
        let runtime = RuntimeConfig::new(data_root.clone()).unwrap();
        let process = BackendProcess::launch_development(runtime).unwrap();

        process.wait_until_healthy(Duration::from_secs(20)).unwrap();
        process.stop();

        assert!(process.child.lock().unwrap().is_none());
        fs::remove_dir_all(data_root).unwrap();
    }
}
