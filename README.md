# Minecraft MOTD for Calagopus

`ily.gfs.minecraftmotd` 是適用於 Calagopus `>=1.2.2,<2.0.0` 的 Minecraft Java Edition MOTD 擴充套件。它由 Panel 擴充套件及每台 Wings 節點上的 root 代理組成。

本專案不包含離線大廳、不代理玩家流量，也不會在伺服器啟動完成後把既有連線轉送至遊戲伺服器。每次非 running 連線只會收到該狀態的 Server List Ping 回覆或登入踢出訊息，然後中斷。

## 功能

- 固定狀態優先序：`suspended → node_maintenance → transferring → installing → install_failed → restoring_backup → backup_restore_failed → node_unreachable → starting → stopping → offline → running`。
- Wings 回報 running 後，代理先探測 Java TCP 埠；尚未監聽時仍顯示 starting，成功監聽才移除攔截並直接放行。
- 每個非 running 狀態可設定輪播 MOTD、版本、協定、線上／最大玩家數與登入踢出訊息。
- 支援 `{server_name}`、`{node_name}`、`{state}`、`§`／`&` 色碼、兩行交換及多段十六進位漸層。
- 原創 64×64 預設圖示與 64×64 PNG 自訂上傳。
- 可排除 Egg UUID／伺服器 UUID；預設只攔截主要 allocation，可切換為全部 allocations。
- 每服「離線連線自動啟動」預設關閉。只有 next-state 為 login 的 Java 握手會觸發；Server List Ping/Pong 不會觸發。
- Panel 在喚醒前重新驗證代理節點、allocation、排除規則、伺服器狀態及每服開關，並以 30 秒資料庫鎖去重。
- 代理以 nftables 為優先，iptables/ip6tables 為後備，只建立及清理 `calagopus_motd`／`CALAGOPUS_MOTD` 專屬規則。
- 支援 IPv4、IPv6、分段封包、`SO_ORIGINAL_DST` allocation 驗證、Status Ping/Pong 及 Login Disconnect JSON Chat Component。
- 每 2 秒同步快照；Panel 失聯 15 秒後保留映射、停用喚醒並顯示 node-unreachable。

## 安裝 Panel 擴充套件

1. 確認 Calagopus 使用可編譯擴充套件的 heavy 映像，版本為 1.2.2 或相容的 1.x。
2. 在管理後台的 Extensions 頁上傳 `dist/ily_gfs_minecraftmotd.c7s.zip`，等待後端及前端重建完成後重新啟動 Panel。
3. 在 Minecraft MOTD 管理頁設定狀態文字、圖示、排除清單及 allocation 模式。
4. 在每台節點旁產生一次性註冊權杖。權杖 15 分鐘後失效，且成功使用後立即刪除。

## 安裝節點代理：systemd

在對應 Wings 主機選擇架構後執行：

```bash
chmod +x packaging/install-agent.sh
sudo packaging/install-agent.sh \
  dist/agent/calagopus-minecraft-motd-agent-x86_64 \
  https://panel.example.com \
  YOUR_ONE_TIME_ENROLLMENT_TOKEN
```

aarch64 主機改用 `calagopus-minecraft-motd-agent-aarch64`。代理預設監聽 `4001/tcp`；可在安裝命令第四個參數指定其他本機未使用埠。這個埠不需要對外開放。

代理需要 root／`CAP_NET_ADMIN` 以管理 NAT 規則及讀取原始目的地。安裝後可用：

```bash
systemctl status calagopus-minecraft-motd-agent
journalctl -u calagopus-minecraft-motd-agent -f
```

撤銷時，先在 Panel 管理頁撤銷節點憑證，再執行：

```bash
sudo packaging/uninstall-agent.sh --purge
```

systemd 以 SIGTERM 正常停止代理；代理會在退出前只移除自己的防火牆 table／chain。

## 安裝節點代理：Docker Compose

