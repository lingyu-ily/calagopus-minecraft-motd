mod config;
mod firewall;
mod model;
mod original_dst;
mod presentation;
mod protocol;

use anyhow::Context;
use clap::{Parser, Subcommand};
use config::AgentConfig;
use firewall::FirewallManager;
use model::{
    EnrollmentRequest, EnrollmentResponse, HeartbeatRequest, ServerState, Snapshot, SnapshotServer,
    WakeRequest, WakeResponse,
};
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    collections::BTreeSet,
    env,
    io::ErrorKind,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Mutex, RwLock},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "calagopus-minecraft-motd-agent", version = VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run {
        #[arg(long, default_value = "/etc/calagopus-motd-agent/config.toml")]
        config: PathBuf,
    },
    Enroll {
        #[arg(long)]
        panel_url: String,
        #[arg(long)]
        enrollment_token: String,
        #[arg(long, default_value = "/etc/calagopus-motd-agent/config.toml")]
        output: PathBuf,
        #[arg(long, default_value_t = 4001)]
        listen_port: u16,
    },
}

#[derive(Clone)]
struct CachedSnapshot {
    value: Snapshot,
    received: Instant,
}

type SharedSnapshot = Arc<RwLock<Option<CachedSnapshot>>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "calagopus_minecraft_motd_agent=info".into()),
        )
        .init();

    match Cli::parse().command {
        Command::Run { config } => {
            match tokio::fs::metadata(&config).await {
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    let panel_url = env::var("MOTD_PANEL_URL")
                        .context("MOTD_PANEL_URL is required for first-time enrollment")?;
                    let enrollment_token = env::var("MOTD_ENROLLMENT_TOKEN")
                        .context("MOTD_ENROLLMENT_TOKEN is required for first-time enrollment")?;
                    if panel_url.trim().trim_end_matches('/').is_empty() {
                        anyhow::bail!("MOTD_PANEL_URL must not be empty");
                    }
                    if enrollment_token.is_empty() {
                        anyhow::bail!("MOTD_ENROLLMENT_TOKEN must not be empty");
                    }
                    let listen_port = match env::var("MOTD_LISTEN_PORT") {
                        Ok(value) => value
                            .parse::<u16>()
                            .context("MOTD_LISTEN_PORT must be a valid TCP port")?,
                        Err(env::VarError::NotPresent) => 4001,
                        Err(error) => return Err(error).context("MOTD_LISTEN_PORT is invalid"),
                    };
                    if listen_port == 0 {
                        anyhow::bail!("MOTD_LISTEN_PORT must not be zero");
                    }
                    enroll(&panel_url, &enrollment_token, &config, listen_port).await?;
                }
                Err(error) => return Err(error).context("failed to inspect agent configuration"),
            }

            let mut agent_config = AgentConfig::load(&config).await?;
            if let Ok(panel_url) = env::var("MOTD_PANEL_URL") {
                let panel_url = panel_url.trim().trim_end_matches('/');
                if panel_url.is_empty() {
                    anyhow::bail!("MOTD_PANEL_URL must not be empty");
                }
                agent_config.panel_url = panel_url.to_owned();
            }
            run(agent_config).await
        }
        Command::Enroll {
            panel_url,
            enrollment_token,
            output,
            listen_port,
        } => enroll(&panel_url, &enrollment_token, &output, listen_port).await,
    }
}

fn endpoint(panel_url: &str, path: &str) -> String {
    format!(
        "{}/api/minecraft-motd/agent/v1/{path}",
        panel_url.trim_end_matches('/')
    )
}

async fn enroll(
    panel_url: &str,
    enrollment_token: &str,
    output: &std::path::Path,
    listen_port: u16,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let response = client
        .post(endpoint(panel_url, "enroll"))
        .json(&EnrollmentRequest {
            enrollment_token,
            version: VERSION,
        })
        .send()
        .await?
        .error_for_status()?
        .json::<EnrollmentResponse>()
        .await?;

    AgentConfig {
        panel_url: panel_url.trim_end_matches('/').to_owned(),
        agent_token: response.agent_token,
        node_uuid: response.node_uuid,
        listen_port,
        poll_interval_seconds: 2,
        stale_after_seconds: 15,
        firewall_backend: Default::default(),
    }
    .save(output)
    .await?;
    println!(
        "enrolled node {} and wrote {}",
        response.node_uuid,
        output.display()
    );
    Ok(())
}

