#[macro_use]
extern crate log;
extern crate serde;
extern crate serde_json;

use std::collections::{HashMap, HashSet};
use std::borrow::Cow;
use std::convert::Infallible;
use std::fmt;
use std::future::Future;
use std::net::SocketAddr;
use std::path::{PathBuf, Path};
use std::sync::{Arc, RwLock};
use std::error::{Error as StdError};
use std::time::Instant;

use futures::{TryFutureExt, FutureExt};

use hyper::Server;
use hyper::service::{make_service_fn, service_fn};

use task_file::{find_task_files, ConfigFileError};

use crate::task::TaskDef;
use crate::task_file::TaskFileToml;
use crate::server_file::ServerToml;

mod interleave;
mod json;
mod json_conv;
pub mod password;
mod server;
mod server_file;
mod task;
mod task_file;
mod utils;
mod web;

#[derive(Debug)]
enum AppError {
    InvalidListenAddr
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::InvalidListenAddr => write!(f, "Invalid listen address")
        }
    }
}

impl std::error::Error for AppError {}

// current directory by default
const DEFAULT_TASK_DIR: &'static str = ".";
const DEFAULT_LISTEN_ADDR: &'static str = "0.0.0.0:8080";

pub struct ServerConfig {
    // pub listen_addr: SocketAddr,
    // pub task_dir: PathBuf,
    pub config: PathBuf,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CachedCredential {
    pub username: String,
    pub password_hash: Vec<u8>,
}

pub struct UserSession {
    pub expires_at: Instant,
}

pub struct UserDef {
    pub username: String,
    pub password: password::PasswordParts,
    pub roles: HashSet<String>,
}

pub struct Shared {
    // pub config: ServerConfig,
    pub tasks: RwLock<HashMap<String, TaskDef>>,
    pub users: RwLock<HashMap<String, UserDef>>,
    pub sessions: RwLock<HashMap<CachedCredential, UserSession>>,
}

pub struct TaskRequest {
    pub name: String,
    pub method: http::Method,
    pub params: HashMap<String, String>,
}

#[derive(Debug)]
pub struct TaskExec {
    pub command: String,
    pub args: Vec<String>,
    pub dir: PathBuf,
    pub env: HashMap<String, String>,
}

impl fmt::Display for ConfigFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigFileError::Io(wrapped, path) =>
                write!(f, "Error reading file: {} ({})",
                       path.as_ref().map(|x| x.to_string_lossy()).unwrap_or(Cow::from("<unknown>")), wrapped),
            ConfigFileError::Toml(wrapped, path) =>
                write!(f, "Error parsing file: {} ({})", path.to_string_lossy(), wrapped),
            ConfigFileError::InvalidTaskFileName(path) =>
                write!(f, "Invalid task file name: {}", path.to_string_lossy()),
            ConfigFileError::InvalidPasswordHash { username } =>
                write!(f, "Invalid password hash for username: {}", username),
        }
    }
}

impl StdError for ConfigFileError {}

type GenericError = Box<dyn StdError + Send>;

fn box_error<E: StdError + Send + 'static>(e: E) -> Box<dyn StdError + Send> {
    Box::new(e) as GenericError
}

fn load_tasks(task_dir: &Path) -> Result<HashMap<String, TaskDef>, GenericError> {
    let task_files = find_task_files(task_dir).map_err(box_error)?;

    info!("Found task files: {:?}", task_files);

    let tasks = task_files.iter()
        .map(|path| TaskFileToml::load(&path))
        .collect::<Result<Vec<TaskDef>, _>>()
        .map_err(box_error)?;

    let tasks_by_name = tasks.into_iter()
        .map(|t| (t.name.clone(), t))
        .collect::<HashMap<String, TaskDef>>();

    Ok(tasks_by_name)
}

async fn shutdown_signal() -> () {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
}

pub async fn run_server<F, SF>(config: ServerConfig, on_bind: F, shutdown_fut: SF) -> Result<(), GenericError>
    where F: FnOnce(SocketAddr) -> (),
          SF: Future<Output=()> + Unpin {
    let server_toml = server_file::load_toml::<ServerToml>(&config.config).map_err(box_error)?;

    let listen_addr = match server_toml.server.as_ref().and_then(|server| server.listen.as_ref()) {
        Some(s) => s.parse::<SocketAddr>().map_err(|e| {
            error!("Error parsing listen address: {:?}", e);
            AppError::InvalidListenAddr
        }),
        None => Ok(DEFAULT_LISTEN_ADDR.parse().unwrap()),
    }.map_err(box_error)?;

    let task_dir: PathBuf = server_toml.server
        .and_then(|server| server.dir)
        .map(|x| PathBuf::from(x))
        .unwrap_or(PathBuf::from(DEFAULT_TASK_DIR));

    // resolve tasks directory relative to server config if not an absolute path
    let task_dir_resolved = if task_dir.is_absolute() {
        task_dir
    } else {
        match config.config.parent() {
            Some(config_dir) => config_dir.join(task_dir),
            None => task_dir
        }
    };

    let tasks_by_name = load_tasks(&task_dir_resolved)?;

    let users: Vec<UserDef> = match server_toml.auth {
        None => vec![],
        Some(auth) => {
            let mut result = Vec::<UserDef>::new();
            // using a loop rather than 'map()' for simpler control flow on error
            for user in auth.users {
                result.push(UserDef {
                    username: user.username.clone(),
                    password: password::parse_password(&user.password).map_err(|err| {
                        error!("Error parsing password for user: {} ({:?})", user.username.clone(), err);
                        ConfigFileError::InvalidPasswordHash {
                            username: user.username.clone()
                        }
                    }).map_err(box_error)?,
                    roles: user.roles.clone().into_iter().collect(),
                });
            }
            result
        }
    };

    let users_by_name = users.into_iter()
        .map(|t| (t.username.clone(), t))
        .collect::<HashMap<String, UserDef>>();

    let shared = Shared {
        // config,
        tasks: RwLock::new(tasks_by_name),
        users: RwLock::new(users_by_name),
        sessions: RwLock::new(HashMap::new()),
    };

    let shared = Arc::new(shared);

    let make_svc = make_service_fn(move |_conn| {
        let shared = shared.clone();
        async {
            Ok::<_, Infallible>(service_fn({
                move |req| {
                    crate::server::handle(shared.clone(), req)
                }
            }))
        }
    });

    // TODO: handle bind error with better message
    let server = Server::try_bind(&listen_addr)
        .map_err(box_error)?
        .serve(make_svc);

    // find out what port we actually bound to
    on_bind(server.local_addr().clone());

    let shutdown_signal = Box::pin(shutdown_signal());

    let shutdown_combined = futures::future::select(shutdown_signal, shutdown_fut).map(|_| ());

    let graceful_server = server.with_graceful_shutdown(shutdown_combined);

    Ok(graceful_server.map_err(box_error).await?)
}
