//! ListenSupervisor —— 进程内「小网关」Listen 层（用户裁决方案一：**改地址不重启**）。
//!
//! 分层（任务级最小重启粒度）：
//! - **内核**（Router / auth RwLock / handlers / ServeDir，`Arc` 共享）**永不因换地址而亡**；
//! - **Listen 层** = 本模块：独立 supervisor task，唯一职责 = 持有当前 listener +
//!   serve task，接受热替换命令（`Rebind` / `Shutdown`）。
//!
//! ## Rebind 时序（try-bind → 优雅退场 → spawn）
//!
//! 1. **先 try-bind 新地址**（绝大多数情形 —— 端口/IP 不重叠时）：
//!    失败 → 返回错误给调用方，**旧监听原样保留**（带病不上线）；
//!    成功 → spawn 新 `axum::serve(app, …).with_graceful_shutdown(cancel)` 任务
//!    → 取消旧 serve（**停 accept、在途请求跑完**；`DRAIN_TIMEOUT` 超时兜底
//!    `abort()` 强杀）→ 更新共享 `Arc<RwLock<SocketAddr>>`。
//! 2. **同端口重叠例外**（local⇄cloud 成对默认同为 `:11435`，仅 IP 不同 ——
//!    Windows/POSIX 上新 bind 会被旧监听挡住，无法先 try-bind）：
//!    优雅退场旧 serve → try-bind → 成功 spawn；失败 → **恢复绑定旧地址**
//!    并返回错误（旧地址刚释放，恢复必成；仍失败则报 `restore failed` 臁目）。
//!
//! 粒度声明：**任务级**（tokio task + socket，零进程重启、零内核重建）。
//! 进程级独立小网关（故障隔离场景）不在本期实现 —— 见 deployment.md
//! 「重启粒度」备注的可选演进。
//!
//! 显式 bind（配置/`JEV_BIND`）时 mode 翻转**不**走 Rebind（`explicit` 标志），
//! 由 auth 中间件 peer 校验兜底（见 `auth::require_*`）。

use crate::config::RunMode;
use axum::Router;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::task::JoinHandle;

/// 旧 serve 优雅退场兜底时长：超时（在途请求卡死）→ `abort()` 强杀。
pub const DRAIN_TIMEOUT: Duration = Duration::from_secs(10);

/* ══════════════════════════════════════════════════════════════════
   错误
   ══════════════════════════════════════════════════════════════════ */

#[derive(Debug, Error)]
pub enum ListenError {
    /// supervisor 已停止（shutdown 后 / 通道关闭）。
    #[error("listen supervisor stopped")]
    Stopped,
    /// try-bind 失败（旧监听未动，或同端口交接后已恢复旧地址）。
    #[error("bind {addr} failed: {source}")]
    Bind {
        addr: String,
        #[source]
        source: std::io::Error,
    },
    /// 同端口交接失败且恢复旧地址也失败（ 臁目：服务可能中断）。
    #[error("bind {addr} failed ({source}); CRITICAL: restore {prev} also failed: {restore_source}")]
    RestoreFailed {
        addr: String,
        prev: String,
        #[source]
        source: std::io::Error,
        restore_source: std::io::Error,
    },
}

/* ══════════════════════════════════════════════════════════════════
   成对默认（mode ⇄ 地址；测试注入自定义对避免撞 11435）
   ══════════════════════════════════════════════════════════════════ */

/// 成对默认地址对（未显式 bind 时：mode 翻转的 Rebind 目标 / `addr=auto` 目标）。
#[derive(Debug, Clone, Copy)]
pub struct PairDefaults {
    pub local: std::net::SocketAddr,
    pub cloud: std::net::SocketAddr,
}

impl PairDefaults {
    /// 生产标准对：local = `127.0.0.1:11435`、cloud = `0.0.0.0:11435`。
    pub fn standard() -> Self {
        PairDefaults {
            local: crate::config::paired_default(RunMode::Local),
            cloud: crate::config::paired_default(RunMode::Cloud),
        }
    }

    pub fn for_mode(self, mode: RunMode) -> std::net::SocketAddr {
        match mode {
            RunMode::Local => self.local,
            RunMode::Cloud => self.cloud,
        }
    }
}

/// 启动计划（`start` 入参）。
#[derive(Debug, Clone, Copy)]
pub struct ListenPlan {
    /// 首次监听地址（`Config::effective_bind` 的结果）。
    pub addr: std::net::SocketAddr,
    /// 是否显式 bind（文件/`JEV_BIND`）—— 显式时 mode 翻转不 Rebind。
    pub explicit: bool,
    /// 成对默认对。
    pub defaults: PairDefaults,
}

/* ══════════════════════════════════════════════════════════════════
   命令通道
   ══════════════════════════════════════════════════════════════════ */