#[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
async fn run(config: AgentConfig) -> anyhow::Result<()> {
    #[cfg(not(target_os = "linux"))]
    anyhow::bail!("the node agent can only manage production traffic on Linux");

    #[cfg(target_os = "linux")]
    {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        let snapshot: SharedSnapshot = Arc::new(RwLock::new(None));
        let firewall = Arc::new(Mutex::new(
            FirewallManager::detect(config.firewall_backend, config.listen_port).await?,
        ));
        let listener = create_listener(config.listen_port)?;

        let poll_task = tokio::spawn(poll_loop(
            config.clone(),
            client.clone(),
            Arc::clone(&snapshot),
            Arc::clone(&firewall),
        ));

        tracing::info!(port = config.listen_port, "Minecraft MOTD agent listening");
        let shutdown = shutdown_signal();
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                result = listener.accept() => {
                    let (stream, peer) = result?;
                    let config = config.clone();
                    let client = client.clone();
                    let snapshot = Arc::clone(&snapshot);
                    tokio::spawn(async move {
                        if let Err(error) = handle_connection(stream, &config, &client, &snapshot).await {
                            tracing::debug!(%peer, %error, "rejected Minecraft connection");
                        }
                    });
                }
                result = &mut shutdown => {
                    result?;
                    break;
                }
            }
        }

        poll_task.abort();
        firewall.lock().await.cleanup().await?;
        tracing::info!("Minecraft MOTD agent stopped and firewall rules were removed");
        Ok(())
    }
}

async fn shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

fn create_listener(port: u16) -> anyhow::Result<TcpListener> {
    let socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_reuse_address(true)?;
    socket.set_only_v6(false)?;
    socket.bind(&SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port).into())?;
    socket.listen(1024)?;
    socket.set_nonblocking(true)?;
    Ok(TcpListener::from_std(socket.into())?)
}

async fn fetch_snapshot(
    config: &AgentConfig,
    client: &reqwest::Client,
) -> anyhow::Result<Snapshot> {
    let snapshot = client
        .get(endpoint(&config.panel_url, "snapshot"))
        .bearer_auth(&config.agent_token)
        .send()
        .await?
        .error_for_status()?
        .json::<Snapshot>()
        .await?;
    if snapshot.node_uuid != config.node_uuid {
        anyhow::bail!("panel returned a snapshot for the wrong node");
    }
    Ok(snapshot)
}

