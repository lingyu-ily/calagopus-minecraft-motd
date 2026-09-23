use crate::config::FirewallPreference;
use std::{
    collections::BTreeSet,
    net::{IpAddr, SocketAddr},
};

const NFT_TABLE: &str = "calagopus_motd";
const IPT_CHAIN: &str = "CALAGOPUS_MOTD";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Nftables,
    Iptables,
}

pub struct FirewallManager {
    backend: Backend,
    listen_port: u16,
    current_allocations: BTreeSet<SocketAddr>,
}

impl FirewallManager {
    pub async fn detect(preference: FirewallPreference, listen_port: u16) -> anyhow::Result<Self> {
        let nft_available = command_exists("nft").await;
        let iptables_available =
            command_exists("iptables").await && command_exists("iptables-restore").await;
        let backend = match preference {
            FirewallPreference::Nftables if nft_available => Backend::Nftables,
            FirewallPreference::Iptables if iptables_available => Backend::Iptables,
            FirewallPreference::Auto if nft_available => Backend::Nftables,
            FirewallPreference::Auto if iptables_available => Backend::Iptables,
            FirewallPreference::Nftables => {
                anyhow::bail!("nftables was requested but nft is unavailable")
            }
            FirewallPreference::Iptables => {
                anyhow::bail!("iptables was requested but iptables is unavailable")
            }
            FirewallPreference::Auto => anyhow::bail!("neither nftables nor iptables is available"),
        };
        Ok(Self {
            backend,
            listen_port,
            current_allocations: BTreeSet::new(),
        })
    }

    pub async fn reconcile(&mut self, allocations: BTreeSet<SocketAddr>) -> anyhow::Result<()> {
        if allocations == self.current_allocations {
            return Ok(());
        }
        match self.backend {
            Backend::Nftables => apply_nftables(self.listen_port, &allocations).await?,
            Backend::Iptables => apply_iptables(self.listen_port, &allocations).await?,
        }
        tracing::info!(count = allocations.len(), backend = ?self.backend, "reconciled Minecraft MOTD firewall rules");
        self.current_allocations = allocations;
        Ok(())
    }

    pub async fn cleanup(&mut self) -> anyhow::Result<()> {
        match self.backend {
            Backend::Nftables => {
                let _ = run("nft", &["delete", "table", "inet", NFT_TABLE]).await;
            }
            Backend::Iptables => {
                cleanup_iptables_family("iptables").await;
                if command_exists("ip6tables").await {
                    cleanup_iptables_family("ip6tables").await;
                }
            }
        }
        self.current_allocations.clear();
        Ok(())
    }
}

fn render_nftables(listen_port: u16, allocations: &BTreeSet<SocketAddr>, replace: bool) -> String {
    let mut script = String::new();
    if replace {
        script.push_str(&format!("delete table inet {NFT_TABLE}\n"));
    }
    script.push_str(&format!(
        "table inet {NFT_TABLE} {{\n  chain prerouting {{\n    type nat hook prerouting priority dstnat; policy accept;\n"
    ));
    for allocation in allocations {
        let destination = match allocation.ip() {
            IpAddr::V4(ip) if ip.is_unspecified() => "meta nfproto ipv4".to_owned(),
            IpAddr::V4(ip) => format!("ip daddr {ip}"),
            IpAddr::V6(ip) if ip.is_unspecified() => "meta nfproto ipv6".to_owned(),
            IpAddr::V6(ip) => format!("ip6 daddr {ip}"),
        };
        script.push_str(&format!(
            "    {destination} tcp dport {} redirect to :{listen_port} comment \"Calagopus Minecraft MOTD\"\n",
            allocation.port()
        ));
    }
    script.push_str("  }\n}\n");
    script
}

#[cfg(unix)]
async fn apply_nftables(
    listen_port: u16,
    allocations: &BTreeSet<SocketAddr>,
) -> anyhow::Result<()> {
    let replace = run("nft", &["list", "table", "inet", NFT_TABLE])
        .await
        .is_ok();
    run_with_stdin(
        "nft",
        &["-f", "-"],
        &render_nftables(listen_port, allocations, replace),
    )
    .await
}