enum Cmd {
    Rebind {
        addr: std::net::SocketAddr,
        reply: oneshot::Sender<Result<(std::net::SocketAddr, std::net::SocketAddr), ListenError>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

/* ══════════════════════════════════════════════════════════════════
   ListenHandle —— 供 main / admin handler 克隆持有（mpsc::Sender 可多克隆）
   ══════════════════════════════════════════════════════════════════ */

/// Listen 层句柄（挂 `AppState.listen`；`Clone` = 指针克隆）。
#[derive(Clone)]
pub struct ListenHandle {
    cmd_tx: mpsc::Sender<Cmd>,
    bound: Arc<RwLock<std::net::SocketAddr>>,
    explicit: Arc<AtomicBool>,
    defaults: PairDefaults,
}

impl ListenHandle {
    /// 当前实际监听地址（Rebind 成功后更新）。
    pub fn bound(&self) -> std::net::SocketAddr {
        *self.bound.read().expect("bound addr lock")
    }

    /// 是否显式 bind（mode 翻转是否应 Rebind 的判据）。
    pub fn is_explicit(&self) -> bool {
        self.explicit.load(Ordering::SeqCst)
    }

    /// 更新显式标志（`PUT /listen` 写回后调用）。
    pub fn set_explicit(&self, v: bool) {
        self.explicit.store(v, Ordering::SeqCst);
    }

    /// 成对默认对。
    pub fn defaults(&self) -> PairDefaults {
        self.defaults
    }

    /// 请求热切换监听地址。返回 `(from, to)`。
    ///
    /// 地址与当前相同 → no-op 成功（同值幂等）。失败时旧监听**原样保留**
    /// （同端口交接例外会先优雅退场再恢复 —— 见模块文档时序）。
    pub async fn rebind(
        &self,
        addr: std::net::SocketAddr,
    ) -> Result<(std::net::SocketAddr, std::net::SocketAddr), ListenError> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Cmd::Rebind { addr, reply: tx })
            .await
            .map_err(|_| ListenError::Stopped)?;
        rx.await.map_err(|_| ListenError::Stopped)?
    }

    /// 请求优雅停机（停 accept、在途跑完、超时强杀）。
    pub async fn shutdown(&self) -> Result<(), ListenError> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(Cmd::Shutdown { reply: tx })
            .await
            .map_err(|_| ListenError::Stopped)?;
        rx.await.map_err(|_| ListenError::Stopped)
    }
}

/* ══════════════════════════════════════════════════════════════════
   启动 / supervisor 循环
   ══════════════════════════════════════════════════════════════════ */

/// 当前 serve 资源（supervisor task 独占持有）。
struct Serving {
    addr: std::net::SocketAddr,
    shutdown: Arc<Notify>,
    task: JoinHandle<()>,
}

impl Serving {
    /// 优雅退场：取消 accept 循环（在途请求继续）→ 超时 abort 兜底。
    async fn stop(&mut self) {
        self.shutdown.notify_one();
        match tokio::time::timeout(DRAIN_TIMEOUT, &mut self.task).await {
            Ok(_) => {} // graceful 完成（含在途请求收尾）
            Err(_) => {
                tracing::warn!(addr = %self.addr, "serve drain timeout — force abort");
                self.task.abort();
                let _ = (&mut self.task).await; // 收割，防僵尸
            }
        }
    }
}

/// spawn 一个 serve 任务：`into_make_service_with_connect_info`（peer 校验需要
/// `ConnectInfo<SocketAddr>`）+ `with_graceful_shutdown(Notify)`。
fn spawn_serve(
    listener: TcpListener,
    app: Router,
) -> (Arc<Notify>, JoinHandle<()>) {
    let shutdown = Arc::new(Notify::new());
    let signal = shutdown.clone();
    let task = tokio::spawn(async move {
        let svc = app.into_make_service_with_connect_info::<std::net::SocketAddr>();
        let result = axum::serve(listener, svc)
            .with_graceful_shutdown(async move { signal.notified().await })
            .await;
        if let Err(e) = result {
            tracing::error!(error = %e, "serve task exited with error");
        }
    });
    (shutdown, task)
}

/// 新旧地址是否在「绑不进彼此」的重叠域（同端口 + IP 相等或任一侧 unspecified）。
fn binds_overlap(a: std::net::SocketAddr, b: std::net::SocketAddr) -> bool {
    a.port() == b.port()
        && (a.ip() == b.ip() || a.ip().is_unspecified() || b.ip().is_unspecified())
}

/// supervisor 主循环：持有当前 serve 资源，串行处理命令（一次只做一件事 ——
/// 并发 Rebind 排队，语义简单可推理）。
async fn run_supervisor(
    mut rx: mpsc::Receiver<Cmd>,
    app: Router,
    mut current: Serving,
    bound: Arc<RwLock<std::net::SocketAddr>>,
) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            Cmd::Rebind { addr, reply } => {
                let from = current.addr;
                let result = if addr == from {
                    Ok((from, addr)) // 同值幂等（含 PUT listen 钉住当前地址的场景）
                } else {
                    rebind(&mut current, app.clone(), addr, &bound).await
                };
                let _ = reply.send(result);
            }
            Cmd::Shutdown { reply } => {
                current.stop().await;
                let _ = reply.send(());
                return;
            }
        }
    }
    // 所有句柄 drop（无人再发命令）→ 顺手停 serve，防孤儿任务
    current.stop().await;
}