async fn send_heartbeat(config: &AgentConfig, client: &reqwest::Client) -> anyhow::Result<()> {
    client
        .post(endpoint(&config.panel_url, "heartbeat"))
        .bearer_auth(&config.agent_token)
        .json(&HeartbeatRequest { version: VERSION })
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn poll_loop(
    config: AgentConfig,
    client: reqwest::Client,
    shared: SharedSnapshot,
    firewall: Arc<Mutex<FirewallManager>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(config.poll_interval_seconds));
    let mut last_heartbeat = Instant::now()
        .checked_sub(Duration::from_secs(30))
        .unwrap_or_else(Instant::now);
    loop {
        interval.tick().await;
        match fetch_snapshot(&config, &client).await {
            Ok(mut snapshot) => {
                prepare_snapshot(&mut snapshot).await;
                let allocations = desired_allocations(&snapshot, config.listen_port);
                if let Err(error) = firewall.lock().await.reconcile(allocations).await {
                    tracing::error!(%error, "failed to reconcile firewall rules");
                    continue;
                }
                tracing::debug!(
                    revision = snapshot.revision,
                    generated_at = %snapshot.generated_at,
                    node_reachable = snapshot.node_reachable,
                    servers = snapshot.servers.len(),
                    "snapshot updated"
                );
                *shared.write().await = Some(CachedSnapshot {
                    value: snapshot,
                    received: Instant::now(),
                });
                if last_heartbeat.elapsed() >= Duration::from_secs(30) {
                    match send_heartbeat(&config, &client).await {
                        Ok(()) => last_heartbeat = Instant::now(),
                        Err(error) => tracing::warn!(%error, "failed to send agent heartbeat"),
                    }
                }
            }
            Err(error) => {
                tracing::warn!(%error, "failed to refresh panel snapshot");
                let stale = shared.read().await.clone();
                if let Some(mut cached) = stale
                    && cached.received.elapsed() >= Duration::from_secs(config.stale_after_seconds)
                {
                    for server in &mut cached.value.servers {
                        if matches!(server.state, ServerState::Running) && !port_ready(server).await
                        {
                            server.state = ServerState::NodeUnreachable;
                            server.autostart_on_join = false;
                        }
                    }
                    let allocations = desired_allocations(&cached.value, config.listen_port);
                    if let Err(error) = firewall.lock().await.reconcile(allocations).await {
                        tracing::error!(%error, "failed to reconcile stale-state firewall rules");
                    }
                    *shared.write().await = Some(cached);
                }
            }
        }
    }
}

async fn prepare_snapshot(snapshot: &mut Snapshot) {
    let mut checks = tokio::task::JoinSet::new();
    for (index, server) in snapshot.servers.iter().enumerate() {
        if matches!(server.state, ServerState::Running) {
            let server = server.clone();
            checks.spawn(async move { (index, port_ready(&server).await) });
        }
    }
    while let Some(Ok((index, ready))) = checks.join_next().await {
        if !ready {
            snapshot.servers[index].state = ServerState::Starting;
        }
    }
}

fn connect_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        value => value,
    }
}

async fn port_ready(server: &SnapshotServer) -> bool {
    let target = SocketAddr::new(connect_ip(server.allocation_ip), server.allocation_port);
    tokio::time::timeout(Duration::from_millis(800), TcpStream::connect(target))
        .await
        .is_ok_and(|result| result.is_ok())
}

fn desired_allocations(snapshot: &Snapshot, listen_port: u16) -> BTreeSet<SocketAddr> {
    if !snapshot.settings.enabled {
        return BTreeSet::new();
    }
    snapshot
        .servers
        .iter()
        .filter(|server| server.allocation_port != listen_port)
        .filter(|server| !matches!(server.state, ServerState::Running))
        .map(|server| SocketAddr::new(server.allocation_ip, server.allocation_port))
        .collect()
}

fn allocation_matches(original: SocketAddr, server: &SnapshotServer) -> bool {
    original.port() == server.allocation_port
        && (original.ip() == server.allocation_ip || server.allocation_ip.is_unspecified())
}

fn stale_state(server: &SnapshotServer, stale: bool) -> ServerState {
    if !stale {
        return server.state;
    }
    match server.state {
        ServerState::Suspended
        | ServerState::NodeMaintenance
        | ServerState::Transferring
        | ServerState::Installing
        | ServerState::InstallFailed
        | ServerState::RestoringBackup
        | ServerState::BackupRestoreFailed => server.state,
        _ => ServerState::NodeUnreachable,
    }
}

async fn wake_server(
    config: &AgentConfig,
    client: &reqwest::Client,
    server: &SnapshotServer,
) -> anyhow::Result<WakeResponse> {
    Ok(client
        .post(endpoint(&config.panel_url, "wake"))
        .bearer_auth(&config.agent_token)
        .json(&WakeRequest {
            server_uuid: server.server_uuid,
            allocation_port: server.allocation_port,
        })
        .send()
        .await?
        .error_for_status()?
        .json::<WakeResponse>()
        .await?)
}