#[cfg(not(unix))]
async fn apply_nftables(
    _listen_port: u16,
    _allocations: &BTreeSet<SocketAddr>,
) -> anyhow::Result<()> {
    anyhow::bail!("firewall management requires Linux")
}

async fn ensure_iptables_family(
    program: &str,
    restore_program: &str,
    listen_port: u16,
    allocations: &BTreeSet<SocketAddr>,
    ipv6: bool,
) -> anyhow::Result<()> {
    if run(program, &["-t", "nat", "-N", IPT_CHAIN]).await.is_err() {
        run(program, &["-t", "nat", "-F", IPT_CHAIN]).await?;
    }
    if run(program, &["-t", "nat", "-C", "PREROUTING", "-j", IPT_CHAIN])
        .await
        .is_err()
    {
        run(
            program,
            &["-t", "nat", "-I", "PREROUTING", "1", "-j", IPT_CHAIN],
        )
        .await?;
    }
    let mut rules = format!("*nat\n:{IPT_CHAIN} - [0:0]\n-F {IPT_CHAIN}\n");
    for allocation in allocations
        .iter()
        .filter(|allocation| allocation.is_ipv6() == ipv6)
    {
        let destination = if allocation.ip().is_unspecified() {
            String::new()
        } else {
            format!("-d {} ", allocation.ip())
        };
        rules.push_str(&format!(
            "-A {IPT_CHAIN} -p tcp {destination}--dport {} -m comment --comment \"Calagopus Minecraft MOTD\" -j REDIRECT --to-ports {listen_port}\n",
            allocation.port()
        ));
    }
    rules.push_str("COMMIT\n");
    run_with_stdin(restore_program, &["--noflush"], &rules).await
}

async fn apply_iptables(
    listen_port: u16,
    allocations: &BTreeSet<SocketAddr>,
) -> anyhow::Result<()> {
    ensure_iptables_family(
        "iptables",
        "iptables-restore",
        listen_port,
        allocations,
        false,
    )
    .await?;
    if command_exists("ip6tables").await && command_exists("ip6tables-restore").await {
        ensure_iptables_family(
            "ip6tables",
            "ip6tables-restore",
            listen_port,
            allocations,
            true,
        )
        .await?;
    }
    Ok(())
}

async fn cleanup_iptables_family(program: &str) {
    while run(program, &["-t", "nat", "-D", "PREROUTING", "-j", IPT_CHAIN])
        .await
        .is_ok()
    {}
    let _ = run(program, &["-t", "nat", "-F", IPT_CHAIN]).await;
    let _ = run(program, &["-t", "nat", "-X", IPT_CHAIN]).await;
}

async fn command_exists(program: &str) -> bool {
    tokio::process::Command::new(program)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}

async fn run(program: &str, args: &[&str]) -> anyhow::Result<()> {
    let output = tokio::process::Command::new(program)
        .args(args)
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!(
            "{} {} failed: {}",
            program,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

async fn run_with_stdin(program: &str, args: &[&str], input: &str) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;

    let mut child = tokio::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("{program} stdin unavailable"))?
        .write_all(input.as_bytes())
        .await?;
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        anyhow::bail!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nftables_rules_include_the_complete_destination() {
        let allocations = BTreeSet::from([
            "192.0.2.10:25565".parse().unwrap(),
            "[2001:db8::10]:25566".parse().unwrap(),
        ]);
        let script = render_nftables(4001, &allocations, true);
        assert!(script.contains("ip daddr 192.0.2.10 tcp dport 25565"));
        assert!(script.contains("ip6 daddr 2001:db8::10 tcp dport 25566"));
        assert!(script.contains("redirect to :4001"));
        assert!(script.starts_with("delete table inet calagopus_motd"));
        assert!(!script.contains("udp"));
    }
}