在每台 **Linux Wings 主機**各部署一個 agent 容器。Docker 映像會從 `agent/` 原始碼建置，支援 Linux `amd64` 與 `arm64`；不需要先執行 `scripts/build-agents.sh`。以下命令都在本專案根目錄執行，主機須已安裝具有主機網路與 `NET_ADMIN` 權限的 rootful Docker Engine 及 Compose 外掛。

容器使用主機網路，才能更新主機的 NAT 規則、接收轉送到本機的連線並查詢原始目的位址。它與 Wings 及遊戲伺服器使用不同容器，但**不具備網路隔離**。Compose 只授予 `NET_ADMIN`、`NET_RAW`，並以唯讀方式掛載節點憑證。請確認預設的 `4001/tcp`（或自訂監聽埠）未被占用，且主機防火牆不允許外部直接連入該埠；主機網路模式下不使用 Docker 的 `ports` 發布設定。

首次安裝先在 Panel 管理頁取得該節點的一次性註冊權杖，再執行：

```bash
sudo docker compose -f packaging/compose.yaml build agent
sudo install -d -m 0700 /etc/calagopus-motd-agent
sudo docker run --rm --network host --cap-drop ALL \
  --mount type=bind,src=/etc/calagopus-motd-agent,dst=/etc/calagopus-motd-agent \
  calagopus-minecraft-motd-agent:local enroll \
  --panel-url https://panel.example.com \
  --enrollment-token YOUR_ONE_TIME_ENROLLMENT_TOKEN \
  --listen-port 4001 \
  --output /etc/calagopus-motd-agent/config.toml
sudo docker compose -f packaging/compose.yaml up -d --no-build agent
```

若使用其他未占用埠，修改註冊命令的 `--listen-port`；執行中的 agent 從同一份設定讀取埠號。查看狀態與日誌：

```bash
sudo docker compose -f packaging/compose.yaml ps
sudo docker compose -f packaging/compose.yaml logs -f agent
```

在測試節點上，依序檢查離線伺服器的 MOTD、啟動後的直接連線、離線登入觸發的自動啟動，以及節點有提供的 IPv4／IPv6 allocations。停止容器後，確認 `calagopus_motd` table 或 `CALAGOPUS_MOTD` chain 已被清除。若主機無法使用 nftables，將主機設定檔中的 `firewall_backend` 設為 `"iptables"` 後重新啟動容器。

若已透過 systemd 安裝，**不需要重新註冊**。先停止舊服務，確認它完成防火牆規則清理，再啟動容器；兩者不能同時運行：

```bash
sudo docker compose -f packaging/compose.yaml build agent
sudo systemctl disable --now calagopus-minecraft-motd-agent.service
sudo docker compose -f packaging/compose.yaml up -d --no-build agent
```

遷移後若需回到 systemd，依序停止容器並啟動舊服務：

```bash
sudo docker compose -f packaging/compose.yaml down
sudo systemctl enable --now calagopus-minecraft-motd-agent.service
```

Compose 停止時會傳送 SIGTERM，並等待最多 30 秒讓 agent 清除自己的防火牆規則。正式撤銷節點時，先在 Panel 管理頁撤銷憑證，再執行 `sudo docker compose -f packaging/compose.yaml down`；設定檔位於主機的 `/etc/calagopus-motd-agent/config.toml`。

## 使用 GitHub Actions 建置 Docker 映像

將 **`calagopus-minecraft-motd/` 的內容作為 GitHub 儲存庫根目錄**。`.github/workflows/agent-image.yml` 在 pull request 和一般分支推送時檢查 `linux/amd64`、`linux/arm64` 建置；推送到儲存庫的預設分支時，另外發布 `ghcr.io/OWNER/REPO:latest` 與 `:sha-...`；推送 `v*` 標籤時發布對應版本標籤。`OWNER/REPO` 自動取自 GitHub 儲存庫名稱並轉為小寫。發布使用 GitHub 提供的 `GITHUB_TOKEN`，不需另設推送憑證。