async fn handle_connection(
    mut stream: TcpStream,
    config: &AgentConfig,
    client: &reqwest::Client,
    shared: &SharedSnapshot,
) -> anyhow::Result<()> {
    stream.set_nodelay(true)?;
    let original = original_dst::original_destination(&stream)
        .context("could not determine original destination")?;
    let packet = tokio::time::timeout(Duration::from_secs(5), protocol::read_packet(&mut stream))
        .await
        .context("handshake timed out")??;
    let handshake = protocol::parse_handshake(&packet)?;
    if handshake.server_address.is_empty() {
        anyhow::bail!("handshake server address is empty");
    }
    if handshake.server_port != original.port() {
        anyhow::bail!("handshake port does not match the original destination");
    }

    let cached = shared
        .read()
        .await
        .clone()
        .context("no panel snapshot is available")?;
    let stale = cached.received.elapsed() >= Duration::from_secs(config.stale_after_seconds);
    let server = cached
        .value
        .servers
        .iter()
        .find(|server| allocation_matches(original, server))
        .context("allocation is not managed by this agent")?;
    let mut state = stale_state(server, stale);
    if matches!(state, ServerState::Running) {
        state = ServerState::Starting;
    }

    if handshake.next_state == 1 {
        let status = presentation::status_json(
            &cached.value.settings,
            server,
            state,
            handshake.protocol_version,
        )?;
        protocol::serve_status(&mut stream, status).await?;
        return Ok(());
    }

    if matches!(state, ServerState::Offline) && server.autostart_on_join && !stale {
        match wake_server(config, client, server).await {
            Ok(response)
                if response.started
                    || matches!(
                        response.reason.as_str(),
                        "not_offline" | "disabled_or_throttled"
                    ) =>
            {
                state = ServerState::Starting;
            }
            Ok(response) => {
                tracing::warn!(reason = %response.reason, server = %server.server_uuid, "wake request was rejected")
            }
            Err(error) => {
                tracing::warn!(%error, server = %server.server_uuid, "wake request failed")
            }
        }
    }
    let message = presentation::kick_message(&cached.value.settings, server, state)?;
    protocol::disconnect_login(&mut stream, &message).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_state_keeps_administrative_reason() {
        let server = SnapshotServer {
            server_uuid: uuid::Uuid::nil(),
            server_name: "test".into(),
            node_name: "node".into(),
            allocation_ip: "127.0.0.1".parse().unwrap(),
            allocation_port: 25565,
            state: ServerState::Suspended,
            autostart_on_join: true,
        };
        assert_eq!(stale_state(&server, true), ServerState::Suspended);
    }

    #[test]
    fn desired_allocations_keep_ip_and_port_distinct() {
        let server = SnapshotServer {
            server_uuid: uuid::Uuid::nil(),
            server_name: "test".into(),
            node_name: "node".into(),
            allocation_ip: "192.0.2.10".parse().unwrap(),
            allocation_port: 25565,
            state: ServerState::Offline,
            autostart_on_join: false,
        };
        let mut other = server.clone();
        other.server_uuid = uuid::Uuid::from_u128(1);
        other.allocation_ip = "192.0.2.11".parse().unwrap();
        let snapshot = Snapshot {
            revision: 1,
            generated_at: chrono::Utc::now(),
            node_uuid: uuid::Uuid::nil(),
            node_reachable: true,
            settings: test_settings(),
            servers: vec![server.clone(), other],
        };
        let allocations = desired_allocations(&snapshot, 4001);
        assert_eq!(allocations.len(), 2);
        assert!(allocations.contains(&"192.0.2.10:25565".parse().unwrap()));
        assert!(allocations.contains(&"192.0.2.11:25565".parse().unwrap()));
    }

    fn test_settings() -> model::GlobalSettings {
        model::GlobalSettings {
            enabled: true,
            _all_allocations: false,
            rotation_interval_seconds: 10,
            swap_lines: false,
            gradient_enabled: false,
            gradient_colors: vec![],
            favicon_base64: None,
            _excluded_egg_uuids: vec![],
            _excluded_server_uuids: vec![],
            states: Default::default(),
        }
    }
}