async fn rebind(
    current: &mut Serving,
    app: Router,
    addr: std::net::SocketAddr,
    bound: &Arc<RwLock<std::net::SocketAddr>>,
) -> Result<(std::net::SocketAddr, std::net::SocketAddr), ListenError> {
    let from = current.addr;
    // 阶段 1：先 try-bind（不重叠地址 = 零停机换端口）
    match TcpListener::bind(addr).await {
        Ok(listener) => {
            let (shutdown, task) = spawn_serve(listener, app);
            current.stop().await; // 旧 serve 优雅退场（在途跑完）
            *current = Serving {
                addr,
                shutdown,
                task,
            };
            *bound.write().expect("bound addr lock") = addr;
            Ok((from, addr))
        }
        Err(e) if binds_overlap(addr, from) => {
            // 阶段 2：同端口成对切换（127.0.0.1 ⇄ 0.0.0.0:同端口）——
            // 旧监听物理占位，必须先优雅退场再 bind。
            current.stop().await;
            match TcpListener::bind(addr).await {
                Ok(listener) => {
                    let (shutdown, task) = spawn_serve(listener, app);
                    *current = Serving {
                        addr,
                        shutdown,
                        task,
                    };
                    *bound.write().expect("bound addr lock") = addr;
                    Ok((from, addr))
                }
                Err(e2) => {
                    // 恢复旧地址（刚释放，应当必成）
                    tracing::error!(failed = %addr, prev = %from, error = %e2,
                        "rebind failed — restoring previous listener");
                    match TcpListener::bind(from).await {
                        Ok(listener) => {
                            let (shutdown, task) = spawn_serve(listener, app);
                            *current = Serving {
                                addr: from,
                                shutdown,
                                task,
                            };
                            Err(ListenError::Bind {
                                addr: addr.to_string(),
                                source: e2,
                            })
                        }
                        Err(e3) => Err(ListenError::RestoreFailed {
                            addr: addr.to_string(),
                            prev: from.to_string(),
                            source: e2,
                            restore_source: e3,
                        }),
                    }
                }
            }
        }
        Err(e) => {
            // 端口不重叠仍 bind 失败（被第三方占用 / 权限）→ 旧监听一字未动
            Err(ListenError::Bind {
                addr: addr.to_string(),
                source: e,
            })
        }
    }
}

/// 启动 Listen 层：bind 首地址 → 注册句柄进 `slot`（**先于** accept 循环，
/// 保证请求到达时句柄已可见）→ spawn serve + supervisor。
pub async fn start(
    app: Router,
    plan: ListenPlan,
    slot: &Arc<std::sync::OnceLock<ListenHandle>>,
) -> Result<ListenHandle, ListenError> {
    let listener = TcpListener::bind(plan.addr).await.map_err(|source| ListenError::Bind {
        addr: plan.addr.to_string(),
        source,
    })?;
    let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>(8);
    let bound = Arc::new(RwLock::new(plan.addr));
    let handle = ListenHandle {
        cmd_tx,
        bound: bound.clone(),
        explicit: Arc::new(AtomicBool::new(plan.explicit)),
        defaults: plan.defaults,
    };
    // 先入 slot 再开 serve：杜绝「已监听但句柄未就绪」窗口
    let _ = slot.set(handle.clone());
    let (shutdown, task) = spawn_serve(listener, app.clone());
    let current = Serving {
        addr: plan.addr,
        shutdown,
        task,
    };
    tokio::spawn(run_supervisor(cmd_rx, app, current, bound));
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_rules_cover_pair_defaults() {
        let l: std::net::SocketAddr = "127.0.0.1:11435".parse().unwrap();
        let c: std::net::SocketAddr = "0.0.0.0:11435".parse().unwrap();
        let other: std::net::SocketAddr = "127.0.0.1:11436".parse().unwrap();
        assert!(binds_overlap(l, c), "成对默认同端口重叠（须走阶段 2）");
        assert!(binds_overlap(c, l));
        assert!(!binds_overlap(l, other), "不同端口不重叠（阶段 1 零停机）");
        // 同地址本身也属重叠域 —— 但 rebind 入口 `addr == from` 已短路为 no-op，
        // 不会走到 overlap 分支（此处只锁函数自身语义）
        assert!(binds_overlap(l, l));
    }

    #[test]
    fn pair_defaults_standard_and_for_mode() {
        let p = PairDefaults::standard();
        assert_eq!(p.for_mode(RunMode::Local).to_string(), "127.0.0.1:11435");
        assert_eq!(p.for_mode(RunMode::Cloud).to_string(), "0.0.0.0:11435");
    }
}