若 GitHub 儲存庫尚未建立，先在 GitHub 建立一個空儲存庫，再從本專案目錄推送；將網址換成自己的儲存庫：

```bash
cd calagopus-minecraft-motd
git init -b main
git add .
git commit -m "Add Minecraft MOTD agent and Docker build"
git remote add origin https://github.com/OWNER/REPO.git
git push -u origin main
```

在 GitHub 的 **Actions → Build MOTD agent image** 查看建置結果。若要讓 Wings 主機免登入拉取映像，將 GHCR package 設為公開；私人 package 需先在節點以具備 `read:packages` 權限的憑證登入 `ghcr.io`。

在每台 Linux Wings 主機取用 GitHub 建好的映像時，仍保留本專案的 `packaging/compose.yaml`，從專案根目錄執行：

```bash
printf 'MOTD_AGENT_IMAGE=ghcr.io/OWNER/REPO:latest\n' > packaging/image.env
sudo docker compose --env-file packaging/image.env -f packaging/compose.yaml pull agent
```

首次安裝需先取得 Panel 一次性註冊權杖，並使用相同的 GHCR 映像執行上方「首次安裝」中的 `docker run ... enroll` 命令（將映像名稱換成 `ghcr.io/OWNER/REPO:latest`）；已有 systemd 設定的節點可直接沿用。確認舊 agent 已停止後啟動：

```bash
sudo docker compose --env-file packaging/image.env -f packaging/compose.yaml up -d --no-build agent
```

日後更新時，重新執行 `pull agent` 與 `up -d --no-build agent`。Compose 會停止舊容器，讓 agent 清理防火牆規則後啟動新映像。

## 每服自動啟動

伺服器擁有者與具有 `settings.motd` 權限的子使用者可在伺服器 Settings 頁切換。資料庫預設固定為 `false`；變更會寫入 `server:motd.update` 活動紀錄。使用者第一次登入離線伺服器時會收到 starting 踢出訊息，需稍後重新連線。

## API

- `GET/PUT /api/admin/minecraft-motd/settings`
- `POST /api/admin/minecraft-motd/nodes/{node}/enrollment-token`
- `DELETE /api/admin/minecraft-motd/nodes/{node}/agent`
- `GET/PUT /api/client/servers/{server}/minecraft-motd`
- `POST /api/minecraft-motd/agent/v1/enroll`
- `GET /api/minecraft-motd/agent/v1/snapshot`
- `POST /api/minecraft-motd/agent/v1/wake`
- `POST /api/minecraft-motd/agent/v1/heartbeat`

代理憑證是 256-bit 隨機值，Panel 僅保存 SHA-256 雜湊。每個憑證只能取得其註冊節點的伺服器／allocation，wake API 也不能指定其他節點或未授權連接埠。

## 從原始碼建置

Panel 套件（Windows PowerShell）：

```powershell
./scripts/build-extension.ps1
```

Linux 代理（需 Rust、Zig 與 cargo-zigbuild）：

```bash
./scripts/build-agents.sh
```

品質檢查：

```bash
cd agent
cargo fmt --check
cargo test
cargo clippy --target x86_64-unknown-linux-musl --all-targets -- -D warnings
```

Panel 後端需放入 Calagopus 1.2.2 workspace 的 `backend-extensions` 後執行 Cargo 測試；前端放入 `frontend/extensions` 後執行 `pnpm biome:validate`、`pnpm typecheck` 與 `pnpm build:ci`。

## 安全提醒

參考資料夾中的舊 Pterodactyl 外掛設定含有可辨識為 API 金鑰的內容。本專案未讀取、使用或封裝該憑證；部署前應在原 Pterodactyl Panel 撤銷並輪替。請勿把 `/etc/calagopus-motd-agent/config.toml` 提交到版本控制。
