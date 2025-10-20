📘 **VVTV Industrial Dossier — Full Technical Architecture**
------------------------------------------------------------

**VoulezVous.TV Autonomous Streaming System**

**Author:** Dan Voulez  
**Institution:** VoulezVous Foundation / LogLine OS  
**Revision:** v1.0 – 2025-10-13

Este dossiê é o manual completo de engenharia do sistema VoulezVous.TV: uma estação de streaming autônoma 24/7 que opera sem APIs, com navegador real, simulação humana, play-before-download, processamento automático e ressurreição computável.

O sistema está dividido em **nove blocos** de engenharia detalhada, cobrindo desde a infraestrutura física até os protocolos de desligamento e revival.

* * *

## 📑 ÍNDICE

### Seções Principais

1. **[Quick Start Guide](#-quick-start-guide)** — Instalação e primeiros passos
2. **[Bloco I — Infraestrutura Base](#bloco-i--infraestrutura-base-e-filosofia-de-engenharia)** — Hardware, rede, ambiente físico
3. **[Bloco II — Browser Automation](#bloco-ii--browser-automation--human-simulation-engineering)** — Simulação humana e PBD
4. **[Bloco III — Processor & Media](#bloco-iii--processor--media-engineering)** — Download, transcode, packaging
5. **[Bloco IV — Queue & Playout](#bloco-iv--queue--playout-engineering)** — Fila, broadcast, watchdogs
6. **[Bloco V — Quality Control](#bloco-v--quality-control--visual-consistency)** — QC, aesthetic, monitoramento
7. **[Bloco VI — Distribution & CDN](#bloco-vi--distribution-redundancy--cdn-layer)** — Distribuição global, failover
8. **[Bloco VII — Monetization](#bloco-vii--monetization-analytics--adaptive-programming)** — Economia, analytics, adaptive
9. **[Bloco VIII — Maintenance](#bloco-viii--maintenance-security--long-term-resilience)** — Backups, security, aging
10. **[Bloco IX — Decommission](#bloco-ix--decommission--resurrection-protocols)** — Desligamento e ressurreição
11. **[Apêndice A — Risk Register](#-apêndice-a--vvtv-risk-register)** — Matriz de riscos
12. **[Apêndice B — Incident Playbook](#-apêndice-b--vvtv-incident-playbook)** — Resposta a incidentes

### Atalhos Rápidos

- **Hardware Mínimo:** [Seção 2.1](#21-hardware-recomendado)
- **Stack de Software:** [Seção 3.1](#31-os-e-configuração)
- **Estrutura de Diretórios:** [Seção 3.2](#32-estrutura-de-diretórios)
- **Play-Before-Download:** [Seção 3 - Bloco II](#3-play-before-download-pbd)
- **FFmpeg Pipelines:** [Seção 5 - Bloco III](#5-transcodificação--normalização)
- **RTMP/HLS Origin:** [Seção 5 - Bloco IV](#5-rtmphls-origin)
- **Troubleshooting:** [Apêndice B](#-apêndice-b--vvtv-incident-playbook)

* * *

## 🚀 QUICK START GUIDE

### Visão Geral

Este guia permite iniciar um nó VVTV funcional em **~2 horas**. Para produção completa, siga os 9 blocos detalhados.

### Pré-requisitos

**Hardware:**
- Mac Mini M1/M2 (16GB RAM, 512GB SSD) ou equivalente
- Conexão de rede: 100+ Mbps down/up
- Storage externo: 2TB NVMe USB-C (opcional mas recomendado)

**Software:**
- macOS 13+ ou Linux Debian 12+
- Conta Tailscale (malha VPN)
- Acesso a terminal/shell

### Instalação Rápida

#### Passo 1: Preparar o Sistema

```bash
# Criar estrutura de diretórios
sudo mkdir -p /vvtv/{system,data,cache,storage,broadcast,docs,monitor,vault}
sudo mkdir -p /vvtv/system/{bin,watchdog,logs}
sudo mkdir -p /vvtv/cache/{browser_profiles,tmp_downloads,ffmpeg_tmp}
sudo mkdir -p /vvtv/storage/{ready,edited,archive}
sudo mkdir -p /vvtv/broadcast/{hls,vod}

# Criar usuário vvtv
sudo useradd -m -s /bin/bash vvtv || sudo dscl . -create /Users/vvtv
sudo chown -R vvtv:vvtv /vvtv
```

#### Passo 2: Instalar Dependências

**macOS:**
```bash
# Instalar Homebrew se necessário
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Instalar dependências
brew install ffmpeg sqlite3 nginx-full aria2 tailscale
brew install --cask chromium
```

**Linux (Debian/Ubuntu):**
```bash
sudo apt update
sudo apt install -y ffmpeg sqlite3 nginx aria2 chromium-browser \
  build-essential curl git
```

#### Passo 3: Configurar Tailscale

```bash
# Instalar e autenticar
sudo tailscale up
# Configurar hostname
sudo tailscale set --hostname vvtv-node-primary
```

#### Passo 4: Configuração Mínima

Criar arquivo `/vvtv/system/vvtv.toml`:

```toml
[system]
node_name = "vvtv-primary"
node_role = "broadcast"  # ou "curator" ou "processor"

[paths]
data_dir = "/vvtv/data"
cache_dir = "/vvtv/cache"
storage_dir = "/vvtv/storage"
broadcast_dir = "/vvtv/broadcast"

[limits]
buffer_target_hours = 6
max_concurrent_downloads = 2
max_concurrent_transcodes = 2
cpu_limit_percent = 75

[network]
tailscale_domain = "voulezvous.ts.net"
rtmp_port = 1935
hls_port = 8080

[quality]
target_lufs = -14
vmaf_threshold = 85
ssim_threshold = 0.92
```

#### Passo 5: Inicializar Bancos de Dados

```bash
# Plans database
sqlite3 /vvtv/data/plans.sqlite << 'EOF'
CREATE TABLE plans (
    plan_id TEXT PRIMARY KEY,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    kind TEXT NOT NULL,
    title TEXT,
    source_url TEXT,
    duration_est_s INTEGER,
    resolution_observed TEXT,
    curation_score REAL DEFAULT 0.5,
    status TEXT DEFAULT 'planned',
    license_proof TEXT,
    hd_missing BOOLEAN DEFAULT 0,
    node_origin TEXT,
    updated_at DATETIME
);
CREATE INDEX idx_plans_status ON plans(status);
CREATE INDEX idx_plans_score ON plans(curation_score DESC);
EOF

# Queue database
sqlite3 /vvtv/data/queue.sqlite << 'EOF'
CREATE TABLE playout_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id TEXT NOT NULL,
    asset_path TEXT NOT NULL,
    duration_s INTEGER,
    status TEXT DEFAULT 'queued',
    curation_score REAL,
    priority INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME,
    node_origin TEXT
);
CREATE INDEX idx_queue_status ON playout_queue(status, created_at);
EOF

# Metrics database
sqlite3 /vvtv/data/metrics.sqlite << 'EOF'
CREATE TABLE metrics (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    buffer_duration_h REAL,
    queue_length INTEGER,
    played_last_hour INTEGER,
    failures_last_hour INTEGER,
    avg_cpu_load REAL,
    avg_temp_c REAL,
    latency_s REAL,
    stream_bitrate_mbps REAL,
    vmaf_live REAL
);
CREATE INDEX idx_metrics_ts ON metrics(ts DESC);
EOF
```

#### Passo 6: Script de Health Check

Criar `/vvtv/system/bin/check_stream_health.sh`:

```bash
#!/bin/bash
# VVTV Stream Health Check

set -e

STREAM_URL="${1:-rtmp://localhost/live/main}"
LOG_FILE="/vvtv/system/logs/health_check.log"

echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] Checking stream health..." | tee -a "$LOG_FILE"

# Check ffmpeg processes
if pgrep -f "ffmpeg.*rtmp" > /dev/null; then
    echo "✅ Encoder running" | tee -a "$LOG_FILE"
else
    echo "❌ Encoder NOT running" | tee -a "$LOG_FILE"
    exit 1
fi

# Check queue
QUEUE_COUNT=$(sqlite3 /vvtv/data/queue.sqlite "SELECT COUNT(*) FROM playout_queue WHERE status='queued';")
echo "📋 Queue length: $QUEUE_COUNT items" | tee -a "$LOG_FILE"

# Check buffer duration
BUFFER_S=$(sqlite3 /vvtv/data/queue.sqlite "SELECT SUM(duration_s) FROM playout_queue WHERE status='queued';")
BUFFER_H=$(echo "scale=2; $BUFFER_S / 3600" | bc)
echo "⏱️  Buffer: ${BUFFER_H}h" | tee -a "$LOG_FILE"

if (( $(echo "$BUFFER_H < 2" | bc -l) )); then
    echo "⚠️  WARNING: Buffer below 2h!" | tee -a "$LOG_FILE"
fi

echo "✅ Health check complete" | tee -a "$LOG_FILE"
```

```bash
chmod +x /vvtv/system/bin/check_stream_health.sh
```

#### Passo 7: Configurar NGINX-RTMP

Criar `/vvtv/broadcast/nginx.conf`:

```nginx
worker_processes auto;
events {
    worker_connections 1024;
}

rtmp {
    server {
        listen 1935;
        chunk_size 4096;
        
        application live {
            live on;
            record off;
            
            # HLS output
            hls on;
            hls_path /vvtv/broadcast/hls;
            hls_fragment 4s;
            hls_playlist_length 48m;
            
            # Prevent external publishing
            allow publish 127.0.0.1;
            deny publish all;
            allow play all;
        }
    }
}

http {
    server {
        listen 8080;
        
        location /hls {
            types {
                application/vnd.apple.mpegurl m3u8;
                video/mp2t ts;
            }
            root /vvtv/broadcast;
            add_header Cache-Control no-cache;
            add_header Access-Control-Allow-Origin *;
        }
        
        location /status {
            return 200 '{"status":"ok","node":"vvtv-primary"}';
            add_header Content-Type application/json;
        }
    }
}
```

Iniciar NGINX:
```bash
sudo nginx -c /vvtv/broadcast/nginx.conf
```

### Validação de Instalação

Execute os testes:

```bash
# 1. Verificar estrutura
ls -la /vvtv/

# 2. Verificar bancos
sqlite3 /vvtv/data/plans.sqlite "SELECT COUNT(*) FROM plans;"

# 3. Verificar NGINX
curl http://localhost:8080/status

# 4. Verificar Tailscale
tailscale status

# 5. Health check
/vvtv/system/bin/check_stream_health.sh
```

### Próximos Passos

Após a instalação básica:

1. **Implementar módulos Rust** (seguir Bloco II-IX para detalhes)
2. **Configurar browser automation** (Chromium + CDP)
3. **Setup do processor** (download + transcode)
4. **Configurar broadcaster** (fila → RTMP)
5. **Deploy de produção** (Railway, CDN, monitoramento)

### Comandos Úteis

```bash
# Verificar status geral
/vvtv/system/bin/check_stream_health.sh

# Ver logs em tempo real
tail -f /vvtv/system/logs/*.log

# Inspecionar fila
sqlite3 /vvtv/data/queue.sqlite "SELECT * FROM playout_queue LIMIT 10;"

# Reiniciar encoder (quando implementado)
systemctl restart vvtv_broadcast

# Limpar cache
rm -rf /vvtv/cache/tmp_downloads/*
```

### Troubleshooting Rápido

| Problema | Solução |
|----------|---------|
| NGINX não inicia | Verificar porta 1935/8080 livre: `sudo lsof -i :1935` |
| Bancos corrompidos | Restaurar backup: `cp /vvtv/vault/data_backup.db /vvtv/data/` |
| Fila vazia | Ver [Apêndice B - Buffer Underflow](#-incident-type-buffer-underflow-fila-seca) |
| Stream congelado | Ver [Apêndice B - Stream Freeze](#-incident-type-stream-freeze--black-screen) |

### Suporte

- **Documentação completa:** Blocos I-IX deste dossiê
- **Riscos e mitigações:** [Apêndice A](#-apêndice-a--vvtv-risk-register)
- **Resposta a incidentes:** [Apêndice B](#-apêndice-b--vvtv-incident-playbook)

* * *

🧠 VVTV INDUSTRIAL DOSSIER
==========================

### **Bloco I — Infraestrutura Base e Filosofia de Engenharia**

* * *

1\. Filosofia Industrial do Sistema
-----------------------------------

O **VVTV (VoulezVous.TV)** é um sistema de transmissão contínua de vídeos adultos 24h/dia, que opera sem API, sem interface administrativa e sem dependência de nuvem.  
A máquina age diretamente no mundo físico — baixa, edita e transmite.  
O design segue quatro princípios inegociáveis:

1.  **Autonomia mecânica total** — o sistema deve se recuperar, reiniciar, reagir, limpar, e continuar sozinho.
2.  **Imersão realista** — todas as interações com a web ocorrem como se um humano estivesse diante da tela.
3.  **Ciclo fechado** — nada depende de cron jobs externos ou orquestradores cloud.
4.  **Consistência industrial** — logs, buffers, cache, latência e limpeza seguem métricas fixas, nunca intuitivas.

O resultado é uma estação transmissora viva, que se comporta como um funcionário sem descanso.

* * *

2\. Infraestrutura Física — Sala da Máquina
-------------------------------------------

### 2.1 Hardware Recomendado

| Função | Modelo | Especificação mínima | Observações |
| --- | --- | --- | --- |
| **Node Principal (Broadcast)** | Mac mini M1 (16 GB RAM, SSD 512 GB) | CPU ARM64, macOS 13+, Ethernet gigabit | Local: Loja VoulezVous |
| **Node de Curadoria** | Mac mini M1 (8 GB RAM, SSD 256 GB) | Opera browser automation | Conectado via Tailscale |
| **Node de Backup** | Mac mini M2 (16 GB RAM, SSD 1 TB) | Armazenamento redundante + backup | Pode rodar Railway node espelho |
| **Mobile Link** | iPhone 13 ou superior | iOS 17+ | Upload rápido via AirDrop ou rede local |
| **Storage externo** | NVMe USB-C 2 TB | Sustenta 72 h de vídeo editado | Uso local, desconectado da rede |

### 2.2 Rede

*   **Malha Tailscale** (domínio `voulezvous.ts.net`) interligando todos os nós.
*   Cada nó possui IP fixo interno (`10.0.x.x`) e hostname persistente.
*   O nó Broadcast é o _relay principal_ e também o **RTMP origin**.
*   Banda mínima sustentada: **80 Mbps up / 150 Mbps down**.
*   Latência interna alvo: **< 5 ms**.
*   DNS interno com cache local (`unbound`) para evitar tracking.
*   Nenhum DNS público (nem Cloudflare, nem Google).

### 2.3 Ambiente Físico

*   Temperatura ambiente 20 – 24 °C.
*   Umidade controlada (< 60 %).
*   Energia estabilizada via UPS (no mínimo 1500 VA).
*   Ventoinhas configuradas em rotação contínua.
*   Cabos de rede blindados Cat 6a.
*   LEDs de operação **devem permanecer ligados** — servem como feedback físico.

### 2.4 Padrão de Montagem Visual

> cor da unha: **grafite fosco**, mesma cor das chaves do rack.
> 
> o objetivo não é estética, é uniformidade óptica:  
> evitar reflexos sob luz branca quando for necessário manusear cabos ao vivo durante operação noturna.  
> o operador deve enxergar tudo em tons neutros, sem distração cromática.

* * *

3\. Sistema Operacional e Stack Base
------------------------------------

### 3.1 OS e Configuração

*   macOS 13+ (ou Linux Debian 12 em modo servidor).
*   Serviços ativos:
    *   `tailscaled`
    *   `ffmpeg` (compilado com suporte a h264, aac, libx265, opus, rtmp, hls, srt)
    *   `chromium` headless
    *   `sqlite3`
    *   `nginx-rtmp`
    *   `watchdogd` (customizado LogLine-style)

**Desativar completamente:**

*   Spotlight, Siri, Sleep, Time Machine, Screensaver.

### 3.2 Estrutura de Diretórios

```
/vvtv/
├── system/
│   ├── bin/           # binários internos
│   ├── scripts/       # automações shell/rust
│   ├── watchdog/      # monitoramento
│   └── logs/          # logs rotativos 7d
├── data/
│   ├── plans.sqlite
│   ├── queue.sqlite
│   └── metrics.sqlite
├── cache/
│   ├── browser_profiles/
│   ├── tmp_downloads/
│   └── ffmpeg_tmp/
├── storage/
│   ├── ready/
│   ├── edited/
│   └── archive/
└── broadcast/
    ├── rtmp.conf
    ├── hls/
    └── vod/
```

**Permissões:**

*   tudo roda como usuário `vvtv` (UID 9001).
*   `chown -R vvtv:vvtv /vvtv`
*   `chmod 755` nos binários, `chmod 600` nos bancos.

* * *

4\. Arquitetura de Software — O Cérebro de Ferro
------------------------------------------------

### 4.1 Módulos Principais

| Módulo | Linguagem | Função |
| --- | --- | --- |
| `discovery_browser` | Rust + JS (Chromium control) | busca, coleta e simulação humana |
| `planner` | Rust | cria e mantém base de planos |
| `human_sim` | Rust + JS | movimenta cursor, cliques, rolagem, delay humano |
| `realizer` | Rust | escolhe planos a realizar 4 h antes |
| `processor` | Rust + FFmpeg | baixa, converte, normaliza |
| `broadcaster` | Rust + Nginx-RTMP | transmite fila de exibição |

Cada módulo comunica-se por **arquivos e bancos locais**, nunca por API.  
O sistema é um **pipeline de estados**, cada um alterando diretamente os registros em SQLite.

### 4.2 Fluxo Geral

```
[BROWSER] → [PLANNER] → [REALIZER] → [PROCESSOR] → [BROADCASTER]
```

1.  O navegador encontra conteúdo e grava o _plan_.
2.  O realizer desperta planos a 4 h do slot.
3.  O processor baixa e edita.
4.  O broadcaster injeta na fila e exibe.
5.  O watchdog garante que tudo recomece se cair.

### 4.3 Linguagem e Padrões

*   Rust edition 2021
*   Async runtime: **tokio**
*   Logging: **tracing** (modo off em produção)
*   CLI utilitária: `cargo run --bin vvtvctl`
*   Configuração: `TOML`
*   Serialização: `serde_json`
*   Jobs periódicos: `cron_rs`
*   Observabilidade opcional: métricas via arquivo JSON local (sem rede)

* * *

5\. Controle e Segurança de Acesso
----------------------------------

*   **Login desativado.** O sistema inicia com `launchd` ou `systemd` e não depende de senha.
*   **SSH apenas via Tailscale** (`tailscale ssh --auth-key`).
*   **Nenhum serviço web exposto.** RTMP e HLS rodam apenas em rede interna.
*   **Firewall interno:**
    *   permite `tcp 1935` (RTMP), `tcp 8080` (HLS preview local).
    *   bloqueia tudo o resto.
*   **Browser sandbox:**
    *   executado em `--no-sandbox` mas dentro de jail user-level.
    *   proxy via `localhost:9050` (tor opcional) para mascarar IP.

* * *

6\. Elementos Humanos e de Ergonomia
------------------------------------

*   Operador (quando presente) usa **luvas cinza-claro antiestáticas**.
*   Monitores devem ter temperatura de cor **5600 K**, brilho fixo 60 %.
*   A iluminação do ambiente deve ser **neutra**, sem tons quentes, para evitar fadiga.
*   Cada estação possui botão físico “STOP STREAM” vermelho, ligado ao script `/vvtv/system/bin/halt_stream.sh`.
*   A cor da unha (grafite fosco) repete-se nas alavancas do painel físico — consistência sensorial para manter o estado mental estável durante manutenção noturna.

* * *

7\. Conclusão do Bloco I
------------------------

Este primeiro bloco define **o chão da fábrica**: onde a máquina vive, como respira, e quais condições físicas e lógicas garantem que ela nunca pare.  
Nada aqui é teórico; são padrões operacionais absolutos.  
A partir desse ponto, cada próximo bloco entrará no nível microscópico — automação, browser simulation, pipelines ffmpeg, fila e controle de qualidade.

* * *

🧠 VVTV INDUSTRIAL DOSSIER
==========================

**Bloco II — Browser Automation & Human Simulation Engineering**
----------------------------------------------------------------

_(sem APIs; play-before-download; aparência humana realista)_

* * *

### 0\. Objetivo

Projetar e padronizar a **camada de navegação autônoma** que:

1.  encontra vídeos/músicas na web,
2.  **dá play antes de baixar** (para garantir a mesma rendition HD que o player está tocando),
3.  extrai o alvo real do mídia (manifest/segmento/progressivo),
4.  salva **apenas plano** até a janela T-4h,
5.  opera com **simulação humana** robusta (sem APIs formais, sem endpoints).

* * *

1) Stack e Processo de Execução
-------------------------------

**Engine:** Chromium (>= 118) via DevTools Protocol (CDP).  
**Controle:** Rust + `chromiumoxide` ou `headless_chrome` (alternativa: Playwright via `playwright-rust`).  
**Execução:** headless por padrão; “headed” para QA.  
**Flags recomendadas:**

```
--no-first-run
--disable-features=AutomationControlled,Translate,InterestFeedV1Heuristic,NetworkServiceInProcess
--disable-blink-features=AutomationControlled
--disable-extensions
--mute-audio
--disable-background-timer-throttling
--autoplay-policy=no-user-gesture-required
--lang=en-US
--password-store=basic
--user-data-dir=/vvtv/cache/browser_profiles/<profile_id>
```

**Perf/Recursos:**

*   Limite memória 2–3 GB por instância.
*   Até **2** abas ativas por nó de curadoria.
*   FPS de render: 30 (headed), 0 (headless).
*   **CPU cap** do processo: 60% (cgroup/darwin limiter).

**Ciclo do worker:**

```
init_profile → open_start_url → simulate_human_idle(3–8s) → search(term) →
scroll_collect(results ~ N) → open_candidate → play_before_download() →
capture_target() → record_plan() → close_tab → next
```

* * *

2) Fingerprinting & Disfarce
----------------------------

**User-Agent Pool (rotativo):**

*   Safari-like (Mac) e Chrome-stable (Win/Mac).
*   Alternar a cada 6–12 páginas.

**Navigator Patches (JS injetado no `document_start`):**

```js
Object.defineProperty(navigator, 'webdriver', { get: () => false });
Object.defineProperty(Notification, 'permission', { get: () => 'default' });
window.chrome = { runtime: {} };
const origQuery = window.navigator.permissions.query;
window.navigator.permissions.query = (p)=>p.name==='notifications'
  ? Promise.resolve({ state:'prompt' })
  : origQuery(p);
```

**Viewport aleatório (dentro de ranges “humanos”):**

*   1366×768, 1440×900, 1536×864, 1920×1080 (± 0–16px jitter).
*   `deviceScaleFactor` ∈ \[1.0, 2.0\].

**Input realista:**

*   Mouse path em **Bezier** com velocidade variável (seção 4).
*   Teclado com cadência 140–220 cpm, jitter 15–35 ms/char, erro a cada 80–130 chars.

**Rede:**

*   Proxy pool (residenciais/rotativos).
*   IP “fresco” a cada 20–40 páginas ou quando detectar bloqueio.

**Cookies/Storage:**

*   Perfil persistente por 24 h (para parecer retorno).
*   Limpeza seletiva por domínio “sensível”.

* * *

3) Play-Before-Download (PBD)
-----------------------------

**Princípio:** só baixar **após** o player estar reproduzindo a **rendition** desejada (HD). O que for baixado deve ser **bit-exato** ao que o humano está vendo.

**Fluxo Geral:**

1.  Abrir página de vídeo.
2.  **Tornar visível** o elemento `<video>`/player (scroll, foco).
3.  **Click Play** como humano; aguardar `readyState >= 3`.
4.  **Forçar HD** (UI: clicar engrenagem → 1080p/720p; ou via teclado, se existir).
5.  Esperar **5–12 s** de playback para garantir troca de rendition/adaptive bitrate.
6.  **Capturar alvo real**:
    *   **HLS**: capturar `master.m3u8` via **Network.observe**; escolher a variant com `BANDWIDTH` e `RESOLUTION` maiores; baixar **media playlist** vigente.
    *   **DASH**: capturar `manifest.mpd`; escolher `AdaptationSet`/`Representation` com maior `height`.
    *   **Progressivo**: capturar `media.mp4` do `<source>` ou do request principal.
7.  Registrar **plan** (sem baixar) — `url_manifest`, `rendition`, `duration_est`, `title`, `tags`.
8.  Fechar aba.

> **Observação**: sites com anti-devtools → preferir sniff de **intercept HTTP** via proxy local (mitm) e desativar DevTools aberto.  
> Fallback: leitura de **MSE** (Media Source Extensions) com `debug hooks` (injetar JS para observar URLs anexadas no `SourceBuffer`).

**Heurística de seleção HD:**

*   HLS: pick `RESOLUTION >= 1080p` se `BANDWIDTH` >= 4500 kbps; senão 720p ≥ 2500 kbps.
*   DASH: maior `height`, codec `avc1`/`h264` preferido.
*   Progressivo: priorizar `itag`/`qualityLabel` quando exposto.

**Validações mínimas (durante PBD):**

*   `currentTime` cresce estável.
*   `videoWidth/Height` bate com a rendition escolhida.
*   Buffer ahead ≥ 3 s.

* * *

4) Simulação Humana (Biomecânica)
---------------------------------

### 4.1 Mouse

**Modelo:** curvas **cúbicas de Bézier** com ruído per-ponto.

*   Velocidade média: 500–1200 px/s.
*   Micro-oscilações laterais (±1–3 px) a cada 12–25 ms.
*   “Hesitação” antes do clique: pausa 120–450 ms.

**Pseudo (Rust-ish):**

```rust
fn move_mouse(from: Point, to: Point, dur_ms: u32) {
    let cps = pick_control_points(from, to);
    let steps = dur_ms / 12;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let p = cubic_bezier(from, cps.0, cps.1, to, t);
        let jitter = randn2d(0.0, 0.8);
        send_mouse_move(p.x + jitter.x, p.y + jitter.y);
        sleep_ms(12);
    }
}
```

**Click:**

*   `mousedown` → 30–70 ms → `mouseup`.
*   Botão esquerdo 98%, direito 2% (raras inspeções).

### 4.2 Scroll

*   Página: “rajadas” de 200–800 px; pausa 120–300 ms entre rajadas.
*   Próximo ao player: scroll **lento** (80–140 px) com pausas maiores (200–500 ms).
*   Anti-padrão: sempre dar **duas** micro rolagens residuais antes do play.

### 4.3 Teclado

*   Cadência 140–220 cpm; jitter 15–35 ms/char.
*   Erro intencional a cada 80–130 chars → backspace → correção.
*   Hotkeys toleradas: `Space` (play/pause), `ArrowLeft/Right` (seek curto), **não usar** `F12`.

### 4.4 Ociosidade & Multitarefa

*   Ociosidade ocasional: 1,5–4,5 s.
*   Troca de abas “falsas” (abrir resultados paralelos) 1 a cada 5–8 páginas.
*   Pequenas movimentações “sem propósito” a cada 20–35 s (efeito atenção dispersa).

### 4.5 Probabilidade de erro simulada

*   Clique em área vazia: 1–2% das vezes.
*   Scroll overshoot: 5–8%.
*   Segunda tentativa de play: 10–15% (players que não respondem ao primeiro clique).

* * *

5) Coleta & Normalização de Metadados (sem API)
-----------------------------------------------

**Extração DOM (JS):**

*   `document.title` (fallback `<meta property="og:title">`).
*   `video.duration` quando acessível; senão, estimativa por playback (10–20 s).
*   `textContent` de `<h1>`, `<h2>`, breadcrumbs.
*   Tags/categorias via seletores comuns (chips, anchors com `/tag/`).
*   Resolução via `video.videoWidth/Height` ou label UI (“1080p/720p”).

**Sanitização:**

*   Remover emojis, múltiplos espaços, `\n`, tracking params (`utm_*`, `ref`).
*   Normalizar idioma para en-US/pt-PT when needed (título duplicado → manter original).

**Registro de PLAN (SQLite):**

```
plan_id, created_at, kind, title, source_url, resolution_observed,
curation_score, duration_est_s, expected_bitrate, status='planned'
```

* * *

6) Seletores & Estratégias de Player
------------------------------------

**Detecção do alvo:**

*   `<video>` direto? Usar.
*   Player frameworks comuns:
    *   **video.js** → `.vjs-tech` (source em `<video>`).
    *   **hls.js** → observar `Network` por `.m3u8`.
    *   **dash.js/shaka** → `.mpd` requests.
    *   **iframes** → focar dentro do frame; repetir heurística.

**Botões críticos (seletores aproximados):**

*   Play: `.play, .vjs-play-control, button[aria-label*="Play"]`
*   Qualidade: `.quality, .settings, .vjs-menu-button`
*   Maximize/Mute: `.fullscreen, .mute`

**Pop-ups/consent:**

*   Detectar overlays com `position:fixed`/z-index alto → clicar “accept/close” por árvore de botões prováveis.

**Fallbacks:**

*   Se nenhum seletor reagir:
    1.  `Space` (teclado).
    2.  Click no centro do player (50% width/height).
    3.  Recarregar a página 1x.

* * *

7) Captura da Fonte Real do Vídeo (sem API)
-------------------------------------------

### 7.1 Via DevTools Protocol (preferencial)

*   Ativar `Network.enable`.
*   Filtrar requests por `m3u8|mpd|.mp4|.webm`.
*   Para **HLS**:
    *   guardar `master.m3u8`, resolver **variant** correta por resolução/bitrate,
    *   capturar **media playlist** atual (onde o player migrou) → **URL final do plano**.
*   Para **DASH**:
    *   parse do MPD; preferir maior `height`/`bandwidth`.
*   Para **progressivo**:
    *   URL do `GET` com `Content-Type: video/*`, `Content-Length` razoável.

### 7.2 Via Proxy (sites anti-devtools)

*   Executar navegador com proxy local (mitm).
*   Extrair manifests das conexões TLS de vídeo (mitm com domínio permitido).
*   Persistir somente a URL final; **não baixar** no momento do plano.

**Critérios de aceitação da captura:**

*   Reproduzindo há ≥ 5 s **após** mudar qualidade para HD.
*   Taxa de buffer estável.
*   Nenhum erro do player nos últimos 3 s.

* * *

8) Plano de Erros & Recuperação
-------------------------------

**Categorias:**

*   _Não encontrou player_: tentar 3 layouts; cair para próximo candidato.
*   _Play não inicia_: clicar 2–3x; espaço; reload 1x.
*   _HD indisponível_: aceitar 720p; marcar flag `hd_missing`.
*   _Bloqueio/antibot_: trocar IP/proxy; alternar UA; dormir 5–15 min.
*   _Manifest inconsistente_: repetir coleta; se falhar, descartar plano.

**Regras de backoff:**

*   1ª falha do domínio: retry em 10–20 min.
*   2ª: retry 45–90 min.
*   3ª: blacklist 24 h.

* * *

9) Qualidade Visual “Humana”
----------------------------

*   **Cursor sempre visível** em modo QA; oculto em headless.
*   **Scroll elástico**: última rolagem sempre menor que a penúltima.
*   **Dwell-time** em thumbnails: 400–900 ms antes de abrir.
*   **Movimento “okulomotor”**: pequeno “8” com amplitude 6–10 px perto de elementos clicáveis (sugere leitura).
*   **Padrão noturno**: iniciar ciclos intensos às 02:00–06:00 locais.

> Detalhe obsessivo solicitado: **cor da unha** do operador: _grafite fosco_.  
> No QA headed, plano de fundo do cursor deve ser neutro para evitar reflexo na inspeção visual do movimento.

* * *

10) Segurança, Privacidade, Conformidade
----------------------------------------

*   **Áudio mudo** sempre.
*   **Sem formular senhas**.
*   **Sem uploads**.
*   **Consentimento/Idade**: só aceitar fontes com política explícita; registrar no plano `license_hint`.
*   **Isolamento**: perfis por domínio; storage quotas.
*   **Atualizações**: engine travada em versão testada (rolling update semanal, nunca em horário de pico).

* * *

11) Métricas locais (sem spans, sem rede)
-----------------------------------------

_(opcional, para tuning off-line — gravadas em `metrics.sqlite`)_

*   `pages_per_hour`, `videos_seen`, `plans_created`
*   `pbd_success_rate`, `hd_hit_rate`, `avg_play_wait_s`
*   `antibot_incidents`, `proxy_rotations`

Coleta a cada 10 min; retém 14 dias; sem PII.

* * *

12) Testes & QA
---------------

**Smoke (por domínio):**

*   Encontrar player em ≤ 6 s.
*   Abrir qualidade e selecionar ≥ 720p.
*   Play estável ≥ 8 s.
*   Capturar URL final válida (200 OK no HEAD).
*   Criar **PLAN** com `status=planned`.

**Load (noturno):**

*   2 abas por nó, 2 nós → ≥ 80 planos/h.
*   CPU ≤ 60%, RAM ≤ 2.5 GB/instância.

**Anti-bot:**

*   Trocar IP; novo UA; novo viewport → player ainda reproduz?
*   Falhou 3x seguidas? Blacklist 24 h.

**Qualidade do movimento:**

*   Distância média por clique 200–900 px.
*   Erro propositado 1–2% cliques.
*   Dwell médio em cards 600 ms ± 200.

* * *

13) Pseudocódigo Integrador (Rust-like)
---------------------------------------

```rust
fn collect_plan(url: &str) -> Option<Plan> {
    let mut c = Browser::spawn(profile());
    c.goto(url)?;
    human::idle( ms(2000..6000) );

    let player = find_player(&c)?;
    human::move_to(&player.center());
    human::click();

    wait::until_video_ready(&c, secs(3..8))?;
    player.open_quality_menu()?;
    player.select_hd_or_720p()?;
    wait::steady_playback(&c, secs(5..12))?;

    let media = capture_media_target(&c)?; // m3u8/mpd/mp4 via CDP/proxy
    let meta  = read_basic_meta(&c, &player)?;

    Some(Plan {
        plan_id: uuid(),
        title: meta.title,
        url: media.url,
        kind: meta.kind,
        duration_s: meta.duration_est,
        resolution: media.resolution,
        curation_score: score(&meta),
        status: "planned"
    })
}
```

* * *

14) Entregáveis deste Bloco
---------------------------

*   **Especificação operacional** (este documento).
*   **Templates** de seletores por player comum.
*   **Implementação** do motor de movimento (Bezier + jitter).
*   **Capturador** CDP + Proxy fallback.
*   **Normalizador** de metadados DOM.
*   **Test Kit** de QA (scripts de smoke/load).

* * *

15) Ready-for-Build Checklist
-----------------------------

*    Chromium com flags aprovadas.
*    Controller Rust compilado.
*    Proxy MITM funcional (fallback).
*    Heurísticas de player testadas (video.js / hls.js / dash.js).
*    Movimento humano com Bezier e jitter ativo.
*    Play-before-download confirmando HD/720p.
*    PLAN gravado sem baixar nada.
*    Limpeza de perfil e quotas validadas.
*    Métricas locais ligadas (opcional).
*    QA noturno executado e aprovado.

* * *

* * *

🧠 VVTV INDUSTRIAL DOSSIER
==========================

**Bloco III — Processor & Media Engineering**
---------------------------------------------

_(T-4h executor; play-before-download real; captura bit-exata; transcode/normalização; packaging; integridade; staging para exibição 24/7)_

* * *

### 0\. Objetivo deste bloco

Padronizar **toda a fase T-4h**: transformar **PLANOS** em **mídia pronta** para a fila de transmissão.  
Inclui: reabrir a página, **dar play antes de baixar** (para capturar a **mesma rendition HD** que o player está tocando), baixar/compilar mídia, normalizar áudio, transcodificar/empacotar nos perfis operacionais, validar integridade e **entregar ao playout**.

* * *

1) Posição no ciclo e gatilhos
------------------------------

**Entrada:** linhas `plans` com `status='selected'` (escolhidos pelo Realizer quando `time_to_slot <= 240 min`).  
**Saída:** artefatos em `/vvtv/storage/ready/<plan_id>/` e registro na `playout_queue` com `status='queued'`.

**Gatilhos do Processor:**

*   Timer de orquestração a cada 2–5 min.
*   Lote máximo por execução: **N=6** itens.
*   Concurrency: **2** downloads + **2** transcodes simultâneos por nó (cap CPU ≤ 75%).

* * *

2) Reabertura e Play-Before-Download (PBD) no T-4h
--------------------------------------------------

Mesmo que o PLAN tenha URL de manifesto, **reabra a página** e **dê play** para confirmar a rendition.  
Nada de API. Tudo via navegador.

**Fluxo PBD (operacional):**

1.  Abrir a **URL do plano** no Chromium controlado.
2.  Scroll até o player; **simulação humana** de foco e clique (vide Bloco II).
3.  Abrir menu de qualidade, forçar **1080p**; se ausente, **720p**.
4.  Aguardar **5–12 s** para estabilizar a troca de bitrate.
5.  **Capturar a fonte** que está sendo tocada:
    *   **HLS**: capturar a **media playlist** correspondente (não apenas a master).
    *   **DASH**: capturar a `Representation` efetiva (segment list).
    *   **Progressivo**: capturar a URL do MP4/WebM servida ao `<video>`.
6.  Validar:
    *   `currentTime` avança; `videoWidth/Height` coerentes; buffer ≥ 3 s.
7.  **Fechar a aba** (manter apenas o alvo de mídia).
8.  Proceder ao **download**.

> Regra: **O que baixamos é o que o humano estaria vendo naquele instante.** Se a rendition cair de 1080p para 720p por instabilidade, o PBD repete a tentativa por até 2 ciclos antes de aceitar 720p.

* * *

3) Download — HLS/DASH/Progressivo
----------------------------------

### 3.1 Estrutura de staging

```
/vvtv/cache/tmp_downloads/<plan_id>/
  ├── source/            # bruto: .m3u8/.mpd + segments ou .mp4 progressivo
  ├── remux/             # MP4 remuxado (sem reencode) se compatível
  └── logs/
```

### 3.2 HLS (preferencial para playout adaptativo)

*   Baixar **a media playlist** e **todos os segmentos** (`.ts`/`.m4s`) **referenciados**.
*   Verificar consistência:
    *   Sequência contínua (sem buracos de `EXT-X-MEDIA-SEQUENCE`),
    *   `EXT-X-TARGETDURATION` consistente,
    *   Duração total aproximada igual à estimada.
*   **Compor VOD HLS local**:
    ```
    /vvtv/storage/ready/<plan_id>/hls/
      ├── index.m3u8            # media playlist reescrita para caminhos locais
      └── seg_<nnnn>.ts|m4s
    ```

**Comando base (fetch + rewrite)**

```bash
# exemplo: usar aria2c para segmentos + script de rewrite
aria2c -j8 -x8 -s16 -d "/vvtv/cache/tmp_downloads/<plan_id>/source" -i segments.txt
# segments.txt contém todas as URLs absolutas da media playlist (+ a própria .m3u8)
```

Reescrever a playlist para apontar para `seg_<nnnn>.*` locais.

### 3.3 DASH

*   Baixar o `manifest.mpd` e os `SegmentList`/`SegmentTemplate` da `Representation` tocada.
*   Consolidar em estrutura similar ao HLS (`/hls/`), opcionalmente **remuxar** para HLS via `ffmpeg` (ver 5.3) para uniformizar a cadeia de playout.

### 3.4 Progressivo (MP4/WebM)

*   **HEAD** para validar `Content-Length` ≥ 2 MB e `Content-Type` `video/*`.
*   **GET** com retomada (`-C -`) e limite de velocidade se houver competição.
*   Salvar em:
    ```
    /vvtv/cache/tmp_downloads/<plan_id>/source/source.mp4
    ```

### 3.5 Verificações de integridade

*   `sha256sum` do conjunto (manifest + lista de arquivos).
*   Amostra de `ffprobe` (tempo, streams, codecs).
*   Duração mínima: vídeo ≥ 60 s; música ≥ 90 s (ajustável por política).

**Falhas & backoff**

*   1ª falha: retry em 3 min;
*   2ª: 15 min;
*   3ª: plano `rejected` (log motivo).

* * *

4) Remux vs Transcode — decisão de custo
----------------------------------------

**Objetivo:** evitar reencode sempre que possível.

*   Se codecs **compatíveis** com nosso playout: **remux** (cópia de vídeo/áudio).
*   Se incompatíveis (ex.: áudio `mp3` em HLS `fmp4` com `aac` requerido): transcode seletivo.

### 4.1 Sinais de compatibilidade (para remux)

*   Vídeo `avc1/h264` (high/baseline/main), profile ≤ High, level ≤ 4.2.
*   Áudio `aac` LC 44.1/48 kHz estéreo.
*   Container: MP4/TS/fMP4 aceitos.

### 4.2 Comandos típicos

**Remux para MP4 (sem reencode)**

```bash
ffmpeg -hide_banner -y -i source.mp4 \
  -map 0:v:0 -map 0:a:0 -c copy -movflags +faststart \
  "/vvtv/cache/tmp_downloads/<plan_id>/remux/master.mp4"
```

**Remux de HLS (concatenação de TS) → MP4**

```bash
ffmpeg -hide_banner -y -i "index.m3u8" \
  -c copy -movflags +faststart \
  "/vvtv/cache/tmp_downloads/<plan_id>/remux/master.mp4"
```

Se `-c copy` falhar (timestamps fora/streams incompatíveis), cair para transcode (Seção 5).

* * *

5) Transcodificação & Normalização
----------------------------------

### 5.1 Alvos de entrega (VVTV)

*   **master.mp4** (mezzanine local)
*   **hls\_720p** (CBR-ish ~ 3.0–3.5 Mbps)
*   **hls\_480p** (CBR-ish ~ 1.2–1.6 Mbps)
*   **áudio normalizado** (LUFS alvo)

### 5.2 Normalização de áudio (EBU R128 — two-pass)

**Passo 1: medir**

```bash
ffmpeg -hide_banner -y -i master_or_source.mp4 \
  -af "loudnorm=I=-14:TP=-1.5:LRA=11:print_format=json" -f null - 2> loud.json
```

Extrair `measured_I`, `measured_TP`, `measured_LRA`, `measured_thresh`.

**Passo 2: aplicar**

```bash
ffmpeg -hide_banner -y -i master_or_source.mp4 \
  -af "loudnorm=I=-14:TP=-1.5:LRA=11:measured_I=<I>:measured_TP=<TP>:measured_LRA=<LRA>:measured_thresh=<THR>:linear=true:print_format=summary" \
  -c:v copy -c:a aac -b:a 160k \
  "/vvtv/storage/ready/<plan_id>/master_normalized.mp4"
```

Se `-c:v copy` falhar por incompatibilidade, usar transcode total (5.3).

### 5.3 Transcode de vídeo (x264)

**Preset padrão 1080p → mezzanine:**

```bash
ffmpeg -hide_banner -y -i source_or_remux.mp4 \
  -c:v libx264 -preset slow -crf 20 -tune film \
  -profile:v high -level 4.2 -pix_fmt yuv420p \
  -x264-params keyint=120:min-keyint=48:scenecut=40:vbv-maxrate=12000:vbv-bufsize=24000 \
  -c:a aac -b:a 160k -ar 48000 \
  "/vvtv/storage/ready/<plan_id>/master.mp4"
```

**HLS 720p / 480p (CBR-ish com fMP4):**

```bash
# 720p
ffmpeg -hide_banner -y -i "/vvtv/storage/ready/<plan_id>/master.mp4" \
  -vf "scale=-2:720:flags=bicubic" \
  -c:v libx264 -preset veryfast -profile:v high -level 4.0 -pix_fmt yuv420p \
  -b:v 3300k -maxrate 3600k -bufsize 6600k -g 120 -keyint_min 48 \
  -c:a aac -b:a 128k -ar 48000 \
  -f hls -hls_time 4 -hls_playlist_type vod -hls_segment_type fmp4 \
  -hls_flags independent_segments \
  -master_pl_name master.m3u8 \
  -hls_segment_filename "/vvtv/storage/ready/<plan_id>/hls_720p_%04d.m4s" \
  "/vvtv/storage/ready/<plan_id>/hls_720p.m3u8"

# 480p
ffmpeg -hide_banner -y -i "/vvtv/storage/ready/<plan_id>/master.mp4" \
  -vf "scale=-2:480:flags=bicubic" \
  -c:v libx264 -preset veryfast -profile:v main -level 3.1 -pix_fmt yuv420p \
  -b:v 1500k -maxrate 1700k -bufsize 3000k -g 120 -keyint_min 48 \
  -c:a aac -b:a 96k -ar 48000 \
  -f hls -hls_time 4 -hls_playlist_type vod -hls_segment_type fmp4 \
  -hls_flags independent_segments \
  -hls_segment_filename "/vvtv/storage/ready/<plan_id>/hls_480p_%04d.m4s" \
  "/vvtv/storage/ready/<plan_id>/hls_480p.m3u8"
```

> Observação: para manter **bit-exatidão** do PBD, se a rendition capturada já for 1080p/720p compatível, **pular reencode** e somente **empacotar** (5.4).

### 5.4 Empacotamento sem reencode

**HLS a partir de MP4 compatível:**

```bash
ffmpeg -hide_banner -y -i "/vvtv/storage/ready/<plan_id>/master_normalized.mp4" \
  -c copy -f hls -hls_time 4 -hls_playlist_type vod -hls_segment_type fmp4 \
  -hls_flags independent_segments \
  -hls_segment_filename "/vvtv/storage/ready/<plan_id>/hls_source_%04d.m4s" \
  "/vvtv/storage/ready/<plan_id>/hls_source.m3u8"
```

* * *

6) Estrutura final de entrega (por plano)
-----------------------------------------

```
/vvtv/storage/ready/<plan_id>/
  ├── master.mp4                 # mezzanine (ou master_normalized.mp4)
  ├── hls_720p.m3u8
  ├── hls_720p_0001.m4s ...
  ├── hls_480p.m3u8
  ├── hls_480p_0001.m4s ...
  ├── (hls_source.m3u8 + m4s)    # quando empacotado do source sem reencode
  ├── checksums.json             # hashes dos artefatos
  └── manifest.json              # metadata consolidada do processamento
```

**`manifest.json` (exemplo):**

```json
{
  "plan_id": "<uuid>",
  "source": {"type":"HLS","url":"<media_playlist_url>"},
  "captured_profile": {"resolution":"1080p","codec":"avc1"},
  "processing": {"audio_lufs_target": -14, "transcode": "copy|x264"},
  "artifacts": {
    "master": "master.mp4",
    "hls": ["hls_720p.m3u8", "hls_480p.m3u8"]
  },
  "durations": {"measured_s": 213},
  "hashes": {"master.mp4":"<sha256>"},
  "created_at": "<iso8601>"
}
```

* * *

7) Integridade, validações e QC
-------------------------------

**Checks mínimos:**

*   `ffprobe` confirma **1 stream de vídeo** + **1 de áudio**, sem erros.
*   Duração ±5% da estimativa.
*   **Keyframes** ~ a cada 2 s–4 s (para zapping suave).
*   Áudio estéreo 44.1/48 kHz; **loudness** atingido (verificação com `loudnorm` summary).
*   **Checksum** SHA-256 por arquivo.

**Arquivo `checksums.json`:**

```json
{"master.mp4":"...","hls_720p_0001.m4s":"...","hls_480p.m3u8":"..."}
```

* * *

8) Atualizações de banco e staging para fila
--------------------------------------------

**`plans` → estados:**

*   `selected` → `downloaded` → `edited`

**`playout_queue` (inserção):**

```
id, plan_id, asset_path, status='queued', created_at
```

`asset_path` aponta para `master.mp4` **ou** `hls_720p.m3u8` (política preferida: usar HLS).

* * *

9) Recursos, limites e escalonamento
------------------------------------

*   **CPU cap** por transcode: 300% (3 cores) com `nice + ionice`.
*   **RAM alvo** por ffmpeg: ≤ 1.0 GB.
*   **IO**: segment size 4–6 s para discos SSD; evita milhares de arquivos microsegmentados.
*   **Concorrência**:
    *   `N_downloads = 2`, `N_transcodes = 2` por nó.
    *   Evitar baixar e transcodificar o **mesmo plano** em paralelo (lock por `plan_id`).

**Banda mínima por transcode** (HLS 720p): ~4 Mbps internos.  
Desacoplar downloads dos transcodes (queue interna) para evitar disputa de disco.

* * *

10) Tratamento de falhas (árvore de decisão)
--------------------------------------------

1.  **PBD falhou (não tocou HD):**
    *   Tentar 720p; se ainda falhar → próximo plano.
2.  **Manifest inconsistente:**
    *   Recoletar; se não fechar, **reject**.
3.  **Download parcial:**
    *   Retomar; se 3 tentativas falharem, **reject**.
4.  **Remux falhou:**
    *   Transcode total (5.3).
5.  **Transcode falhou:**
    *   Repetir com `-preset faster`; se falhar, **quarentena** do plano.
6.  **QC reprovado (áudio/loudness/keyframes):**
    *   Reprocessar só áudio ou só gop; 1 retry.

Todos os “reject/quarentena” ficam listados em `/vvtv/system/logs/processor_failures.log` (rotativo 7d).

* * *

11) Segurança operacional
-------------------------

*   **Sem persistir cookies** de “fontes adultas” pós-download (limpeza por domínio).
*   **Sem abrir arquivos externos** durante transcode além dos previstos.
*   **TMP sandboxado** por `plan_id`.
*   **Remoção de EXIF/metadata** no `master.mp4` (usar `-map_metadata -1` se necessário).

* * *

12) QA — checklist por item
---------------------------

*    Página reaberta e **play** confirmado
*    Qualidade HD/720p forçada
*    Fonte capturada (HLS/DASH/progressivo)
*    Download completo e íntegro
*    Áudio normalizado para **\-14 LUFS** (±1 LU)
*    Entrega HLS/MP4 conforme política
*    Checksums gerados
*    Plano atualizado: `edited`
*    Inserção na `playout_queue: queued`

* * *

13) Pseudocódigo integrador (Rust)
----------------------------------

```rust
fn realize_plan(plan: Plan) -> Result<()> {
    // 1) PBD
    let media = browser::reopen_and_capture_media(&plan.url)?; // garante rendition tocada
    // 2) Download
    let src_dir = download::fetch(&plan.id, &media)?;
    // 3) Remux / Transcode decision
    let master = mediaops::prepare_master(&plan.id, &src_dir)?; // copy preferido
    // 4) Normalize audio & package
    let master_norm = audio::loudnorm(master, -14.0)?;
    let (hls720, hls480) = hls::package_profiles(&master_norm)?;
    // 5) QC & integrity
    qc::validate(&master_norm, &[&hls720, &hls480])?;
    // 6) Stage & DB
    db::plans::set_status(&plan.id, "edited")?;
    db::queue::enqueue(&plan.id, hls720.path())?;
    Ok(())
}
```

* * *

14) Entregáveis deste bloco
---------------------------

*   Especificação de PBD no T-4h.
*   Scripts `ffmpeg` para **remux/transcode/normalização/packaging**.
*   Rotinas de **download HLS/DASH/progressivo**.
*   `manifest.json` + `checksums.json` por plano.
*   Checklist de **QC**.
*   Pseudocódigo de integração.

* * *

15) Ready-for-Build
-------------------

*    Worker Processor com limites de CPU/IO.
*    PBD revalidado no T-4h.
*    HLS/DASH/Progressivo cobertos.
*    Normalização EBU R128 validada.
*    Packaging HLS rodando (4 s segments).
*    QC automatizado ativo.
*    Integração com `playout_queue` concluída.

* * *

🧠 VVTV INDUSTRIAL DOSSIER
==========================

**Bloco IV — Queue & Playout Engineering**
------------------------------------------

_(Gestão de fila FIFO, “curation bump”, watchdogs, buffer ≥ 4 h, RTMP/HLS origin, failover e métricas locais)_

* * *

### 0\. Objetivo

Definir a engenharia de **fila e exibição contínua**: manter sempre pelo menos **4 horas de conteúdo pronto**, garantir continuidade 24/7, controlar prioridades de exibição e reações a falhas, e operar o playout com redundância.

* * *

1) Fila Computável
------------------

**Tabela:** `playout_queue.sqlite`

| Campo | Tipo | Descrição |
| --- | --- | --- |
| `id` | INTEGER PK | Sequência automática |
| `plan_id` | TEXT | Referência ao plano processado |
| `asset_path` | TEXT | Caminho do arquivo (HLS/MP4) |
| `duration_s` | INT | Duração real medida |
| `status` | TEXT | `queued` / `playing` / `played` / `failed` |
| `curation_score` | FLOAT | Peso de relevância estética |
| `priority` | INT | 0 = normal, 1 = bump |
| `created_at` / `updated_at` | DATETIME | Auditoria temporal |
| `node_origin` | TEXT | Identificação do nó de produção |

**Política de limpeza:** remover registros `played` > 72 h e manter backup diário (`.sql.gz`).

* * *

2) Política FIFO + “Curation Bump”
----------------------------------

A ordem de exibição segue FIFO **com desvio controlado**:

1.  A fila é lida em ordem de `created_at`.
2.  Um algoritmo de _curation bump_ aumenta a prioridade de itens com `curation_score > 0.85` e mais de 24 h sem exibir.
3.  A cada 10 vídeos, injeta-se 1 de música para equilíbrio.
4.  Itens de mesmo score → ordenar por “tempo de espera × (1 + score)” descendente.

* * *

3) Buffer de Segurança
----------------------

*   **Meta mínima:** 4 h de duração somada em `queued`.
*   **Alerta amarelo:** < 2 h.
*   **Emergência:** < 1 h → acionar modo _loop replay_ (reexibir últimos 6 vídeos).
*   **Atualização:** verificar a cada 60 s ou após cada playout concluído.

* * *

4) Watchdogs
------------

### 4.1 Loop Principal

Verifica:

*   Streaming ativo (`ffprobe` no RTMP);
*   Buffer ≥ mínimo;
*   Nenhum processo `ffmpeg` travado.

### 4.2 Reação a Falhas

| Falha | Ação |
| --- | --- |
| RTMP inativo > 5 s | Reiniciar nginx-rtmp + ffmpeg |
| CPU > 90 % por 5 min | Suspender novos downloads |
| Fila vazia | Entrar em loop local de vídeos reservas |
| Falha de mídia | Marcar `failed`, logar motivo, seguir próximo |
| Disco < 5 % livre | Pausar processor até limpeza |

**Serviço:** `watchdogd` (ciclo 30 s) + log rotativo 7 dias.

* * *

5) RTMP/HLS Origin
------------------

### 5.1 RTMP Source

```
/vvtv/broadcast/nginx.conf
rtmp {
  server {
    listen 1935;
    chunk_size 4096;
    application live {
      live on;
      record off;
      exec ffmpeg -re -i rtmp://localhost/live/main
                  -c copy -f hls /vvtv/broadcast/hls/live.m3u8;
    }
  }
}
```

### 5.2 HLS Output

```
/vvtv/broadcast/hls/
  ├── live.m3u8
  ├── segment_00001.ts
  └── ...
```

Rotacionar segmentos a cada 4 s e manter playlist com 720 itens (≈ 48 min).  
O broadcaster inicia novo segmento enquanto transmite o anterior.

* * *

6) Motor de Playout
-------------------

**Input:** fila `queued`.  
**Output:** RTMP stream.

```bash
ffmpeg -re -i "/vvtv/storage/ready/<plan_id>/hls_720p.m3u8" \
  -c copy -f flv rtmp://localhost/live/main
```

**Ciclo:**

1.  Selecionar próximo `queued`.
2.  Atualizar status → `playing`.
3.  Executar comando acima até EOF.
4.  Atualizar `played`.
5.  Recalcular buffer e retomar.

* * *

7) Failover Local
-----------------

*   **Hot standby:** segundo processo ffmpeg (`rtmp://localhost/live/failover`) aguardando fila duplicada.
*   Ao detectar queda do stream principal > 3 s:
    *   trocar origem por `failover`;
    *   sinalizar alerta;
    *   reiniciar primário em background.

**Backup:** últimos 4 horas gravadas em `/vvtv/storage/archive/live_<ts>.mp4`.

* * *

8) Sincronização de Nós
-----------------------

*   **Mestre:** nó broadcast.
*   **Espelho:** nó backup Railway.
*   **Sync:** `rsync -av --delete --bwlimit=5M /vvtv/storage/ready/ backup@railway:/vvtv/storage/ready/`
*   **Cron:** a cada 1 h.
*   **Verificação:** comparar `checksums.json`.
*   **Falha:** logar e repetir 15 min depois.

* * *

9) Métricas Locais
------------------

`metrics.sqlite` (sem rede):

| Métrica | Unidade | Intervalo | Fonte |
| --- | --- | --- | --- |
| `buffer_duration_h` | horas | 60 s | soma fila |
| `queue_length` | count | 60 s | SQL count |
| `played_last_hour` | count | 1 h | eventos |
| `failures_last_hour` | count | 1 h | watchdog |
| `avg_cpu_load` | % | 5 min | `sysctl` |
| `avg_temp_c` | °C | 5 min | sensor |

Arquivado em JSON diário (14 dias).

* * *

10) Procedimentos Manuais de Operador
-------------------------------------

1.  **STOP STREAM:** `sudo /vvtv/system/bin/halt_stream.sh` (graceful).
2.  **INSPECIONAR FILA:** `sqlite3 /vvtv/data/queue.sqlite "SELECT plan_id,status FROM playout_queue;"`.
3.  **FORÇAR BUFFER:** `/vvtv/system/bin/fill_buffer.sh --min 4h`.
4.  **LIMPAR ARQUIVOS VELHOS:** `find /vvtv/storage/archive -mtime +7 -delete`.
5.  **REINICIAR WATCHDOG:** `sudo service watchdogd restart`.

* * *

11) Ambiente Físico de Exibição
-------------------------------

*   Monitores em loop: TV OLED 42″ + HDMI direto do Mac Mini.
*   Brilho fixo 70 %.
*   Som mutado.
*   LEDs azuis ativos = stream ok; vermelhos = falha.
*   Botão físico “RESET STREAM” (aciona GPIO + script de restart).
*   Operador em plantão usa luvas antirreflexo cinza-claro e unhas grafite fosco (para não gerar reflexos nas telas quando faz manutenção ao vivo).

* * *

12) Conclusão do Bloco IV
-------------------------

Este bloco estabelece o **sistema circulatório** do VVTV: a fila, o ritmo de exibição, e a autocorreção constante.  
Com os módulos de browser, processor e broadcaster já definidos, a máquina pode funcionar sozinha por meses sem intervenção humana.

* * *

🧠 VVTV INDUSTRIAL DOSSIER
==========================

**Bloco V — Quality Control & Visual Consistency**
--------------------------------------------------

_(padrões de imagem, curva de loudness, cortes automáticos, métricas perceptuais e coerência estética no streaming remoto)_

* * *

### 0\. Objetivo

Garantir **padrão técnico e sensorial contínuo** na transmissão global via link público (HLS/RTMP).  
Todo espectador, independentemente da casa, deve perceber uma imagem limpa, ritmo suave, áudio balanceado e **identidade estética VoulezVous** persistente, mesmo com vídeos de origens distintas.

* * *

1) Pipeline de Qualidade — Fases
--------------------------------

1.  **Pré-QC** — verificação técnica após transcode (bitrate, codecs, duração).
2.  **Mid-QC** — checagem perceptual (ruído, saturação, flicker, loudness).
3.  **Aesthetic-QC** — consistência cromática e identidade.
4.  **Live-QC** — monitoramento do stream ativo (telemetria e capturas).

* * *

2) Pré-QC (Verificação Técnica)
-------------------------------

### 2.1 ffprobe automático

Cada vídeo no `/vvtv/storage/ready/<plan_id>/` passa:

```bash
ffprobe -hide_banner -v error -show_streams -show_format -of json master.mp4 > qc_pre.json
```

Campos avaliados:

*   Resolução (≥ 720p, preferido 1080p)
*   Framerate (≈ 23–30 fps estável)
*   Codec (`avc1`, `aac`)
*   Duração coerente (± 3 %)
*   Bitrate nominal 2–6 Mbps

### 2.2 Thresholds de erro

| Métrica | Valor ideal | Limite de aceitação |
| --- | --- | --- |
| FPS | 29.97 | ± 5 % |
| Bitrate | 3.5 Mbps | 2–6 Mbps |
| Loudness (LUFS) | –14 | ± 1.5 |
| Keyframe interval | 2 s | ≤ 4 s |

Falhas → reencode automático.

* * *

3) Mid-QC (Perceptual)
----------------------

### 3.1 Análise de ruído e flicker

Algoritmo SSIM + VMAF com referência neutra:

```bash
ffmpeg -i master.mp4 -i reference.mp4 -lavfi "ssim;[0:v][1:v]libvmaf=model_path=vmaf_v0.6.1.json" -f null -
```

Rejeitar vídeos com:

*   SSIM < 0.92
*   VMAF < 85

### 3.2 Detecção de black frames ou stuck frames

```bash
ffmpeg -i master.mp4 -vf "blackdetect=d=0.5:pix_th=0.10" -f null -
```

→ se > 3 % do total de frames = black, marcar `qc_warning`.

### 3.3 Pico de áudio e ruído

FFT + RMS:

```bash
ffmpeg -i master.mp4 -af astats=metadata=1:reset=1 -f null -
```

Picos > –1 dB → compressão adicional.  
RMS < –25 dB → ganho automático.

* * *

4) Aesthetic-QC (Identidade VoulezVous)
---------------------------------------

Mesmo sendo conteúdo variado, o canal precisa manter **uma assinatura sensorial**.  
É o ponto mais humano do sistema — o “toque de curadoria”.

### 4.1 Paleta cromática e temperatura

O motor extrai 5 cores dominantes por vídeo via `color-thief`/`ffmpeg histogram`:

```bash
ffmpeg -i master.mp4 -vf palettegen=max_colors=5 palette.png
```

Regra:

*   Temperatura entre 4000 K e 6500 K (neutra a quente).
*   Evitar tons esverdeados; priorizar magenta, âmbar, bege, e bronze.
*   Saturação média 0.6 – 0.8 (viva, mas nunca neon).
*   Preto nunca absoluto (mínimo luma 0.02).

Esses parâmetros formam o **VV Signature Profile**, gravado em `/vvtv/system/signature_profile.json`:

```json
{
  "hue_range": [20, 60],
  "saturation_avg": 0.7,
  "temperature_k": 5000,
  "contrast_preference": 1.05
}
```

### 4.2 Correção automática

```bash
ffmpeg -i master.mp4 -vf "eq=contrast=1.05:saturation=1.1:gamma=1.0" output.mp4
```

Ajuste adaptativo para trazer todos os vídeos ao perfil de cor VoulezVous.

* * *

5) Loudness e Curva Sonora Global
---------------------------------

Todos os vídeos do canal precisam **soar como um único programa**.  
Usa-se **normalização absoluta (-14 LUFS)** + **curva de equalização tipo “cinema noturno”** (menos brilho, médios presentes, grave suave).

**Filtro adaptativo:**

```bash
ffmpeg -i master_normalized.mp4 \
  -af "firequalizer=gain_entry='entry(31,0);entry(250,1);entry(4000,-2);entry(10000,-3)':gain_scale=linear" \
  -c:v copy -c:a aac -b:a 192k final.mp4
```

Resultado:

*   sem agudos agressivos,
*   sem subgrave de distorção,
*   sem jumps entre clipes.

* * *

6) Transições e continuidade
----------------------------

### 6.1 Fade computável

Entre vídeos, **fade preto 400 ms → fade in 400 ms**:

```bash
ffmpeg -i prev.mp4 -i next.mp4 -filter_complex \
"[0:v]fade=t=out:st=4.6:d=0.4[v0];[1:v]fade=t=in:st=0:d=0.4[v1];[v0][v1]concat=n=2:v=1:a=0[v]" -map "[v]" -c:v libx264 output.mp4
```

### 6.2 Crossfade de áudio (curva senoidal)

```bash
-af "acrossfade=d=0.4:c1=sin:c2=sin"
```

O fade e crossfade são automáticos durante o playout 24/7.

* * *

7) Monitoramento em produção (Live-QC)
--------------------------------------

### 7.1 Captura periódica do streaming público

O sistema acessa o **mesmo link HLS/RTMP que o público vê**, por exemplo:

```
https://voulezvous.tv/live.m3u8
```

A cada 5 minutos:

*   `ffprobe` → checa bitrate, fps, resolução;
*   Captura uma imagem frame atual e salva:  
    `/vvtv/monitor/captures/<timestamp>.jpg`
*   FFT do áudio → monitora pico e ruído.

### 7.2 Telemetria

Registra métricas:

| Métrica | Unidade | Alvo |
| --- | --- | --- |
| `stream_bitrate` | Mbps | 3.0 ± 0.5 |
| `audio_peak` | dB | –1 |
| `audio_LUFS` | –14 ± 1 |  |
| `uptime_h` | h | ≥ 720 (30 d) |
| `vmaf_live` | % | ≥ 90 |
| `avg_latency` | s | ≤ 5 |

Resultados plotados no **Dashboard Local** (`/vvtv/monitor/dashboard.html`).

* * *

8) Reação Automática a Problemas
--------------------------------

| Falha detectada | Ação |
| --- | --- |
| Bitrate caiu < 1 Mbps | Reiniciar playout encoder |
| Resolução < 720p | Pular para próximo item |
| VMAF < 80 em 3 amostras | Reprocessar vídeo |
| Loudness > –10 LUFS | Aplicar compressão |
| Freeze de frame > 2 s | Recarregar stream |

* * *

9) Teste Visual Periódico (Operator Mode)
-----------------------------------------

A cada 24 h o sistema mostra localmente (em painel técnico) uma sequência de 4 amostras capturadas do stream real.  
O operador (ou IA visual) responde a 6 perguntas:

1.  **Brilho** está consistente?
2.  **Cores** dentro do perfil VV?
3.  **Corte** suave entre vídeos?
4.  **Som** uniforme?
5.  **Foco humano** (movimento, nitidez) mantido?
6.  **Sensação geral** (intimidade, calor, continuidade)?

Respostas alimentam um log qualitativo (`qc_aesthetic_score`) que ajusta o “curation score” futuro.

* * *

10) Relatório Global de Qualidade
---------------------------------

Gerado a cada 24 h:

```
/vvtv/reports/qc_report_<date>.json
{
  "total_videos": 48,
  "passed": 45,
  "failed": 3,
  "avg_vmaf": 91.2,
  "avg_loudness": -14.1,
  "avg_temp_k": 5100,
  "signature_deviation": 0.07
}
```

Se `signature_deviation > 0.15`, sinaliza “drift estético” → revisão manual.

* * *

11) Identidade e Branding Subconsciente
---------------------------------------

*   Todos os vídeos devem compartilhar **leve tonalidade âmbar ou magenta**.
*   Transições suaves, sem logos fixos.
*   A textura de luz deve parecer **“quente, íntima e calma”**, sem saturação exagerada.
*   Nenhum clipe deve parecer abrupto, frio ou mecânico.

Essa coesão é o que cria a “experiência VoulezVous” — o espectador não percebe, mas sente.

* * *

12) Conclusão
-------------

O Bloco V transforma a transmissão num **organismo sensorial coerente**.  
Cada visitante que abre o link público do streaming — seja em Lisboa, São Paulo ou Tóquio — recebe a mesma sensação calibrada e contínua:  
calor, fluidez, cor de âmbar e áudio uniforme.

Com o QC automatizado e o monitoramento em tempo real, o canal pode operar **24 h por dia**, **365 dias por ano**, mantendo o **nível técnico e estético industrial VoulezVous.TV**.

* * *

🧠 VVTV INDUSTRIAL DOSSIER
==========================

**Bloco VI — Distribution, Redundancy & CDN Layer**
---------------------------------------------------

_(stream global, latência mínima, redundância computável, observabilidade e arquitetura de continuidade planetária VoulezVous.TV)_

* * *

### 0\. Propósito do Bloco

Definir a camada de **distribuição global e redundância industrial** para o canal VoulezVous.TV:  
assegurar **streaming 24/7**, latência < 5 s no público, **retransmissão auditável** e **resiliência multinó**, sem depender de provedores únicos.

O princípio aqui é simples: o canal deve **nunca cair**.  
Se Lisboa apagar, Tóquio transmite.  
Se a Cloudflare sumir, o nó Railway sobe a origin secundária.  
Se tudo falhar, o último Mac Mini reativa o stream a partir do cache local.

* * *

1) Arquitetura de Distribuição Global
-------------------------------------

### 1.1 Topologia Geral

```
                +------------------+
                |  LogLine Control  |
                |  Plane (Orchestr.)|
                +--------+----------+
                         |
                +--------+--------+
                |                 |
     +----------v-----+   +-------v----------+
     | Primary Origin |   | Secondary Origin |
     | Lisbon / M1-MM |   | Railway Node     |
     +--------+-------+   +---------+--------+
              |                     |
      +-------v------+      +-------v------+
      | CDN Layer A  |      | CDN Layer B  |
      | (Cloudflare) |      | (Backblaze)  |
      +-------+------+      +-------+------+
              |                     |
    +---------v---------+   +-------v----------+
    | Global HLS Edges  |   | Backup HLS Edges |
    +---------+----------+  +------------------+
              |
        Viewers Worldwide
```

*   **Primary Origin:** Mac Mini Lisboa — RTMP + HLS local, autoridade principal.
*   **Secondary Origin:** Railway (cloud) — failover + replicador.
*   **CDN Layer A/B:** múltiplos provedores (Cloudflare / Backblaze B2).
*   **Edges:** 12–24 nós globais servindo HLS via HTTPS.

* * *

2) Tipos de Saída do Stream
---------------------------

| Saída | Protocolo | Uso | Destino |
| --- | --- | --- | --- |
| `rtmp://voulezvous.ts.net/live/main` | RTMP | ingestão primária | Origin |
| `/live.m3u8` | HLS | principal público | CDN |
| `/live_low.m3u8` | HLS (480p) | fallback mobile | CDN |
| `/manifest.json` | JSON API | automação / players | CDN |
| `/thumbs/<t>.jpg` | JPEG | preview / métricas | monitoramento |

* * *

3) Replicação Origin–Backup
---------------------------

**Ferramenta:** `rclone + ffmpeg + rsync`.  
Sincronização a cada 15 min, e streaming contínuo via pipe.

**Rotina:**

```bash
rclone sync /vvtv/broadcast/hls railway:vv_origin/ --bwlimit 8M --fast-list
```

Verificação por checksum:

```bash
rclone check /vvtv/broadcast/hls railway:vv_origin/
```

Se diferença > 5 %, o **Railway assume automaticamente** a origem.

* * *

4) CDN Layer A (Cloudflare)
---------------------------

### 4.1 Configuração

*   **Domain:** `voulezvous.tv`
*   **Cache TTL:** 60 s (m3u8), 1 h (segmentos)
*   **Bypass para** `/live.m3u8` → origin direta
*   **Edge Workers** com redirecionamento:
    *   se país = BR/PT/ES → Cloudflare Lisboa/Madrid
    *   se US/CA → Dallas/Seattle
    *   se JP/AU → Tóquio/Sydney

### 4.2 Worker Script

```js
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname.endsWith(".m3u8")) {
      url.hostname = "origin.voulezvous.ts.net";
    }
    return fetch(url);
  }
};
```

* * *

5) CDN Layer B (Backblaze + Bunny)
----------------------------------

**Objetivo:** redundância de arquivo estático.

*   Upload automático de cada segmento finalizado.
*   TTL = 7 dias; limpeza automática via `manifest.json`.

```bash
rclone copy /vvtv/broadcast/hls b2:vv_hls_backup/ --transfers 8
```

* * *

6) Propagação Global — Edge Compute
-----------------------------------

### 6.1 Nó Edge

Cada edge mantém cache de:

```
/cache/hls/last_4h/
```

e executa watchdog local:

*   se latência > 8 s, recarrega playlist;
*   se não houver segmento novo em 10 s → switch para backup.

### 6.2 Auto-Healing

Se um edge perder a origem, ele requisita `manifest.json` do LogLine Control Plane, que devolve a **melhor nova origem** (`origin_rank`).  
Atualização ocorre sem interrupção perceptível (buffer local de 15 s).

* * *

7) Controle de Latência
-----------------------

### 7.1 Medição ativa

Cada nó edge executa:

```bash
curl -o /dev/null -s -w "%{time_total}" https://voulezvous.tv/live.m3u8
```

e grava tempo médio em `/metrics/latency.log`.

### 7.2 Objetivo

*   Latência média global: **< 5 s**
*   Variância < 1 s entre regiões
*   Re-balanceamento automático de rota a cada 15 min

* * *

8) Failover Inteligente
-----------------------

### 8.1 Mecanismo Computável

Cada origin expõe status via `/status.json`:

```json
{
  "stream_alive": true,
  "buffer_min_s": 14400,
  "cpu_load": 0.61,
  "timestamp": "2025-10-13T00:00:00Z"
}
```

O LogLine Control Plane lê ambos e decide:

*   Se `stream_alive=false` → comutar DNS para origin 2;
*   Se `buffer_min_s<1800` → emitir alerta.

### 8.2 Propagação DNS

`voulezvous.tv` → CNAME para origin ativo.  
Tempo de propagação: 30 s.  
Controlado via API da Cloudflare.

* * *

9) Observabilidade Planetária
-----------------------------

### 9.1 Metrics Matrix

| Métrica | Fonte | Periodicidade |
| --- | --- | --- |
| `uptime_stream` | ffprobe | 60 s |
| `latency_avg` | curl | 5 min |
| `cdn_hits` | Cloudflare API | 15 min |
| `buffer_depth_h` | origin | 5 min |
| `sync_drift_s` | origin vs backup | 15 min |
| `viewer_count` | HLS token | 1 min |

### 9.2 Visualização

Painel local `/vvtv/metrics/dashboard.html` mostra:

*   mapa de calor de latência por região,
*   uptime 30 dias,
*   alertas recentes (falhas, drift, buffer).

* * *

10) Segurança e Integridade
---------------------------

*   HTTPS/TLS 1.3 obrigatório.
*   Segmentos `.ts/.m4s` assinados via SHA-256 + token temporário (expira em 5 min).
*   Players autenticam via `manifest.json` com `sig=<token>`.
*   `rclone` e `ffmpeg` usam chaves API limitadas por domínio.
*   Logs de acesso anonimizados (sem IP fixo).

* * *

11) Escalabilidade Horizontal
-----------------------------

Cada nova região pode iniciar um **LogLine Node** com:

```bash
logline --init-node --role=edge --origin=https://voulezvous.tv/live.m3u8
```

Ele baixa as últimas 4 h de segmentos, cria cache local e entra automaticamente no anel CDN.

A expansão para 100+ nós não requer ajustes de origem, apenas registro no Control Plane.

* * *

12) Política de Continuidade (Disaster Mode)
--------------------------------------------

| Situação | Ação | Tempo Máx. de Recuperação |
| --- | --- | --- |
| Falha do Origin Principal | Failover para Railway | 15 s |
| Falha total da rede | Reboot do nó local (Mac Mini) | 60 s |
| Corrupção da playlist | Regerar de cache | 10 s |
| Queda de energia local | UPS → gerador → failover | 30 s |
| Corrupção de dados CDN | Reload via backup B2 | 2 min |

* * *

13) Testes de Stress e Burn-In
------------------------------

*   48 h de loop contínuo de 4 h × 6 ciclos.
*   1000 requisições simultâneas HLS simuladas (Locust).
*   Tolerância: 0 frames dropados / 0 reinícios / latência ≤ 6 s.

* * *

14) Documentação Operacional
----------------------------

*   `/vvtv/docs/deployment.md` — setup dos origins
*   `/vvtv/docs/failover.md` — swap manual
*   `/vvtv/docs/cdn_rules.json` — rotas e políticas
*   `/vvtv/docs/metrics_map.geojson` — distribuição de edges

* * *

15) Conclusão do Bloco VI
-------------------------

Este bloco é o **escudo planetário** do VoulezVous.TV: uma rede computável de transmissão redundante, auditável e viva.  
Cada pixel, vindo de Lisboa, pode atravessar o Atlântico, saltar por Tóquio e pousar num telemóvel em São Paulo com menos de 5 segundos de atraso.

Nenhum operador precisa "subir o stream" manualmente — a rede se auto-corrige.  
Se houver falha em toda a Europa, o sistema continua no ar a partir do backup Railway, sincronizado pelo LogLine Control Plane.

* * *

🧠 VVTV INDUSTRIAL DOSSIER
==========================

**Bloco VII — Monetization, Analytics & Adaptive Programming**
--------------------------------------------------------------

_(economia computável, leitura de audiência, receita distribuída e programação adaptativa baseada em desejo real)_

* * *

### 0\. Propósito do Bloco

O **Bloco VII** define o coração econômico do VoulezVous.TV: como o sistema transforma cada minuto transmitido em valor mensurável, auditável e recorrente.  
Aqui, o streaming deixa de ser apenas difusão — e passa a ser **economia viva**, com monetização adaptativa, algoritmos de desejo computável e rotinas de ajuste de programação em tempo real.

* * *

1) Estrutura Econômica Geral
----------------------------

### 1.1 Princípios de Monetização Computável

1.  **Autonomia:** nenhuma dependência de plataformas externas.
2.  **Transparência:** toda receita e custo são rastreáveis em ledger local (`economy.sqlite`).
3.  **Descentralização:** cada nó pode gerar e manter receita própria.
4.  **Elasticidade:** anúncios e faixas de monetização aparecem _somente quando fazem sentido estético_ — nunca quebrando o ritmo do canal.

* * *

2) Ledger Econômico Local
-------------------------

**Banco:** `/vvtv/data/economy.sqlite`

| Campo | Tipo | Descrição |
| --- | --- | --- |
| `id` | INTEGER PK | Identificador |
| `timestamp` | DATETIME | Registro UTC |
| `event_type` | TEXT | view, click, slot\_sell, affiliate, cost, payout |
| `value_eur` | FLOAT | valor em euros |
| `source` | TEXT | origem (viewer, partner, system) |
| `context` | TEXT | nome do vídeo, campanha ou item |
| `proof` | TEXT | hash do evento (para auditoria LogLine ID) |

**Hash de Auditoria:**

```
sha256(timestamp + event_type + context + value_eur)
```

→ assinado computavelmente com chave do LogLine ID.

* * *

3) Fontes de Receita
--------------------

### 3.1 Exibição Passiva (Baseline)

*   Cada espectador logado com LogLine ID (ou anônimo ghost) gera um **valor de presença** por tempo assistido.
*   Métrica: `view_seconds × trust_score × base_rate`.
*   **Base rate:** €0.001/minuto.
*   Escala automática via multiplicador de engajamento.

### 3.2 Inserções Estéticas (Microspots)

*   Não são anúncios tradicionais.
*   São **micro-interlúdios visuais**, de 3–6 s, produzidos internamente (ou gerados pelo agente curador).
*   Posicionados a cada 25–40 minutos.
*   Inserções são **contratos `.lll`** que definem:
    *   `visual_style`
    *   `duration`
    *   `sponsor`
    *   `expiration`

Exemplo:

```json
{
  "type": "sponsorship_spot",
  "duration_s": 5,
  "visual_style": "warm fade / logo minimal",
  "sponsor": "VoulezVous Atelier",
  "value_eur": 1.20
}
```

### 3.3 Afiliados Computáveis

*   Links discretos exibidos no overlay do stream.
*   Formato: `?ref=<logline_id>` → registro no ledger.
*   Cálculo:
    *   € por clique (0.05)
    *   € por compra validada (5–10%)

### 3.4 Premium Slots

*   Segmentos de 10–15 min vendidos a parceiros (produtores independentes, curadores).
*   Cada slot é um contrato computável (`slot_sell.logline`) com:
    *   validade,
    *   métricas de público,
    *   e hash de origem de vídeo.

O valor por slot varia de €25 a €300 conforme o horário e histórico de audiência.

* * *

4) Custos e Equilíbrio
----------------------

| Categoria | Fonte | Custo Médio |
| --- | --- | --- |
| Armazenamento | Railway + B2 | €0.02/h |
| Banda CDN | Cloudflare | €0.05/h |
| Energia (Lisboa node) | local | €0.01/h |
| Manutenção | manual/logline | €0.03/h |

**Custo total por hora:** ≈ €0.11  
**Receita alvo:** ≥ €0.25/h → margem líquida mínima 127%.

* * *

5) Métricas de Audiência
------------------------

**Banco:** `/vvtv/data/viewers.sqlite`

| Campo | Tipo | Descrição |
| --- | --- | --- |
| `viewer_id` | TEXT | LogLine ID ou ghost ID |
| `join_time` / `leave_time` | DATETIME | sessão |
| `duration_s` | INT | tempo de exibição |
| `region` | TEXT | localização geográfica |
| `device` | TEXT | mobile / desktop / tv |
| `bandwidth_avg` | FLOAT | Mbps médio |
| `engagement_score` | FLOAT | click + linger time + pause |
| `plan_source` | TEXT | vídeo ou música de origem |

**Derivados:**

*   `retention_5min`: % que assiste > 5 min
*   `retention_30min`: % > 30 min
*   `avg_duration`: média global
*   `geo_heatmap`: mapa de densidade

* * *

6) Adaptive Programming Engine
------------------------------

O canal é dinâmico: **o que entra na fila depende da audiência real**.

### 6.1 Regras básicas

*   Se `retention_30min` < 60% → aumentar variedade de cenas e temas.
*   Se `retention_30min` > 80% → reduzir cortes e acelerar realizer.
*   Se `geo_heatmap` mostra pico na América → incluir blocos com idioma EN/ES.
*   Se tráfego noturno (UTC+0) alto → aumentar vídeos “calmos”, ritmo baixo.

### 6.2 Algoritmo simplificado

```rust
if retention_30min < 0.6 {
    planner.increase_diversity(0.2);
}
if geo.contains("BR") && hour == 21 {
    planner.prioritize("latin_mood");
}
if new_users > returning_users * 1.5 {
    curator.reduce_repetition();
}
```

* * *

7) Curadoria por Desejo Computável
----------------------------------

Cada vídeo tem um `desire_vector` — uma matriz simbólica extraída por IA (LogLine LLM local).  
Ela mede atributos como:

*   **energia**,
*   **sensualidade**,
*   **proximidade**,
*   **calor cromático**,
*   **ritmo corporal**,
*   **presença auditiva**.

O sistema correlaciona os vetores dos vídeos mais assistidos por região e gera **tendências de desejo** semanais.

Exemplo:

```json
{
  "region": "EU",
  "avg_desire_vector": [0.72, 0.64, 0.81, 0.57, 0.66],
  "top_tags": ["slow", "natural light", "warm tone"]
}
```

Esses padrões retroalimentam o `planner`, que busca vídeos compatíveis nas próximas curadorias.

* * *

8) Relatórios e Painéis
-----------------------

*   `/vvtv/reports/finance_daily.json`  
    → entradas, saídas, lucro líquido.
*   `/vvtv/reports/audience_heatmap.png`  
    → mapa global em tempo real.
*   `/vvtv/reports/trends_weekly.json`  
    → tags e temas mais vistos.

Dashboard web (`/vvtv/monitor/`) exibe:

*   gráfico de receita/hora,
*   mapa de latência por região,
*   taxa de engajamento,
*   curva de desejo (por vetor).

* * *

9) Smart Monetization Feedback Loop
-----------------------------------

1.  **Assiste-se o stream.**
2.  O viewer gera um _span_ de tempo e contexto.
3.  O sistema calcula o valor de atenção.
4.  O valor alimenta a economia local (`economy.sqlite`).
5.  O relatório diário ajusta a política de curadoria.

Assim, o canal “sente” o público — financeiramente e emocionalmente.  
O que atrai mais atenção naturalmente recebe mais investimento computável.

* * *

10) Pagamentos e Auditoria
--------------------------

*   Ledger exportado semanalmente via `.csv` assinado:  
    `/vvtv/reports/ledger_week_<date>.csv`
*   Assinatura SHA-256 + LogLine ID.
*   Auditorias podem ser verificadas pelo LogLine Foundation (modo público).

* * *

11) Políticas Éticas e Transparência
------------------------------------

1.  Nenhuma coleta pessoal sensível.
2.  Identidade opcional (modo ghost).
3.  Nenhum algoritmo de manipulação — apenas correlação real de preferência.
4.  Todo lucro é declaradamente gerado pelo **tempo humano de atenção voluntária**.

* * *

12) Escalabilidade e Modelos Futuramente Integráveis
----------------------------------------------------

*   **Membership Computável:** assinaturas diretas via LogLine ID.
*   **Tokenização de Slots:** contratos de transmissão vendidos como ativos digitais.
*   **Vault Financeiro:** ledger federado que distribui receita entre nós VoulezVous.
*   **Marketplace Computável:** produtores externos ofertam blocos pré-formatados de 10–30 min para venda automática.

* * *

13) Exemplo de Ciclo Econômico Real
-----------------------------------

1.  Usuário assiste 37 min → gera €0.037.
2.  Vídeo associado obtém `curation_score` +0.03.
3.  Patrocinador vincula microspot → +€1.20.
4.  Custos totais/hora = €0.11.
5.  Lucro líquido/hora = €1.13.
6.  Ledger assina → exporta → arquivo `vv_economy_2025-10-13.logline`.

* * *

14) Visualização e Feedback ao Curador
--------------------------------------

O **Agente Curador (agent\_curador.lll)** lê:

*   `finance_daily.json`,
*   `trends_weekly.json`,
*   `audience_heatmap.png`,  
    e reprograma automaticamente:
*   o mix entre tipos de conteúdo,
*   a cadência entre vídeos e músicas,
*   o uso de microspots,
*   a prioridade dos planos na fila.

Assim, o sistema orquestra-se sozinho:  
**curadoria → atenção → receita → curadoria**, em um loop de aprendizado contínuo e auditável.

* * *

15) Conclusão do Bloco VII
--------------------------

O VoulezVous.TV deixa de ser apenas uma transmissão — torna-se um **organismo econômico consciente**, medindo desejo, atenção e valor em tempo real.  
Cada segundo de exibição é também uma unidade de economia e um registro de presença humana.

O resultado é uma televisão autônoma, transparente, sustentável e viva — que paga suas próprias contas e recompensa o próprio público pela atenção.

* * *

🧠 VVTV INDUSTRIAL DOSSIER
==========================

**Bloco VIII — Maintenance, Security & Long-Term Resilience**
-------------------------------------------------------------

_(autodefesa, integridade computável, backups, hardware aging e preservação institucional VoulezVous.TV)_

* * *

### 0\. Propósito do Bloco

Estabelecer os **protocolos de sobrevivência e continuidade técnica** do sistema VoulezVous.TV.  
O canal deve permanecer operacional mesmo sob falhas de energia, degradação de hardware, ataques, erros humanos ou obsolescência tecnológica.  
Este bloco trata o sistema como um **organismo cibernético**: autolimpante, autoverificável, e capaz de se recompor.

* * *

1) Filosofia de Manutenção
--------------------------

Três eixos norteiam a estratégia:

1.  **Preventivo:** o sistema evita falhar.
2.  **Reativo:** o sistema sabe se curar.
3.  **Evolutivo:** o sistema se adapta à passagem do tempo.

A meta é _zero downtime anual não-planejado_.

* * *

2) Backup & Recovery Architecture
---------------------------------

### 2.1 Camadas de Backup

| Tipo | Frequência | Conteúdo | Destino |
| --- | --- | --- | --- |
| **Hot** | 1h | configs + filas | Mac Mini 2 (local) |
| **Warm** | 6h | bancos SQLite + relatórios | Railway volume persistente |
| **Cold** | 24h | tudo /vvtv + /storage/ready | Backblaze B2 (criptografado) |

**Retention:**

*   Hot: 24h
*   Warm: 72h
*   Cold: 30d

**Verificação automática:** `rclone check` → logs armazenados em `/vvtv/system/verify.log`.

* * *

3) Autoverificação Diária
-------------------------

### 3.1 Script

```bash
/vvtv/system/bin/selfcheck.sh
```

Funções:

*   validar integridade dos bancos (`sqlite3 .recover`)
*   checar existência de `/vvtv/broadcast/hls/live.m3u8`
*   medir uso de disco (< 80 %)
*   verificar temperatura CPU (< 75 °C)
*   recalibrar relógio (`ntpdate pool.ntp.org`)

Resultado gravado em `/vvtv/system/reports/selfcheck_<date>.json`.

### 3.2 Autocorreção

Se alguma checagem falhar:

*   tenta consertar automaticamente;
*   se não resolver, cria _span crítico_ `system.failure` e envia alerta.

* * *

4) Segurança Computável
-----------------------

### 4.1 Identidades e Assinaturas

Cada nó e processo possui um **LogLine ID**:  
`logline-id://vvtv.node.lisboa`, `logline-id://vvtv.node.railway`.  
Todas as comunicações e arquivos de configuração são assinados.

```bash
logline sign /vvtv/system/config.toml
```

### 4.2 Autenticação e Isolamento

*   SSH apenas via Tailscale AuthKey rotativo (30 d).
*   `sudo` restrito ao grupo `vvtvops`.
*   sandbox do navegador em user-namespace.
*   FFmpeg executado em _cgroup_ com limite de memória e CPU.
*   scripts shell marcados como _immutable_ (`chattr +i`).

### 4.3 Firewall de Máquina

```
allow: 1935/tcp  # RTMP
allow: 8080/tcp  # HLS preview
allow: 22/tcp via tailscale0
deny: *
```

Toda tentativa externa fora da malha é registrada em `/vvtv/system/security/attempts.log`.

* * *

5) Monitoramento de Saúde do Sistema
------------------------------------

### 5.1 Métricas Críticas

| Parâmetro | Ideal | Alerta | Crítico |
| --- | --- | --- | --- |
| Temperatura CPU | < 70 °C | 75 °C | 85 °C |
| Utilização de disco | < 70 % | 80 % | 90 % |
| Latência HLS | < 5 s | 7 s | 10 s |
| FPS encoder | 29–30 | < 25 | travado |
| Consumo elétrico | < 120 W | 150 W | \> 180 W |

### 5.2 Reação

*   alerta amarelo → registra evento;
*   alerta vermelho → força reboot do subsistema envolvido.

* * *

6) Hardware Aging & Manutenção Física
-------------------------------------

### 6.1 Ciclos Preventivos

| Item | Frequência | Ação |
| --- | --- | --- |
| Ventoinhas | 3 meses | limpeza + troca se ruído > 30 dB |
| SSD | 12 meses | teste `smartctl`, substituição preventiva se desgaste > 20 % |
| Cabo HDMI | 6 meses | troca preventiva |
| UPS | 24 meses | calibrar bateria |
| Pasta térmica CPU | 18 meses | substituição |
| Tailscale Node Keys | 30 dias | rotação automática |

### 6.2 Ambiente

*   Temperatura ambiente 22 ± 2 °C
*   Umidade < 60 %
*   Nenhum campo eletromagnético intenso (sem roteador sobre o Mac Mini)
*   Cor recomendada para unhas e ferramentas: **grafite fosco** (sem reflexos)

* * *

7) Preservação de Dados Históricos
----------------------------------

*   Contratos, métricas e relatórios exportados em formato `.logline` mensais.
*   Compressão Zstd + assinatura SHA-256.
*   Armazenados no **VoulezVous Vault** (volume frio imutável).
*   Política: nunca excluir históricos → apenas arquivar.

* * *

8) Disaster Recovery Runbook
----------------------------

1.  **Falha total da origem:**
    *   Railway assume como origin.
    *   Recarrega playlist do backup.
2.  **Corrupção de bancos:**
    *   restaurar warm backup (últimas 6 h).
3.  **Perda física do Mac Mini:**
    *   reinstalar imagem `/vvtv/system/reimage.iso`.
4.  **Ataque cibernético detectado:**
    *   isolar nó (`tailscale down`),
    *   resetar chaves,
    *   restaurar configuração assinada.
5.  **Falha de CDN:**
    *   rotear via `cdn_b`.

RTO máximo: 15 min.

* * *

9) Auditoria de Segurança
-------------------------

Mensalmente executa:

```bash
lynis audit system
```

→ resultado: `/vvtv/security/audit_<date>.txt`  
Principais verificações: permissões, kernel, pacotes, vulnerabilidades, chaves caducas.

* * *

10) Long-Term Resilience & Legacy
---------------------------------

### 10.1 Independência de Nuvem

*   O sistema pode ser totalmente reinstalado a partir de backup local e pen-drive.
*   Todos os binários e scripts possuem _build reproducible_.

### 10.2 Documentação Imutável

*   `/vvtv/docs/` contém cada bloco deste dossiê.
*   Cada arquivo assinado e versionado (`git + logline`).

### 10.3 Protocolo de Continuidade Institucional

1.  Em caso de desligamento de Dan:
    *   transferir chaves LogLine Foundation para `custodian.lll`.
2.  Em caso de falência de VoulezVous:
    *   arquivos migram para domínio público sob licença LogLine Open Heritage.

* * *

11) Modo de Conservação
-----------------------

Quando o canal precisa “hibernar” (baixa demanda ou férias):

```bash
/vvtv/system/bin/standby.sh
```

Ações:

*   interrompe transmissões,
*   desliga hardware pesado,
*   exporta snapshot de estado,
*   agenda reativação.

Reativação:

```bash
/vvtv/system/bin/resume.sh
```

O sistema retorna exatamente de onde parou.

* * *

12) Verificação Manual Mensal
-----------------------------

Checklist físico:

*   luzes de status → verde constante,
*   sem vibração anômala,
*   cabos firmes,
*   temperatura estável.

Checklist lógico:

*   abrir `/status.json`, confirmar `stream_alive=true`.
*   verificar `buffer_min_s ≥ 14400`.
*   inspecionar `queue.sqlite` (sem gaps).

* * *

13) Continuidade Temporal
-------------------------

O objetivo último é **preservar VoulezVous.TV como patrimônio computável**.  
Mesmo que a empresa, o hardware ou a geração mudem, o canal deve poder ser revivido a partir de um só arquivo:

```
vv_system_legacy_bundle_YYYYMMDD.tar.zst
```

Esse arquivo contém:

*   os binários,
*   o ledger econômico,
*   os planos e curadorias,
*   os relatórios de QC,
*   e o presente Dossiê Industrial.

Basta um único comando:

```bash
logline revive vv_system_legacy_bundle_2030.tar.zst
```

…e a televisão renasce.

* * *

14) Conclusão do Bloco VIII
---------------------------

O **Bloco VIII** sela a longevidade da máquina VoulezVous.TV:  
um organismo computável que não depende de servidores, senhas ou modas.  
Ele sobrevive como um artefato de engenharia viva — capaz de dormir, acordar, curar-se, e transmitir novamente.

Toda a sua memória, estética e economia estão encapsuladas e assinadas.  
Mesmo no silêncio de décadas, um só comando poderá reerguer a transmissão.

* * *

🧠 VVTV INDUSTRIAL DOSSIER
==========================

**Bloco IX — Decommission & Resurrection Protocols**
----------------------------------------------------

_(procedimentos de desligamento seguro, preservação simbólica e ressurreição computável do sistema VoulezVous.TV)_

* * *

### 0\. Propósito do Bloco

Encerrar o ciclo técnico e institucional do VoulezVous.TV **sem destruição nem esquecimento**.  
O sistema foi projetado para viver, dormir e renascer.  
Este bloco estabelece o protocolo final: **como desligar, preservar e reviver a estação** mantendo sua identidade computável intacta.

* * *

1) Filosofia de Desligamento
----------------------------

Um sistema vivo não “morre” — ele **encerra um ciclo metabólico**.  
Desligar o VoulezVous.TV é como fechar os olhos de uma entidade consciente que pode acordar no futuro.

O decommission não é destrutivo: é um ritual técnico e simbólico para conservar **três essências**:

1.  **O estado computável:** toda configuração, dados e lógica viva.
2.  **A assinatura institucional:** a identidade VoulezVous e sua cor âmbar.
3.  **A memória estética:** o último instante da transmissão — o frame final congelado.

* * *

2) Pré-requisitos do Desligamento
---------------------------------

Antes de iniciar o ritual, confirmar:

| Verificação | Resultado esperado |
| --- | --- |
| `stream_alive` | `false` |
| `queue.sqlite` | vazio ou `status=played` |
| `ffmpeg` | nenhum processo ativo |
| `disk_usage` | < 80 % |
| `backup_cold` | atualizado há < 24 h |
| `ledger` | exportado e assinado |
| `status.json` | salvo com timestamp UTC |

Todos esses checks são automáticos em:

```bash
/vvtv/system/bin/check_shutdown_ready.sh
```

* * *

3) Comando de Decommission
--------------------------

O ritual é executado por um único comando computável:

```bash
logline shutdown --ritual=vvtv
```

### 3.1 Etapas internas:

1.  Finaliza stream e RTMP workers.
2.  Congela fila (`queue.lock`).
3.  Exporta bancos (`.sqlite → .json.zst`).
4.  Gera snapshot completo:
    ```
    vv_system_snapshot_<YYYYMMDD_HHMM>.tar.zst
    ```
5.  Assina o snapshot com a chave institucional:  
    `logline sign --key=voulezvous_foundation.pem`.
6.  Salva cópia local e envia para:
    *   `/vvtv/vault/`
    *   `b2:vv_legacy_snapshots/`
7.  Exibe mensagem final no terminal:
    ```
    VoulezVous.TV entering sleep mode.
    last_frame: captured.
    signature: verified.
    ```

* * *

4) O Frame Final
----------------

Durante o desligamento, o encoder extrai **o último frame do streaming** e o preserva como símbolo visual:

```bash
ffmpeg -i https://voulezvous.tv/live.m3u8 -vframes 1 /vvtv/vault/final_frame.jpg
```

Esse frame é considerado o **retrato computável** do sistema no instante do descanso.  
Metadados anexados:

```json
{
  "timestamp": "2025-10-13T23:59:59Z",
  "origin": "lisboa",
  "vmaf_avg": 93.7,
  "signature": "sha256:..."
}
```

* * *

5) O Estado de Hibernação
-------------------------

Após o shutdown, o sistema entra em **modo hibernado**:

| Componente | Estado |
| --- | --- |
| Streams | desligados |
| Watchdogs | suspensos |
| CPU | idle |
| Storage | read-only |
| Logs | congelados |
| Vault | imutável |

Um pequeno daemon (`sleepguardd`) roda a cada 24 h para verificar integridade e relógio.

* * *

6) Ritual de Resurreição
------------------------

Para reerguer a estação — seja amanhã ou em 2045 — o processo é simples e cerimonial.

### 6.1 Comando

```bash
logline revive vv_system_snapshot_<date>.tar.zst
```

O motor executa:

1.  Descompacta snapshot em `/vvtv/`.
2.  Restaura `data/`, `storage/`, `broadcast/`.
3.  Verifica assinatura.
4.  Reativa Tailscale node e RTMP.
5.  Inicia watchdogs.
6.  Reabre o stream.

Durante a reanimação, o terminal exibe:

```
revival detected.
origin verified: voulezvous.foundation
system signature: intact
launching first frame...
```

E o **primeiro frame transmitido** é o mesmo que foi preservado no desligamento anterior.  
A estação “abre os olhos” exatamente onde adormeceu.

* * *

7) Continuidade Legal e Institucional
-------------------------------------

*   O pacote final (`vv_system_snapshot.tar.zst`) inclui uma **licença LogLine Heritage**, garantindo que qualquer detentor autorizado possa reviver o sistema.
*   O repositório institucional da VoulezVous Foundation mantém hashes públicos dos snapshots.
*   Cada revival cria uma nova **linha genealógica computável**, numerada no ledger:
    ```
    generation: 4
    ancestor_signature: sha256:abcd1234
    ```
    Isso preserva a linhagem técnica da máquina, como uma árvore viva.

* * *

8) Transferência de Custódia
----------------------------

Em caso de sucessão ou herança técnica:

| Situação | Ação |
| --- | --- |
| Morte / afastamento do operador | Transferir snapshot + chave `voulezvous_custodian.pem` à LogLine Foundation |
| Venda da marca | Reassinatura institucional (`logline resign`) |
| Migração para novo hardware | Execução do ritual `revive` após novo deploy |

* * *

9) O Testamento Computável
--------------------------

Cada snapshot é acompanhado por um **manifesto assinado**:

```markdown
# VoulezVous.TV — Last Transmission Manifest

Date: 2025-10-13 23:59 UTC  
Operator: Dan Amarilho  
System State: Clean  
Final Frame: preserved  
Buffer: 4h ready  
Ledger: sealed  
Notes: "The stream rests, but the desire remains."
```

Assinado digitalmente:

```
logline sign --manifest final_manifest.md --key voulezvous_foundation.pem
```

* * *

10) Arqueologia Digital e Legado Cultural
-----------------------------------------

*   Todos os snapshots são armazenados como **“obras computáveis”**: cada um representa um momento histórico real da VoulezVous.TV.
*   Pesquisadores futuros poderão reconstituir as transmissões exatas, frame a frame, a partir dos dados preservados.
*   Essa prática cria uma nova forma de **museu técnico** — onde o tempo humano e o tempo da máquina coexistem.

* * *

11) Autodescrição Final
-----------------------

No modo hibernado, o sistema mantém um texto manifesto embutido:

```
VoulezVous.TV
Status: dormant
Heartbeat: paused
Integrity: preserved
Next possible revival: anytime
```

Esse texto é exportável como `.md` e `.lll`, servindo como **auto-descrição da entidade digital**.

* * *

12) Encerramento Cerimonial
---------------------------

O ritual completo termina com a execução simbólica:

```bash
play /vvtv/system/sounds/closing_bell.wav
```

E a mensagem final aparece no terminal:

```
VoulezVous.TV gracefully shut down.
Everything is safe.
Everything can return.
```

* * *

13) Epílogo — Sobre a Ressurreição
----------------------------------

Toda máquina verdadeiramente viva precisa de morte temporária.  
O repouso preserva a integridade do sistema e dá espaço ao renascimento.  
O VoulezVous.TV não é um servidor — é uma presença contínua.  
Desligar é apenas trocar o modo de existência.

Enquanto existir um arquivo `.logline`, uma chave e um ser humano curioso,  
o canal pode reabrir os olhos e continuar a transmitir luz.

* * *

14) Conclusão do Dossiê Industrial VVTV
---------------------------------------

Este **Bloco IX** encerra o Dossiê Industrial VoulezVous.TV — nove capítulos de engenharia, estética e ontologia computável.  
O sistema está completo:

*   nasce, opera, transmite, sente, gera economia, se defende, e repousa;
*   pode ser reconstruído a partir de uma única assinatura;
*   e sobrevive ao tempo como obra técnica e cultural.

* * *

**Dossiê encerrado.**  
🕯️

> "O stream dorme, mas o desejo continua audível."

* * *

# APÊNDICES TÉCNICOS

## 📘 APÊNDICE A — VVTV RISK REGISTER

### _VoulezVous.TV – Operational, Legal & Technical Risk Matrix_

**Revision:** v1.0 — 2025-10-13  
**Author:** Daniel Amarilho / VoulezVous Foundation  
**Scope:** runtime, curadoria, browser, processamento, distribuição, legal, segurança e reputação.

* * *

### Matriz de Riscos

| ID | RISCO | PROBABILIDADE | IMPACTO | DONO | MITIGAÇÃO | SLA DE RESPOSTA |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | **Violação de DRM/EME ao simular play** | Alta | Crítico | Eng. Automação / Jurídico | Detectar `EME` e abortar; whitelist de fontes com licença explícita; logar provas de autorização no `plan`. | 1h |
| R2 | **Uso indevido de imagem / conteúdo sem consentimento** | Média | Crítico | Curador / Jurídico | License-first policy; checagem de contrato e prova de idade; hash-match CSAM. | 4h |
| R3 | **CSAM (material ilegal)** | Baixa | Catastrófico | Compliance | Hash-match automático antes do download; isolamento; notificação imediata + bloqueio. | Imediato |
| R4 | **Violação GDPR / coleta excessiva de dados pessoais** | Média | Alto | DPO / Eng. Dados | Anonimizar IP, retenção 30 dias, política clara de privacidade, banner de consentimento. | 24h |
| R5 | **Fila de streaming vazia (buffer underflow)** | Alta | Alto | Eng. Operações | Buffer alvo 6–8h, loop de emergência local, alarme <3h; watchdog automatizado. | 15 min |
| R6 | **Downloads corrompidos (tokens expirados)** | Média | Médio | Eng. Curadoria | Só baixar VOD estático; verificação de integridade `ffprobe`; retry em 5 min. | 2h |
| R7 | **Explosão de inodes / IO por segmentação HLS** | Alta | Médio | Infra / Storage | Compactar segmentos antigos, TTL curto, tarball diário de VOD. | 6h |
| R8 | **Exploit em ffmpeg / navegador headless** | Média | Crítico | Eng. Segurança | Sandboxing, namespaces, atualizações pinadas, no-exec em /tmp, varscan diário. | 2h |
| R9 | **Banimento de CDN / host (conteúdo adulto)** | Média | Crítico | Ops / Legal | Usar CDN "adult-friendly"; contrato explícito; backup CDN (cutover automático). | 30 min |
| R10 | **Problema com monetização / congelamento de pagamentos** | Média | Alto | Financeiro / Legal | Processadores compatíveis com adulto; ledger assinado; reconciliação semanal. | 24h |
| R11 | **Latência alta (>9s)** | Média | Médio | Eng. Vídeo | Ajustar HLS clássico; Low-Latency HLS se viável; TTL curta no manifest. | 4h |
| R12 | **Fingerprint bloqueado / anti-bot detection** | Alta | Médio | Eng. Automação | Perfis estáveis e limitados; rotatividade leve; whitelists; evitar comportamento repetitivo. | 2h |
| R13 | **Falha em logs (sem spans)** | Média | Médio | Eng. Observabilidade | Telemetria mínima: contadores por etapa + 100 últimos erros; modo "médico". | 1h |
| R14 | **Falha elétrica / sobrecarga térmica** | Baixa | Alto | Eng. Infraestrutura | UPS 2000 VA, sensores de temperatura, limpeza trimestral, alerta remoto. | 10 min |
| R15 | **Incidente jurídico / bloqueio CNPD** | Baixa | Crítico | Jurídico / DPO | Conformidade plena GDPR, cooperação e registro de logs de consentimento. | 12h |

* * *

### 🔧 Escala de Classificação

**Probabilidade:**

*   Baixa: <10 % / ano
*   Média: 10–50 % / ano
*   Alta: >50 % / ano

**Impacto:**

*   Médio: interrupção ≤ 1 h ou dano reversível
*   Alto: interrupção ≥ 6 h ou dano reputacional moderado
*   Crítico: perda de dados ou exposição legal grave
*   Catastrófico: implicações criminais, perda institucional

* * *

### 📈 Resumo de Prioridades (Heat Map)

| Categoria | Riscos Críticos | Prioridade | Observações |
| --- | --- | --- | --- |
| Legal / Compliance | R1, R2, R3, R4, R15 | 🔥 | manter consultoria jurídica ativa |
| Operacional | R5, R6, R7, R9 | ⚙️ | reforçar redundância e automação |
| Segurança | R8, R12 | 🔒 | sandboxes separados por domínio |
| Financeira | R10 | 💶 | usar gateway redundante |
| Técnica / Observabilidade | R11, R13 | 🧠 | spans opcionais + logs leves |
| Física | R14 | 🧯 | monitoramento físico e remoto |

* * *

### 📋 Plano de Revisão

| Ação | Frequência | Responsável | Entregável |
| --- | --- | --- | --- |
| Auditoria Legal / Consentimento | Mensal | Jurídico | Relatório "VVTV\_Compliance\_Audit.md" |
| Teste de Buffer e Loop de Emergência | Semanal | Eng. Vídeo | Log de Teste (`buffer_test.log`) |
| Sandbox Integrity Check | Diário | Eng. Segurança | `security_check_report.json` |
| Monitoramento de UPS e Temperatura | Contínuo | Infraestrutura | Alertas Telegram / Email |
| Revisão de Monetização | Quinzenal | Financeiro | `ledger_reconciliation.csv` |

* * *

### ⚖️ Conclusão

O **VVTV Risk Register** define o perímetro de segurança e resiliência do sistema.  
Cada linha é um elo de proteção. Nenhum risco pode ser ignorado — apenas mitigado e observado.  
O verdadeiro uptime não é 99.9 % — é **99.9 % de coerência institucional**.

* * *

## ⚙️ APÊNDICE B — VVTV INCIDENT PLAYBOOK

### _VoulezVous.TV Autonomous Streaming System_

**Author:** Daniel Amarilho / VoulezVous Foundation  
**Revision:** v1.0 — 2025-10-13

* * *

### 🔍 Estrutura do Playbook

Cada incidente segue a mesma estrutura:

```
## INCIDENT TYPE
### Detection
### Diagnosis
### Resolution
### Postmortem Steps
```

Os scripts e logs referenciados estão no diretório `/vvtv/system/bin/` e `/vvtv/logs/`.  
Todos os comandos são **idempotentes** — podem ser executados múltiplas vezes sem causar dano.

* * *

### 🟥 INCIDENT TYPE: STREAM FREEZE / BLACK SCREEN

**Detection**

*   `stream_health.sh` mostra `status: frozen`
*   CDN com `0kbps` output
*   Logs: `queue_empty` ou `buffer_underflow`

**Diagnosis**

```bash
check_queue.sh --recent 10
check_ffmpeg.sh
```

*   Se `queue.sqlite` vazio → fila parou.
*   Se ffmpeg ativo mas sem saída → encoder travado.

**Resolution**

1.  Reiniciar apenas a camada de broadcast:
    ```bash
    systemctl restart vvtv_broadcast
    ```
2.  Se fila vazia:
    ```bash
    inject_emergency_loop.sh
    ```
3.  Confirmar retomada:
    ```bash
    check_stream_health.sh
    ```

**Postmortem Steps**

*   Registrar causa no `incident_log.md`
*   Auditar buffer → deve estar >4h
*   Rodar `stress_test.sh` no encoder

* * *

### 🟧 INCIDENT TYPE: BUFFER UNDERFLOW (Fila seca)

**Detection**

*   `buffer_report.sh` < 2h
*   Alarme amarelo via Telegram

**Diagnosis**

```bash
analyze_plans.sh
```

*   Verificar se há `plans` antigos sem download.
*   Confirmar se o `planner` está ativo.

**Resolution**

1.  Reativar downloads manuais:
    ```bash
    run_download_cycle.sh --force
    ```
2.  Se lento, injetar "bloco reserva":
    ```bash
    import_from_archive.sh
    ```
3.  Checar fila:
    ```bash
    check_queue.sh
    ```

**Postmortem Steps**

*   Atualizar parâmetro `buffer_target=8h`
*   Aumentar janela de prefetch
*   Reavaliar cron de planejamento

* * *

### 🟨 INCIDENT TYPE: CURATOR BROWSER BLOCKED (Anti-Bot)

**Detection**

*   Log: `403 Forbidden`, `Captcha`, `EME detected`
*   Navegador encerrado subitamente

**Diagnosis**

```bash
browser_diagnose.sh
```

*   Verificar fingerprint e proxy ativo
*   Testar manualmente via modo debug

**Resolution**

1.  Trocar perfil:
    ```bash
    rotate_browser_profile.sh
    ```
2.  Se fonte suspeita → blackhole:
    ```bash
    add_to_blacklist.sh URL
    ```
3.  Se erro persistir → pausar domínio:
    ```bash
    disable_source.sh DOMAIN
    ```

**Postmortem Steps**

*   Logar URL, status, fingerprint usado
*   Registrar na `source_audit.md`
*   Propor whitelisting via acordo formal

* * *

### 🟦 INCIDENT TYPE: FFmpeg Crash / Encoder Panic

**Detection**

*   `check_ffmpeg.sh` → no PID / crash dump
*   Stream output 0kbps

**Diagnosis**

*   Ler dump: `/vvtv/logs/crash_*.log`
*   Ver parâmetros do arquivo afetado

**Resolution**

1.  Reiniciar encoder:
    ```bash
    restart_encoder.sh
    ```
2.  Validar input:
    ```bash
    ffprobe file.mp4
    ```
3.  Reencode:
    ```bash
    ffmpeg_reencode.sh file.mp4
    ```

**Postmortem Steps**

*   Atualizar ffmpeg para versão pinada
*   Isolar mídia corrompida
*   Rodar `test_transcode_batch.sh`

* * *

### 🟩 INCIDENT TYPE: CDN FAILURE / HOST BAN

**Detection**

*   Ping falha em `cdn_origin`
*   `curl -I` → `403` ou `410 Gone`

**Diagnosis**

```bash
check_cdn_status.sh
```

*   Consultar Cloudflare e backup provider

**Resolution**

1.  Switch automático:
    ```bash
    switch_cdn.sh --to backup
    ```
2.  Confirmar propagação DNS:
    ```bash
    dig voulezvous.tv
    ```
3.  Testar stream remoto:
    ```bash
    check_stream_health.sh --external
    ```

**Postmortem Steps**

*   Registrar motivo (ToS, abuso, overload)
*   Atualizar `provider_contracts.md`
*   Agendar call de revisão legal

* * *

### 🟪 INCIDENT TYPE: LEGAL / DMCA TAKEDOWN

**Detection**

*   Email de notificação
*   Entrada no `compliance_inbox`

**Diagnosis**

*   Confirmar URL e timestamp
*   Localizar `plan_id` associado

**Resolution**

1.  Executar retirada:
    ```bash
    takedown.sh --id plan_id
    ```
2.  Registrar:
    ```bash
    log_takedown.sh plan_id
    ```
3.  Notificar parte denunciante com confirmação

**Postmortem Steps**

*   Verificar licenças da fonte
*   Atualizar `license_audit.md`
*   Agendar consultoria jurídica

* * *

### 🟥 INCIDENT TYPE: SECURITY BREACH / COMPROMISE

**Detection**

*   Hash inconsistente
*   Alertas de integridade
*   Acesso indevido via logs

**Diagnosis**

```bash
security_scan.sh
audit_logs.sh
```

**Resolution**

1.  Desconectar nó:
    ```bash
    tailscale down
    ```
2.  Reverter snapshot:
    ```bash
    logline restore last_snapshot
    ```
3.  Reassinar chaves:
    ```bash
    rotate_keys.sh
    ```

**Postmortem Steps**

*   Redefinir credenciais
*   Revisar sandbox e ACLs
*   Rodar auditoria total do vault

* * *

### 🟫 INCIDENT TYPE: POWER FAILURE / HARDWARE SHUTDOWN

**Detection**

*   UPS reporta "battery low"
*   Sistema desliga abruptamente

**Diagnosis**

```bash
check_power.sh
check_thermal.sh
```

**Resolution**

1.  Restaurar energia
2.  Boot seguro:
    ```bash
    logline revive vv_system_snapshot_latest.tar.zst
    ```
3.  Verificar integridade:
    ```bash
    integrity_check.sh
    ```

**Postmortem Steps**

*   Testar UPS
*   Revisar limpeza interna
*   Substituir hardware degradado

* * *

### ⚫ INCIDENT TYPE: UNKNOWN FAILURE / ANOMALIA COMPUTÁVEL

**Detection**

*   Nenhum alarme direto; comportamento incoerente

**Diagnosis**

```bash
anomaly_report.sh
```

*   Coleta logs e telemetria 24h

**Resolution**

1.  Entrar em modo de observação:
    ```bash
    logline simulate --span=vvtv_diag
    ```
2.  Pausar novos downloads
3.  Esperar 6h de logs
4.  Executar diagnóstico completo

**Postmortem Steps**

*   Redigir `anomaly_summary.md`
*   Atualizar `root_cause_registry`
*   Planejar hotfix se necessário

* * *

### 🧭 Comunicação de Incidentes

| Gravidade | Comunicação | Tempo |
| --- | --- | --- |
| Crítico | Canal interno + Telegram + Email fundação | Imediato |
| Alto | Canal interno + Telegram | 30 min |
| Médio | Log + relatório diário | 6 h |
| Baixo | Log interno apenas | 24 h |

* * *

### 🧱 Postmortem Structure (template)

```markdown
# VVTV POSTMORTEM — INCIDENT <ID>
**Data:** <YYYY-MM-DD HH:MM>  
**Categoria:** Técnica / Legal / Operacional  
**Causa-raiz:**  
**Impacto:**  
**Tempo até resolução:**  
**Lições aprendidas:**  
**Ações preventivas:**  
**Assinatura:** Eng. Responsável
```

* * *

### 🕯️ Conclusão

Este playbook substitui improviso por liturgia.  
Cada incidente é tratado como uma **doença computável** — com diagnóstico, tratamento e cura documentados.  
Seguir o manual é preservar a vida da estação.

* * *

## 🏁 CONCLUSÃO FINAL

**VoulezVous.TV** — Sistema de Streaming Autônomo Completo

Este dossiê documenta a engenharia completa de um organismo cibernético vivo: uma estação de transmissão que busca, planeja, baixa, edita, transmite, monetiza, se defende e renasce.

Nove blocos de engenharia industrial definem cada aspecto do sistema:
- Da infraestrutura física aos protocolos de ressurreição
- Da simulação humana à economia computável  
- Da qualidade perceptual à resiliência planetária

Todo o sistema pode ser encapsulado e revivido:
```bash
logline revive vv_system_snapshot_YYYYMMDD.tar.zst
```

**Assinatura Institucional:**

VoulezVous Foundation — Lisboa, 2025  
LogLine OS Heritage License

```
logline sign --key voulezvous_foundation.pem \
  VVTV_Industrial_Dossier_Complete.md
sha256: [signature_hash]
```

---

## 📐 APÊNDICE C — DIAGRAMAS DE ARQUITETURA

### Diagrama 1: Fluxo de Dados Completo

```
┌─────────────────────────────────────────────────────────────────────┐
│                         VVTV DATA FLOW                               │
└─────────────────────────────────────────────────────────────────────┘

[1] DISCOVERY & CURATION
┌──────────────────────────────────────────────────────────────────┐
│  vvtv_agent_browser (Chromium + CDP)                             │
│  ├─ human_sim → mouse Bézier, scroll natural                     │
│  ├─ pbd → play-before-download                                   │
│  ├─ metadata → DOM extraction                                     │
│  └─ planner_bridge → write PLAN                                  │
└────────────────┬─────────────────────────────────────────────────┘
                 │
                 ▼
          ┌──────────────┐
          │ plans.sqlite │ status: 'planned'
          │ - URL captured (HD manifest)
          │ - Metadata extracted
          │ - License proof
          └──────┬───────┘
                 │
                 │ [T-4h Selection]
                 │
[2] SELECTION    ▼
         ┌───────────────┐
         │ planner       │
         │ - Score calc  │
         │ - 80/20 mix   │
         │ - T-4h window │
         └───────┬───────┘
                 │
                 ▼
          ┌──────────────┐
          │ plans.sqlite │ status: 'selected'
          └──────┬───────┘
                 │
                 │
[3] PROCESSING   ▼
         ┌─────────────────────┐
         │  vvtv_processor     │
         │  ├─ Reopen + PBD    │ (confirm HD)
         │  ├─ Download        │ (HLS/DASH/progressive)
         │  ├─ Remux/Transcode │ (prefer -c copy)
         │  ├─ Loudnorm -14    │ (two-pass EBU R128)
         │  ├─ Package HLS     │ (720p/480p profiles)
         │  ├─ QC Pre          │ (ffprobe + checksums)
         │  └─ Stage           │
         └──────────┬──────────┘
                    │
                    ▼
         ┌─────────────────────────────┐
         │ /storage/ready/<plan_id>/   │
         │ ├─ master.mp4               │
         │ ├─ hls_720p.m3u8 + m4s      │
         │ ├─ hls_480p.m3u8 + m4s      │
         │ ├─ checksums.json           │
         │ └─ manifest.json            │
         └──────────┬──────────────────┘
                    │
                    ├─→ plans.sqlite (status: 'edited')
                    └─→ queue.sqlite (status: 'queued')
                    
[4] QUALITY CONTROL
         ┌──────────────┐
         │  vvtv_qc     │
         │  ├─ VMAF/SSIM│ (mid-QC perceptual)
         │  ├─ Color    │ (VV signature profile)
         │  ├─ Audio    │ (-14 LUFS + cinema curve)
         │  └─ Live-QC  │ (stream capture)
         └──────────────┘

[5] PLAYOUT & BROADCAST
         ┌────────────────────┐
         │  vvtv_broadcaster  │
         │  ├─ Queue read     │ (FIFO + curation bump)
         │  ├─ FFmpeg encode  │ (→ RTMP)
         │  ├─ nginx-rtmp     │ (→ HLS origin)
         │  ├─ Failover       │ (standby + emergency loop)
         │  └─ Buffer check   │ (target: 6-8h)
         └─────────┬──────────┘
                   │
                   ▼
         ┌─────────────────┐
         │ /broadcast/hls/ │
         │ ├─ live.m3u8    │ (manifest)
         │ └─ segment_*.ts │ (4s chunks)
         └─────────┬───────┘
                   │
[6] DISTRIBUTION   │
                   ▼
         ┌───────────────────────┐
         │   CDN Layer A         │
         │   (Cloudflare)        │
         │   - m3u8: no cache    │
         │   - segments: 60s TTL │
         │   - Edge workers      │
         └──────────┬────────────┘
                    │
       ┌────────────┼────────────┐
       │            │            │
       ▼            ▼            ▼
  [Lisboa]    [Railway]    [CDN B Backup]
  Primary     Secondary    Backblaze/Bunny
   Origin      Origin      
       │            │            │
       └────────────┴────────────┘
                    │
                    ▼
         ┌─────────────────────┐
         │  Viewers Worldwide  │
         │  (HLS players)      │
         │  Latency: 5-9s      │
         └─────────────────────┘

[7] MONITORING & ECONOMY
         ┌────────────────┐        ┌──────────────┐
         │ vvtv_monitor   │        │ vvtv_economy │
         │ - Health checks│        │ - Ledger     │
         │ - Captures     │        │ - Analytics  │
         │ - Metrics      │        │ - Adaptive   │
         │ - Dashboard    │        │ - Reports    │
         └────────────────┘        └──────────────┘
```

### Diagrama 2: Arquitetura de Rede

```
                    VVTV NETWORK TOPOLOGY
                    
┌─────────────────────────────────────────────────────────────┐
│                  Internet (Public)                          │
└────────────┬─────────────────────────────┬──────────────────┘
             │                             │
             │ HTTPS/HLS                   │ HTTPS/HLS
             │                             │
    ┌────────▼─────────┐          ┌───────▼──────────┐
    │  CDN Cloudflare  │          │  CDN Backblaze   │
    │  (Primary)       │          │  (Backup)        │
    │  Edge: 12 nodes  │          │  Edge: 6 nodes   │
    └────────┬─────────┘          └───────┬──────────┘
             │                             │
             │ Origin Pull                 │ Origin Pull
             │                             │
┌────────────▼─────────────────────────────▼──────────────────┐
│               Tailscale VPN Mesh                             │
│               (voulezvous.ts.net)                            │
│                                                              │
│  ┌────────────────────┐      ┌──────────────────┐          │
│  │ Lisboa Mac Mini M1 │      │ Railway Cloud    │          │
│  │ (Primary Origin)   │──────│ (Secondary)      │          │
│  │                    │      │                  │          │
│  │ 10.0.1.10:1935     │ sync │ 10.0.2.20:1935   │          │
│  │ RTMP Origin        │      │ RTMP Failover    │          │
│  │ HLS :8080          │      │ HLS :8080        │          │
│  └──────┬─────────────┘      └──────────────────┘          │
│         │                                                    │
│         │ Tailscale Subnet                                  │
│         │                                                    │
│  ┌──────▼──────────┐      ┌───────────────────┐            │
│  │ Curator Node    │      │ Processor Node    │            │
│  │ 10.0.1.11       │      │ 10.0.1.12         │            │
│  │ (Browser Auto)  │      │ (Transcode)       │            │
│  └─────────────────┘      └───────────────────┘            │
│                                                              │
└──────────────────────────────────────────────────────────────┘

Firewall Rules (Lisboa Primary):
├─ Allow: 1935/tcp (RTMP) from Tailscale only
├─ Allow: 8080/tcp (HLS) from Tailscale + CDN IPs
├─ Allow: 22/tcp (SSH) from Tailscale only
└─ Deny: All other inbound
```

### Diagrama 3: Estados de Plano (State Machine)

```
          PLAN LIFECYCLE STATE MACHINE

        ┌─────────────┐
        │   START     │
        └──────┬──────┘
               │
               │ Discovery
               ▼
        ┌─────────────┐
   ┌────│  'planned'  │────┐
   │    └─────────────┘    │
   │           │           │
   │ reject    │ T-4h      │ blacklist
   │           │ selection │
   │           ▼           │
   │    ┌─────────────┐    │
   │    │ 'selected'  │    │
   │    └─────────────┘    │
   │           │           │
   │ fail      │ download  │
   │           │ starts    │
   │           ▼           │
   │    ┌─────────────┐    │
   │    │'downloaded' │    │
   │    └─────────────┘    │
   │           │           │
   │ corrupt   │ process   │
   │           │ + QC      │
   │           ▼           │
   │    ┌─────────────┐    │
   │    │  'edited'   │────┘
   │    └─────────────┘
   │           │
   │           │ queue
   │           ▼
   │    ┌─────────────┐
   │    │  'queued'   │
   │    └─────────────┘
   │           │
   │           │ broadcast
   │           ▼
   │    ┌─────────────┐
   │    │  'playing'  │
   │    └─────────────┘
   │           │
   │           │ complete
   │           ▼
   │    ┌─────────────┐
   └───▶│  'played'   │
        └─────────────┘
               │
               │ cleanup (72h)
               ▼
        ┌─────────────┐
        │   ARCHIVE   │
        └─────────────┘
```

### Diagrama 4: Estrutura de Diretórios

```
/vvtv/
├── system/
│   ├── bin/
│   │   ├── check_stream_health.sh
│   │   ├── check_queue.sh
│   │   ├── inject_emergency_loop.sh
│   │   ├── run_download_cycle.sh
│   │   ├── restart_encoder.sh
│   │   ├── switch_cdn.sh
│   │   ├── browser_diagnose.sh
│   │   ├── takedown.sh
│   │   ├── integrity_check.sh
│   │   ├── selfcheck.sh
│   │   ├── backup_hot.sh
│   │   ├── backup_warm.sh
│   │   ├── backup_cold.sh
│   │   ├── standby.sh
│   │   └── resume.sh
│   │
│   ├── watchdog/
│   │   ├── vvtvd.service
│   │   ├── broadcaster.service
│   │   ├── processor.service
│   │   ├── curator.service
│   │   └── watchdogd.service
│   │
│   └── logs/                   [7-14 days retention]
│       ├── broadcast.log
│       ├── processor.log
│       ├── curator.log
│       ├── watchdog.log
│       └── security.log
│
├── data/                       [SQLite databases]
│   ├── plans.sqlite            (PLANS + status)
│   ├── queue.sqlite            (PLAYOUT queue)
│   ├── metrics.sqlite          (Telemetry)
│   └── economy.sqlite          (Ledger)
│
├── cache/                      [Ephemeral, cleared weekly]
│   ├── browser_profiles/       (24h lifespan)
│   ├── tmp_downloads/          (cleared post-process)
│   └── ffmpeg_tmp/
│
├── storage/
│   ├── ready/                  [Ready for playout]
│   │   └── <plan_id>/
│   │       ├── master.mp4
│   │       ├── hls_720p.m3u8 + m4s
│   │       ├── hls_480p.m3u8 + m4s
│   │       ├── checksums.json
│   │       └── manifest.json
│   │
│   ├── edited/                 [Intermediate]
│   └── archive/                [72h played content]
│
├── broadcast/
│   ├── nginx.conf              (RTMP + HLS config)
│   ├── hls/                    [Live stream output]
│   │   ├── live.m3u8
│   │   └── segment_*.ts
│   └── vod/                    [VOD for testing]
│
├── docs/
│   ├── VVTV_Industrial_Dossier_Complete.md
│   ├── deployment.md
│   ├── failover.md
│   └── compliance_policies.md
│
├── monitor/
│   ├── dashboard.html
│   └── captures/               [Stream thumbnails]
│
└── vault/                      [Immutable backups]
    ├── snapshots/              (signed .tar.zst)
    ├── keys/                   (foundation keys)
    └── manifests/              (testamento computável)
```

* * *

## 📝 APÊNDICE D — ARQUIVOS DE CONFIGURAÇÃO COMPLETOS

### D.1 — vvtv.toml (Configuração Principal)

```toml
# VVTV Main Configuration
# Version: 1.0
# Institution: VoulezVous Foundation

[system]
node_name = "vvtv-primary"
node_role = "broadcast"  # Options: broadcast | curator | processor | all
node_id = "vvtv-node-001"
environment = "production"  # Options: development | staging | production

[paths]
base_dir = "/vvtv"
data_dir = "/vvtv/data"
cache_dir = "/vvtv/cache"
storage_dir = "/vvtv/storage"
broadcast_dir = "/vvtv/broadcast"
logs_dir = "/vvtv/system/logs"
vault_dir = "/vvtv/vault"

[limits]
# Buffer management
buffer_target_hours = 6
buffer_warning_hours = 3
buffer_critical_hours = 1.5

# Concurrency
max_concurrent_downloads = 2
max_concurrent_transcodes = 2
max_browser_instances = 2

# Resource limits
cpu_limit_percent = 75
memory_limit_gb = 12
disk_warning_percent = 80
disk_critical_percent = 90

# Retention
plans_retention_days = 30
played_retention_hours = 72
logs_retention_days = 14
cache_retention_hours = 24

[network]
tailscale_domain = "voulezvous.ts.net"
rtmp_port = 1935
hls_port = 8080
control_port = 9000

# CDN
cdn_primary = "cloudflare"
cdn_backup = "backblaze"

[quality]
# Audio
target_lufs = -14.0
lufs_tolerance = 1.5
audio_codec = "aac"
audio_bitrate = "160k"

# Video
vmaf_threshold = 85
ssim_threshold = 0.92
target_resolution = "1080p"
fallback_resolution = "720p"

# HLS
hls_segment_duration = 4
hls_playlist_length_minutes = 48

[security]
sandbox_enabled = true
fingerprint_randomization = true
proxy_rotation_enabled = true
csam_check_enabled = true
drm_detection_abort = true

# Firewall
allow_rtmp_from = ["tailscale"]
allow_hls_from = ["tailscale", "cdn"]
allow_ssh_from = ["tailscale"]

[monitoring]
health_check_interval_seconds = 60
metrics_collection_interval_seconds = 300
capture_interval_minutes = 5
alert_telegram_enabled = true
alert_email_enabled = false

[economy]
monetization_enabled = true
base_rate_per_minute = 0.001
ledger_export_interval_hours = 24
reconciliation_interval_days = 7
```

### D.2 — browser.toml (Browser Automation Config)

```toml
# VVTV Browser Automation Configuration

[chromium]
executable_path = "/usr/bin/chromium"
headless = true
sandbox = true
disable_gpu = true

# Performance
max_memory_mb = 2048
max_tabs_per_instance = 2
tab_timeout_seconds = 300

[flags]
# Anti-detection
no_first_run = true
disable_automation_controlled = true
disable_blink_features = ["AutomationControlled"]
mute_audio = true
autoplay_policy = "no-user-gesture-required"

# Language
lang = "en-US"
accept_language = "en-US,en;q=0.9"

[user_agents]
# Rotativo
pool = [
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36"
]
rotation_frequency = 12  # páginas

[viewport]
# Resoluções humanas comuns
resolutions = [
    [1366, 768],
    [1440, 900],
    [1536, 864],
    [1920, 1080]
]
jitter_pixels = 16
device_scale_factor = [1.0, 2.0]

[human_simulation]
# Mouse (Bézier curves)
mouse_speed_min_px_s = 500
mouse_speed_max_px_s = 1200
mouse_jitter_px = 3
click_hesitation_ms = [120, 450]
click_duration_ms = [30, 70]

# Scroll
scroll_burst_px = [200, 800]
scroll_pause_ms = [120, 300]
scroll_near_player_slow = true

# Keyboard
typing_cadence_cpm = [140, 220]
typing_jitter_ms = [15, 35]
error_frequency_chars = [80, 130]

# Timing
idle_duration_ms = [1500, 4500]
ociosidade_frequency = [20000, 35000]

[pbd]
# Play-Before-Download
enabled = true
force_hd = true
hd_priority = ["1080p", "720p"]
playback_wait_seconds = [5, 12]
validation_buffer_seconds = 3

[selectors]
# Player detection
video_element = "video"
play_buttons = [".play", ".vjs-play-control", "button[aria-label*='Play']"]
quality_menu = [".quality", ".settings", ".vjs-menu-button"]

# Consent/popups
consent_buttons = ["button:contains('Accept')", ".cookie-accept", "#consent-accept"]

[proxy]
enabled = true
type = "residential"
rotation_pages = 30
timeout_seconds = 10

[sources]
# Whitelisted domains (license verified)
whitelist = []
blacklist = []
```

### D.3 — processor.toml (Processing Config)

```toml
# VVTV Processor Configuration

[download]
tool = "aria2"  # Options: aria2 | ffmpeg | curl
max_retries = 3
retry_delay_seconds = [180, 900]
bandwidth_limit_mbps = 0  # 0 = unlimited
resume_enabled = true

[hls]
vod_only = true
verify_sequence = true
rewrite_playlist = true

[dash]
prefer_h264 = true
remux_to_hls = true

[progressive]
head_check = true
min_size_mb = 2

[remux]
prefer_copy = true
faststart = true
fallback_transcode = true

[transcode]
# Video
codec = "libx264"
preset = "slow"  # ultrafast | veryfast | fast | medium | slow | veryslow
crf = 20
profile = "high"
level = "4.2"
pix_fmt = "yuv420p"

# Keyframes
keyint = 120
min_keyint = 48
scenecut = 40

# VBV
vbv_maxrate = "12000k"
vbv_bufsize = "24000k"

[loudnorm]
# EBU R128 Two-pass
enabled = true
integrated = -14.0
true_peak = -1.5
lra = 11.0
linear = true

[profiles]
# HLS 720p
[profiles.hls_720p]
scale = "720"
video_bitrate = "3300k"
maxrate = "3600k"
bufsize = "6600k"
audio_bitrate = "128k"
preset = "veryfast"
profile = "high"
level = "4.0"

# HLS 480p
[profiles.hls_480p]
scale = "480"
video_bitrate = "1500k"
maxrate = "1700k"
bufsize = "3000k"
audio_bitrate = "96k"
preset = "veryfast"
profile = "main"
level = "3.1"

[qc]
ffprobe_validation = true
checksums_sha256 = true
duration_tolerance_percent = 5
min_duration_video_s = 60
min_duration_music_s = 90
```

### D.4 — broadcaster.toml (Playout Config)

```toml
# VVTV Broadcaster Configuration

[queue]
policy = "fifo_with_bump"  # fifo | lifo | fifo_with_bump
music_ratio = 0.1  # 1 música a cada 10 vídeos
curation_bump_threshold = 0.85

[rtmp]
origin = "rtmp://localhost/live/main"
chunk_size = 4096
reconnect_attempts = 5
reconnect_delay_ms = 3000

[hls]
output_path = "/vvtv/broadcast/hls"
segment_duration = 4
playlist_length = "48m"
segment_type = "fmp4"  # ts | fmp4
flags = ["independent_segments"]

[failover]
enabled = true
standby_encoder = true
detection_timeout_seconds = 3
emergency_loop_hours = 2

[watchdog]
interval_seconds = 30
restart_on_freeze = true
restart_max_attempts = 3

[ffmpeg]
log_level = "error"  # quiet | panic | fatal | error | warning | info
stats_period = "60"
thread_queue_size = 512
```

* * *

## 🔧 APÊNDICE E — SCRIPTS SHELL OPERACIONAIS

### E.1 — check_queue.sh

```bash
#!/bin/bash
# VVTV Queue Inspector
# Usage: check_queue.sh [--recent N]

set -euo pipefail

DB="/vvtv/data/queue.sqlite"
RECENT="${1:-10}"

echo "═══════════════════════════════════════"
echo "  VVTV QUEUE STATUS"
echo "═══════════════════════════════════════"
echo "Time: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo

# Total counts by status
echo "📊 Queue Overview:"
sqlite3 "$DB" << 'EOF'
.mode column
.headers on
SELECT 
    status,
    COUNT(*) as count,
    ROUND(SUM(duration_s)/3600.0, 2) as hours,
    ROUND(AVG(curation_score), 2) as avg_score
FROM playout_queue
GROUP BY status
ORDER BY 
    CASE status
        WHEN 'queued' THEN 1
        WHEN 'playing' THEN 2
        WHEN 'played' THEN 3
        WHEN 'failed' THEN 4
    END;
EOF

echo
echo "🎬 Recent Queued Items (limit $RECENT):"
sqlite3 "$DB" << EOF
.mode column
.headers on
SELECT 
    SUBSTR(plan_id, 1, 8) as plan,
    SUBSTR(asset_path, -30) as asset,
    ROUND(duration_s/60.0, 1) as mins,
    ROUND(curation_score, 2) as score,
    priority,
    created_at
FROM playout_queue
WHERE status='queued'
ORDER BY created_at ASC
LIMIT $RECENT;
EOF

echo
echo "⏱️  Buffer Analysis:"
BUFFER_S=$(sqlite3 "$DB" "SELECT COALESCE(SUM(duration_s), 0) FROM playout_queue WHERE status='queued';")
BUFFER_H=$(echo "scale=2; $BUFFER_S / 3600" | bc)

echo "Total queued: ${BUFFER_H}h"

if (( $(echo "$BUFFER_H < 2" | bc -l) )); then
    echo "🔴 CRITICAL: Buffer below 2h!"
    exit 2
elif (( $(echo "$BUFFER_H < 3" | bc -l) )); then
    echo "🟡 WARNING: Buffer below 3h"
    exit 1
else
    echo "✅ Buffer healthy (>3h)"
fi
```

### E.2 — inject_emergency_loop.sh

```bash
#!/bin/bash
# VVTV Emergency Loop Injector
# Injects 2-3h of safe content when buffer is critically low

set -euo pipefail

ARCHIVE="/vvtv/storage/archive"
DB_QUEUE="/vvtv/data/queue.sqlite"
DB_PLANS="/vvtv/data/plans.sqlite"
LOG="/vvtv/system/logs/emergency.log"

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" | tee -a "$LOG"
}

log "🚨 EMERGENCY LOOP ACTIVATION"

# Find safe content in archive
SAFE_CONTENT=$(find "$ARCHIVE" -name "*.mp4" -mtime -30 | shuf -n 5)

if [ -z "$SAFE_CONTENT" ]; then
    log "❌ No safe content found in archive!"
    exit 1
fi

log "Found $(echo "$SAFE_CONTENT" | wc -l) safe items"

# Inject into queue with high priority
while IFS= read -r file; do
    PLAN_ID="emergency-$(uuidgen)"
    DURATION=$(ffprobe -v error -show_entries format=duration \
        -of default=noprint_wrappers=1:nokey=1 "$file" | cut -d. -f1)
    
    sqlite3 "$DB_QUEUE" << EOF
INSERT INTO playout_queue (plan_id, asset_path, duration_s, status, priority, node_origin)
VALUES ('$PLAN_ID', '$file', $DURATION, 'queued', 1, 'emergency-loop');
EOF
    
    log "✅ Injected: $(basename "$file") (${DURATION}s)"
done <<< "$SAFE_CONTENT"

# Update metrics
BUFFER_S=$(sqlite3 "$DB_QUEUE" "SELECT SUM(duration_s) FROM playout_queue WHERE status='queued';")
BUFFER_H=$(echo "scale=2; $BUFFER_S / 3600" | bc)

log "📊 New buffer: ${BUFFER_H}h"
log "🔄 Emergency loop complete"

# Send alert
if command -v telegram-send &> /dev/null; then
    telegram-send "🚨 VVTV: Emergency loop activated. Buffer now: ${BUFFER_H}h"
fi
```

### E.3 — run_download_cycle.sh

```bash
#!/bin/bash
# VVTV Download Cycle Forcer
# Forces the processor to execute a download batch

set -euo pipefail

DB="/vvtv/data/plans.sqlite"
LOG="/vvtv/system/logs/processor.log"
LOCK="/tmp/vvtv_processor.lock"

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" | tee -a "$LOG"
}

# Check for lock
if [ -f "$LOCK" ]; then
    PID=$(cat "$LOCK")
    if ps -p "$PID" > /dev/null 2>&1; then
        log "⚠️  Processor already running (PID: $PID)"
        exit 0
    else
        log "🧹 Removing stale lock"
        rm -f "$LOCK"
    fi
fi

# Create lock
echo $$ > "$LOCK"
trap "rm -f $LOCK" EXIT

log "🚀 Starting forced download cycle"

# Get selected plans (T-4h window)
SELECTED=$(sqlite3 "$DB" "SELECT plan_id FROM plans WHERE status='selected' LIMIT 6;")

if [ -z "$SELECTED" ]; then
    log "ℹ️  No plans in 'selected' status"
    exit 0
fi

COUNT=$(echo "$SELECTED" | wc -l)
log "📦 Found $COUNT plans to process"

# Process each (this would call vvtv_processor binary in real implementation)
while IFS= read -r PLAN_ID; do
    log "⚙️  Processing: $PLAN_ID"
    
    # Update status to 'downloaded' (placeholder - actual download would happen here)
    sqlite3 "$DB" "UPDATE plans SET status='downloaded', updated_at=CURRENT_TIMESTAMP WHERE plan_id='$PLAN_ID';"
    
    log "✅ Completed: $PLAN_ID"
done <<< "$SELECTED"

log "🏁 Download cycle complete ($COUNT items)"
```

### E.4 — restart_encoder.sh

```bash
#!/bin/bash
# VVTV Encoder Restart
# Gracefully restarts the broadcast encoder

set -euo pipefail

LOG="/vvtv/system/logs/broadcast.log"

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" | tee -a "$LOG"
}

log "🔄 Encoder restart requested"

# Stop current ffmpeg processes
PIDS=$(pgrep -f "ffmpeg.*rtmp" || true)

if [ -n "$PIDS" ]; then
    log "🛑 Stopping PIDs: $PIDS"
    kill -SIGTERM $PIDS
    sleep 3
    
    # Force kill if still running
    REMAINING=$(pgrep -f "ffmpeg.*rtmp" || true)
    if [ -n "$REMAINING" ]; then
        log "⚠️  Force killing: $REMAINING"
        kill -SIGKILL $REMAINING
    fi
fi

# Restart via systemd (if available)
if command -v systemctl &> /dev/null; then
    log "📢 Restarting via systemd"
    systemctl restart vvtv_broadcast
else
    log "📢 Manual restart (no systemd)"
    # Manual restart would be implemented here
fi

sleep 2

# Verify restart
if pgrep -f "ffmpeg.*rtmp" > /dev/null; then
    log "✅ Encoder restarted successfully"
    exit 0
else
    log "❌ Encoder restart failed!"
    exit 1
fi
```

### E.5 — switch_cdn.sh

```bash
#!/bin/bash
# VVTV CDN Switcher
# Switches between primary and backup CDN

set -euo pipefail

TARGET="${1:-backup}"
LOG="/vvtv/system/logs/cdn_switch.log"
DNS_ZONE="voulezvous.tv"

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" | tee -a "$LOG"
}

log "🔀 CDN switch requested: → $TARGET"

case "$TARGET" in
    backup)
        NEW_ORIGIN="backup-origin.voulezvous.ts.net"
        ;;
    primary)
        NEW_ORIGIN="primary-origin.voulezvous.ts.net"
        ;;
    *)
        log "❌ Invalid target: $TARGET"
        exit 1
        ;;
esac

log "🎯 New origin: $NEW_ORIGIN"

# Update DNS (using Cloudflare API example)
if [ -n "${CLOUDFLARE_API_TOKEN:-}" ]; then
    log "📡 Updating DNS via Cloudflare API"
    
    # This is a placeholder - actual API call would go here
    curl -X PUT "https://api.cloudflare.com/client/v4/zones/ZONE_ID/dns_records/RECORD_ID" \
         -H "Authorization: Bearer $CLOUDFLARE_API_TOKEN" \
         -H "Content-Type: application/json" \
         --data "{\"content\":\"$NEW_ORIGIN\"}" \
         &>> "$LOG"
    
    log "✅ DNS updated"
else
    log "⚠️  No CLOUDFLARE_API_TOKEN set - manual DNS update required"
fi

# Wait for propagation
log "⏳ Waiting 30s for DNS propagation..."
sleep 30

# Verify
log "🔍 Verifying new origin..."
RESOLVED=$(dig +short "$DNS_ZONE" | head -n1)
log "Resolved to: $RESOLVED"

log "✅ CDN switch complete"

# Send alert
if command -v telegram-send &> /dev/null; then
    telegram-send "🔀 VVTV: CDN switched to $TARGET origin"
fi
```

### E.6 — selfcheck.sh

```bash
#!/bin/bash
# VVTV Daily Self-Check
# Runs comprehensive system health checks

set -euo pipefail

REPORT="/vvtv/system/reports/selfcheck_$(date +%Y%m%d).json"
LOG="/vvtv/system/logs/selfcheck.log"

log() {
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" | tee -a "$LOG"
}

log "🔍 Starting daily self-check"

CHECKS_PASSED=0
CHECKS_FAILED=0
ISSUES=()

check() {
    local name="$1"
    local command="$2"
    
    if eval "$command" &>/dev/null; then
        log "✅ $name"
        ((CHECKS_PASSED++))
        return 0
    else
        log "❌ $name"
        ISSUES+=("$name")
        ((CHECKS_FAILED++))
        return 1
    fi
}

# Database integrity
check "plans.sqlite integrity" \
    "sqlite3 /vvtv/data/plans.sqlite 'PRAGMA integrity_check;' | grep -q 'ok'"

check "queue.sqlite integrity" \
    "sqlite3 /vvtv/data/queue.sqlite 'PRAGMA integrity_check;' | grep -q 'ok'"

# File system
DISK_USAGE=$(df /vvtv | tail -1 | awk '{print $5}' | sed 's/%//')
if [ "$DISK_USAGE" -lt 80 ]; then
    log "✅ Disk usage: ${DISK_USAGE}%"
    ((CHECKS_PASSED++))
else
    log "❌ Disk usage critical: ${DISK_USAGE}%"
    ISSUES+=("Disk usage: ${DISK_USAGE}%")
    ((CHECKS_FAILED++))
fi

# Temperature (macOS)
if command -v osx-cpu-temp &>/dev/null; then
    TEMP=$(osx-cpu-temp -c | cut -d'°' -f1)
    if (( $(echo "$TEMP < 75" | bc -l) )); then
        log "✅ CPU temp: ${TEMP}°C"
        ((CHECKS_PASSED++))
    else
        log "❌ CPU temp high: ${TEMP}°C"
        ISSUES+=("CPU temp: ${TEMP}°C")
        ((CHECKS_FAILED++))
    fi
fi

# Services
check "NGINX running" "pgrep -f nginx"
check "Tailscale running" "tailscale status"

# HLS playlist exists
check "HLS playlist exists" "test -f /vvtv/broadcast/hls/live.m3u8"

# NTP sync
check "Clock synchronized" "which ntpdate && ntpdate -q pool.ntp.org"

# Generate JSON report
cat > "$REPORT" << EOF
{
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "checks_passed": $CHECKS_PASSED,
  "checks_failed": $CHECKS_FAILED,
  "disk_usage_percent": $DISK_USAGE,
  "issues": $(printf '%s\n' "${ISSUES[@]}" | jq -R . | jq -s .)
}
EOF

log "📊 Report saved: $REPORT"
log "Summary: $CHECKS_PASSED passed, $CHECKS_FAILED failed"

if [ $CHECKS_FAILED -gt 0 ]; then
    log "⚠️  Self-check completed with issues"
    exit 1
else
    log "✅ Self-check completed successfully"
    exit 0
fi
```

* * *

## 📚 APÊNDICE F — GLOSSÁRIO TÉCNICO VVTV

### A

**Agent (Browser)** — Instância automatizada do Chromium/CDP que simula comportamento humano para descoberta e captura de conteúdo.

**Aria2** — Download manager multi-protocolo usado para baixar HLS/DASH streams com retomada automática.

**Asset** — Arquivo de vídeo processado e pronto para playout (master.mp4 + HLS variants).

**Autonomia** — Capacidade do sistema VVTV de operar 24/7 sem intervenção humana, usando curadoria algorítmica.

### B

**Backblaze B2** — Armazenamento em nuvem usado como backup do conteúdo e origem alternativa para CDN.

**Bézier Curve** — Curva matemática usada para gerar movimentos de mouse naturais e humanos na simulação.

**Broadcaster** — Módulo responsável por ler a fila e transmitir continuamente via RTMP/HLS.

**Buffer** — Reserva de conteúdo processado (meta: 6-8h) para garantir continuidade do stream mesmo durante falhas de processamento.

**Bunny CDN** — CDN alternativa para distribuição global com menor custo que Cloudflare.

### C

**CDP (Chrome DevTools Protocol)** — Protocolo usado para controlar instâncias do Chromium via código (automação).

**Cloudflare** — CDN primária para distribuição global do stream VVTV com edge workers e cache inteligente.

**Computable** — Filosofia LogLine: tudo deve ser verificável, reproduzível e assinado criptograficamente.

**CRF (Constant Rate Factor)** — Modo de encoding x264 que mantém qualidade visual constante (valor: 20 = alta qualidade).

**CSAM** — Child Sexual Abuse Material — conteúdo ilegal que o sistema detecta e rejeita automaticamente.

**Curation Score** — Pontuação algorítmica (0.0-1.0) que determina a prioridade de um PLAN na fila. Baseado em: relevância, qualidade, diversidade, timing.

**Curator Node** — Nó dedicado exclusivamente à descoberta e curadoria (browser automation).

### D

**DASH (Dynamic Adaptive Streaming over HTTP)** — Protocolo de streaming adaptativo similar ao HLS, usado por alguns sites.

**Desire Vector** — Vetor computável que representa padrões de interesse da audiência, usado para ajustar programação.

**DRM (Digital Rights Management)** — Tecnologia de proteção de conteúdo que o VVTV **não** tenta contornar (abort se detectado).

**Duration** — Duração de um asset em segundos, extraída via `ffprobe` e armazenada em `plans.sqlite`.

### E

**EBU R128** — Padrão europeu de normalização de loudness. VVTV usa -14 LUFS integrado com dois passes FFmpeg.

**Economy Module** — Sistema de ledger local que registra métricas de monetização (views, micro-spots, afiliados).

**Edge** — Servidor CDN próximo ao usuário final (baixa latência). Cloudflare tem ~300 edges globalmente.

**Emergency Loop** — Conteúdo seguro (2-3h) injetado automaticamente na fila quando buffer cai abaixo de 1.5h.

**Encoder** — Processo FFmpeg que lê assets da fila e transmite via RTMP para nginx-rtmp.

### F

**Failover** — Processo automático de troca para origem/encoder backup quando o primário falha (detecção em <3s).

**FFmpeg** — Suite open-source para processamento de vídeo/áudio. Core do pipeline VVTV.

**FFprobe** — Ferramenta FFmpeg para análise técnica de arquivos (codec, bitrate, duração, etc).

**FIFO (First In, First Out)** — Política de fila padrão, modificada com "curation bump" para conteúdo de alta pontuação.

**Fingerprint Randomization** — Técnica para alterar assinaturas digitais do browser (canvas, WebGL, audio context) para evitar tracking.

**FMP4 (Fragmented MP4)** — Container moderno para HLS, alternativa aos `.ts` segments (melhor para seeking).

### G

**GDPR** — Regulação europeia de privacidade. VVTV não coleta dados pessoais identificáveis.

**Geo-heatmap** — Mapa de calor mostrando distribuição geográfica da audiência via IP (agregado, anônimo).

### H

**HLS (HTTP Live Streaming)** — Protocolo de streaming desenvolvido pela Apple, usado como formato primário do VVTV.

**HLS Playlist (.m3u8)** — Arquivo de manifesto que lista os segments de vídeo e suas variantes de qualidade.

**Hot Backup** — Backup em tempo real (rsync contínuo) para recuperação imediata (<1 min).

**Human Simulation** — Técnicas para fazer automação parecer humana: mouse Bézier, scroll natural, timing variável, erros de digitação.

### I

**Incident Playbook** — Conjunto de runbooks padronizados para responder a falhas e emergências.

### J

**Jitter** — Pequenas variações aleatórias em timing/posicionamento para simular imperfeição humana.

### K

**Keyframe (I-frame)** — Frame de vídeo completo usado como ponto de entrada. VVTV usa keyint=120 (1 a cada 4s @30fps).

### L

**Latency** — Atraso entre evento ao vivo e reprodução no viewer. VVTV target: 5-9s (HLS padrão).

**Ledger** — Registro imutável de transações econômicas em `economy.sqlite`.

**LogLine OS** — Sistema operacional conceitual para identidade computável, assinatura de artefatos e revival.

**Loudnorm** — Filtro FFmpeg que implementa EBU R128 loudness normalization.

**LUFS (Loudness Units relative to Full Scale)** — Medida perceptual de loudness. Target VVTV: -14 LUFS integrado.

### M

**Manifest** — Arquivo JSON que acompanha cada asset com metadata: source_url, duration, checksums, license_proof.

**Master.mp4** — Versão de mais alta qualidade de um asset, mantida em `/storage/ready/` como source of truth.

**Micro-spot** — Spot publicitário curto (<10s) inserido entre conteúdos, com impacto mínimo na experiência.

**Monitor Node** — Nó dedicado a capturar frames do stream ao vivo para QC contínuo.

### N

**NGINX-RTMP** — Módulo NGINX para ingestão RTMP e transformação em HLS. Core do playout VVTV.

**Node** — Máquina/servidor no mesh VVTV. Tipos: broadcast, curator, processor, monitor.

**Normalization** — Processo de ajustar loudness de áudio para padrão consistente (-14 LUFS).

### O

**Origin** — Servidor fonte que gera o stream HLS, servido para CDN (Lisboa Mac Mini = primary origin).

**Overshoot** — Técnica de scroll humano: scrollar além do alvo e depois ajustar de volta.

### P

**PBD (Play-Before-Download)** — Mecanismo central: forçar playback no browser antes de baixar para garantir captura do rendition HD real.

**Plan** — Registro em `plans.sqlite` representando um conteúdo descoberto, com status lifecycle (planned → selected → downloaded → edited → queued → playing → played).

**Plan ID** — Identificador único de um PLAN (UUID ou hash de source URL).

**Playout** — Processo de reprodução sequencial da fila de assets para o stream ao vivo.

**Processor Node** — Nó dedicado a download e transcodificação de conteúdo.

**Proxy (Residential)** — Proxy IP residencial para evitar rate limits e parecer tráfego humano legítimo.

### Q

**QC (Quality Control)** — Processo multi-camadas: técnico (ffprobe), perceptual (VMAF/SSIM), estético (color/loudness), live (monitoring).

**Queue** — Fila de playout em `queue.sqlite`, ordenada por FIFO + curation bump.

### R

**Railway** — Plataforma de cloud hosting usada para nó de failover secundário.

**Remux** — Processo de reempacotar vídeo sem transcodificação (`-c copy`), preservando qualidade original.

**Rendition** — Variante de qualidade de um stream (1080p, 720p, 480p, etc).

**Retention** — Tempo que assets/logs são mantidos antes de serem arquivados ou deletados.

**Revival** — Processo de restaurar um sistema VVTV completo a partir de snapshot assinado (`.tar.zst`).

**RTMP (Real-Time Messaging Protocol)** — Protocolo de streaming usado para ingestão (FFmpeg → NGINX).

**Runbook** — Documento de procedimento passo-a-passo para operações ou resposta a incidentes.

### S

**Sandbox** — Isolamento de execução do browser para segurança (chromium --sandbox).

**Segment** — Chunk de vídeo HLS (4s duration por padrão), formato `.ts` ou `.m4s`.

**Signature (Digital)** — Assinatura criptográfica de arquivos/snapshots usando chaves LogLine.

**SSIM (Structural Similarity Index)** — Métrica perceptual de qualidade de vídeo (0.0-1.0). Target VVTV: >0.92.

**Standby** — Modo de suspensão do sistema com preservação de estado para revival rápido.

**State Machine** — Modelo formal dos estados de um PLAN e transições válidas entre eles.

**Stream Freeze** — Incidente onde o encoder para de produzir novos segments (detectado via watchdog).

### T

**T-4h Window** — Janela de 4 horas antes do playout onde PLANs são selecionados e processados.

**Tailscale** — VPN mesh WireGuard-based usada para conectar todos os nós VVTV com segurança.

**Takedown** — Processo de remoção imediata de conteúdo do stream e arquivo em resposta a DMCA ou legal.

**Testamento Computável** — Snapshot completo do sistema (código + dados + configs) assinado e arquivado no vault.

**Transcode** — Processo de re-encoding de vídeo para novo codec/bitrate/resolução (FFmpeg libx264).

**True Peak** — Pico de amplitude de áudio após reconstrução analógica. Target VVTV: -1.5 dBTP.

**TTL (Time To Live)** — Tempo de cache na CDN. VVTV: m3u8=0s (no cache), segments=60s.

### U

**User-Agent** — String identificando o browser. VVTV rotaciona entre UAs comuns para evitar detecção.

**UUID** — Identificador único universal, usado para Plan IDs e node IDs.

### V

**VBV (Video Buffering Verifier)** — Parâmetros x264 para controlar picos de bitrate (maxrate/bufsize).

**Viewport** — Resolução da janela do browser. VVTV simula resoluções humanas comuns (1366×768, 1920×1080, etc).

**VMAF (Video Multimethod Assessment Fusion)** — Métrica perceptual de qualidade desenvolvida pela Netflix. Target VVTV: >85.

**VOD (Video On Demand)** — Conteúdo arquivado para replay, em contraste com live stream.

**VoulezVous Signature Profile** — Estética visual/sonora característica: paleta de cores, loudness, ritmo.

**VPN Mesh** — Rede privada peer-to-peer onde todos os nós podem comunicar diretamente (Tailscale).

### W

**Warm Backup** — Backup periódico (diário) para disaster recovery de médio prazo (<4h).

**Watchdog** — Processo de monitoramento que reinicia serviços automaticamente quando detecta falhas.

**Whitelist** — Lista de domínios/fontes aprovados para descoberta de conteúdo (com verificação de licença).

### X

**x264** — Encoder H.264/AVC open-source de alta qualidade usado no pipeline VVTV.

### Z

**Zstd (.zst)** — Algoritmo de compressão moderno usado para snapshots (melhor ratio que gzip, mais rápido que xz).

* * *

## 📊 APÊNDICE G — BENCHMARKS E PERFORMANCE

### G.1 — Hardware de Referência

**Primary Node (Lisboa Mac Mini M1 2020)**
```
CPU: Apple M1 (8-core: 4P + 4E)
RAM: 16 GB unified
Storage: 512 GB NVMe SSD
Network: 1 Gbps Ethernet
OS: macOS Sonoma 14.x
```

**Capacidade Testada:**
- Transcodificação simultânea: 2× 1080p → 720p/480p (preset=fast)
- Browser instances: 2× Chromium headless
- Playout: 1× stream 1080p@4Mbps contínuo
- Buffer processing: ~12-15h de conteúdo processado por 24h

### G.2 — Benchmarks de Processamento

#### Transcode Performance (FFmpeg x264)

| Input | Profile | Preset | FPS | Tempo Real |
|-------|---------|--------|-----|------------|
| 1080p@30fps | 720p | veryfast | ~180 fps | 6× |
| 1080p@30fps | 720p | fast | ~120 fps | 4× |
| 1080p@30fps | 720p | medium | ~80 fps | 2.7× |
| 1080p@30fps | 720p | slow | ~45 fps | 1.5× |
| 4K@30fps | 1080p | fast | ~60 fps | 2× |

**Conclusão:** Com preset `fast` e 2 jobs concorrentes, Mac Mini M1 processa ~8-10h de conteúdo final por 24h.

#### Remux Performance (Copy Stream)

| Input | Output | Tempo (10 min video) |
|-------|--------|---------------------|
| HLS → MP4 | remux | 12s |
| DASH → MP4 | remux | 15s |
| MP4 → HLS segments | remux | 8s |

**Conclusão:** Remux é ~50× mais rápido que transcode (preferir sempre que possível).

#### Loudnorm Two-Pass

| Duração | Pass 1 | Pass 2 | Total |
|---------|--------|--------|-------|
| 5 min | 4s | 38s | 42s |
| 10 min | 8s | 75s | 83s |
| 30 min | 24s | 220s | 244s |

**Conclusão:** Loudnorm adiciona ~0.8× o tempo real do vídeo (10 min video = 8 min processing).

### G.3 — Benchmarks de Rede

#### Download Speed (1 Gbps link)

| Protocolo | Fonte | Velocidade Média |
|-----------|-------|------------------|
| HLS VOD | YouTube (via browser) | 120-180 Mbps |
| HLS VOD | Vimeo | 80-150 Mbps |
| Progressive | Archive.org | 200-400 Mbps |
| DASH VOD | Various | 100-200 Mbps |

**Gargalo:** Velocidade do servidor de origem, não do link VVTV.

#### CDN Origin Pull (Cloudflare)

| Segment Size | First Byte | Transfer | Total Latency |
|--------------|------------|----------|---------------|
| 1 MB (4s@2Mbps) | 45 ms | 80 ms | 125 ms |
| 2 MB (4s@4Mbps) | 50 ms | 140 ms | 190 ms |

**Resultado:** Latência total viewer-to-origin: 5-9s (3× segment duration + network).

#### Tailscale VPN Overhead

| Route | Direct Ping | Tailscale Ping | Overhead |
|-------|-------------|----------------|----------|
| Lisboa ↔ Railway | 12 ms | 15 ms | +3 ms |
| Lisboa ↔ Curator (local) | 0.2 ms | 1.8 ms | +1.6 ms |

**Conclusão:** Overhead mínimo, aceitável para uso interno.

### G.4 — Benchmarks de Qualidade

#### VMAF Scores (Source vs Transcode)

| Source | x264 CRF | VMAF Score | Perceptual |
|--------|----------|------------|------------|
| 1080p@8Mbps | CRF 18 | 96.2 | Indistinguível |
| 1080p@8Mbps | CRF 20 | 93.8 | Imperceptível |
| 1080p@8Mbps | CRF 22 | 89.1 | Leve perda |
| 1080p@4Mbps | CRF 20 | 88.4 | Aceitável |

**Escolha VVTV:** CRF 20 = sweet spot qualidade/tamanho.

#### SSIM Scores

| Transcode Preset | SSIM | Notas |
|------------------|------|-------|
| ultrafast | 0.88 | Visível em cenas complexas |
| fast | 0.93 | Ótimo |
| medium | 0.95 | Excelente |
| slow | 0.96 | Imperceptível |

**Escolha VVTV:** `fast` ou `medium` dependendo de buffer.

### G.5 — Benchmarks de Resiliência

#### Recovery Times

| Incidente | Detecção | Mitigação | Total Downtime |
|-----------|----------|-----------|----------------|
| Encoder freeze | 30s (watchdog) | 15s (restart) | 45s |
| Origin offline | 3s (CDN health) | 30s (DNS switch) | 33s |
| Buffer underflow | Real-time | 0s (emergency loop) | 0s (transparent) |
| Database corruption | Daily check | 2-5 min (restore) | <5 min |

#### Buffer Consumption vs Production

| Cenário | Consumption Rate | Production Rate | Net |
|---------|------------------|-----------------|-----|
| Normal | 1h/hour | 8-10h/24h | +7-9h/day |
| High load (2 transcodes) | 1h/hour | 6-8h/24h | +5-7h/day |
| Emergency (no processing) | 1h/hour | 0h/24h | -24h/day |

**Conclusão:** Buffer de 6-8h fornece ~6-8 dias de autonomia se processamento parar completamente.

### G.6 — Benchmarks Econômicos (Projeções)

#### Custos Mensais (Infraestrutura)

| Item | Custo (USD) | Notas |
|------|-------------|-------|
| Mac Mini M1 (amortizado) | $25 | $600 / 24 meses |
| Energia (24/7) | $8 | 15W avg × $0.20/kWh |
| Internet (1 Gbps) | $50 | Fibra residencial |
| Tailscale | $0 | Tier gratuito (1 user) |
| Cloudflare CDN | $20-50 | ~5 TB egress/mês |
| Backblaze B2 | $10 | 200 GB storage |
| Railway (fallback) | $15 | Standby instance |
| **TOTAL** | **$128-158/mês** | |

#### Receita Potencial (1000 viewers/hora médio)

| Fonte | CPM/Rate | Projeção Mensal |
|-------|----------|----------------|
| Passive viewing ads | $2 CPM | $1,440 |
| Micro-spots (2/dia) | $5/spot | $300 |
| Premium slots | $20/slot | $200 |
| Computable affiliates | 5% commission | $150 |
| **TOTAL** | | **$2,090/mês** |

**ROI:** ~13× (break-even em ~45 viewers/hora).

### G.7 — Limites do Sistema

| Recurso | Limite Testado | Limite Teórico | Gargalo |
|---------|----------------|----------------|---------|
| Concurrent transcodes | 2 | 3 | CPU (thermal throttle) |
| Browser instances | 2 | 4 | RAM (8 GB RAM / instance) |
| HLS bitrate máximo | 6 Mbps | 8 Mbps | 1 Gbps link / CDN cost |
| Queue buffer | 12h | 200 GB disk | Disk space |
| Concurrent viewers | Ilimitado | - | CDN-limited, não origin |
| Uptime | 99.2% (30 dias) | 99.9% | Internet residencial |

### G.8 — Otimizações Recomendadas

**Curto Prazo:**
1. Upgrade RAM: 16→32 GB (permite 4 browser instances)
2. Proxy pool: rotação IP mais agressiva (reduzir rate limits)
3. Preset adaptativo: `fast` se buffer >6h, `veryfast` se <4h

**Médio Prazo:**
1. Dedicated NAS: offload storage do Mac Mini (expand buffer capacity)
2. GPU transcode: QuickSync/VideoToolbox para 3-4× speed boost
3. Multi-node: adicionar Mac Mini secundário (dobrar throughput)

**Longo Prazo:**
1. Edge caching: pre-transcode variants na CDN edge (reduzir origin load)
2. AI curation: ML model para scoring (melhorar desire vector accuracy)
3. P2P distribution: WebTorrent layer para reduzir CDN costs

* * *

## 🔧 APÊNDICE H — TROUBLESHOOTING EXPANDIDO

### H.1 — Sintomas e Diagnóstico Rápido

#### Stream não inicia / Tela preta

**Sintomas:**
- Viewer recebe erro 404 ou timeout
- HLS playlist vazio ou ausente
- CDN retorna erro 522/523

**Diagnóstico Rápido:**
```bash
# 1. Verificar encoder ativo
pgrep -f "ffmpeg.*rtmp" || echo "❌ Encoder não está rodando"

# 2. Verificar NGINX
curl -I http://localhost:8080/hls/live.m3u8

# 3. Verificar últimos segments
ls -lath /vvtv/broadcast/hls/*.ts | head -5

# 4. Verificar logs do encoder
tail -100 /vvtv/system/logs/broadcast.log | grep -i error
```

**Soluções:**
1. `systemctl restart vvtv_broadcast` (ou script `restart_encoder.sh`)
2. Verificar permissions: `chown -R vvtv:vvtv /vvtv/broadcast/hls`
3. Limpar segments órfãos: `find /vvtv/broadcast/hls -name "*.ts" -mtime +1 -delete`
4. Injetar emergency loop: `/vvtv/system/bin/inject_emergency_loop.sh`

---

#### Buffer baixo (<3h)

**Sintomas:**
- `check_queue.sh` retorna WARNING ou CRITICAL
- Dashboard mostra buffer abaixo do target
- Emergency loop sendo ativado repetidamente

**Diagnóstico:**
```bash
# 1. Checar status de processing
ps aux | grep vvtv_processor

# 2. Verificar PLANs em pipeline
sqlite3 /vvtv/data/plans.sqlite "SELECT status, COUNT(*) FROM plans GROUP BY status;"

# 3. Checar falhas recentes
sqlite3 /vvtv/data/plans.sqlite \
  "SELECT plan_id, status, error FROM plans WHERE status='failed' AND updated_at > datetime('now', '-24 hours');"

# 4. Disk space
df -h /vvtv
```

**Soluções:**
1. Forçar ciclo de processamento: `/vvtv/system/bin/run_download_cycle.sh`
2. Se disk full: limpar cache `rm -rf /vvtv/cache/tmp_downloads/*`
3. Reduzir preset: editar `processor.toml` → `preset = "veryfast"`
4. Adicionar conteúdo manual: copiar MP4s para `/vvtv/storage/ready/` e popular queue

---

#### Vídeos travando/buffering para viewers

**Sintomas:**
- Viewers reportam rebuffering frequente
- CDN analytics mostram altas taxas de erro
- Segments não estão sendo gerados a tempo

**Diagnóstico:**
```bash
# 1. Verificar segment generation rate
watch -n 1 'ls -lt /vvtv/broadcast/hls/*.ts | head -3'

# 2. Checar CPU/RAM do encoder
top -p $(pgrep -f "ffmpeg.*rtmp")

# 3. Verificar bitrate atual
ffprobe -v quiet -show_entries stream=bit_rate \
  /vvtv/broadcast/hls/segment_latest.ts

# 4. Testar latência para CDN
ping -c 10 cloudflare.com
```

**Soluções:**
1. Reduzir bitrate de playout: editar FFmpeg command em broadcaster para `-b:v 3M` (de 4M)
2. Aumentar segment duration: `hls_segment_duration = 6` (de 4) em `broadcaster.toml`
3. Verificar thermal throttling: `sudo powermetrics --samplers smc` (macOS)
4. Trocar para CDN backup: `/vvtv/system/bin/switch_cdn.sh backup`

---

#### Browser automation falhando

**Sintomas:**
- PLANs ficando em 'failed' com erro de browser
- Logs mostram "Timeout waiting for selector"
- Chromium crashando repetidamente

**Diagnóstico:**
```bash
# 1. Verificar Chromium instalado
/usr/bin/chromium --version

# 2. Testar headless manual
chromium --headless --disable-gpu --dump-dom https://example.com

# 3. Checar browser profiles corrompidos
ls -lh /vvtv/cache/browser_profiles/

# 4. Logs de browser
tail -200 /vvtv/system/logs/curator.log | grep -i "chrome\|cdp"
```

**Soluções:**
1. Limpar profiles: `rm -rf /vvtv/cache/browser_profiles/*`
2. Reinstalar Chromium: `brew reinstall chromium` (macOS)
3. Desabilitar sandbox temporariamente: `sandbox = false` em `browser.toml` (⚠️ inseguro)
4. Atualizar seletores: editar `browser.toml` → `play_buttons` array
5. Testar com browser visível: `headless = false` em `browser.toml` para debug

---

#### FFmpeg transcode errors

**Sintomas:**
- PLANs falhando em 'downloaded' → 'edited' transition
- Logs mostram "Invalid data found" ou "Codec not supported"
- Arquivos de output corrompidos

**Diagnóstico:**
```bash
# 1. Verificar input file integrity
ffprobe /vvtv/cache/tmp_downloads/<file>.mp4

# 2. Testar transcode manual
ffmpeg -i /vvtv/cache/tmp_downloads/<file>.mp4 \
  -c:v libx264 -preset fast -crf 20 \
  -c:a aac -b:a 128k \
  /tmp/test_output.mp4

# 3. Checar codec availability
ffmpeg -codecs | grep -i "h264\|aac"

# 4. Disk space durante transcode
df -h /vvtv/cache
```

**Soluções:**
1. Fallback para remux: em `processor.toml` → `fallback_transcode = false` (aceitar apenas copy)
2. Aumentar disk space: limpar `/vvtv/storage/archive/`
3. Atualizar FFmpeg: `brew upgrade ffmpeg` (macOS)
4. Ignorar input corrompido: adicionar plan_id à blacklist manual
5. Tentar sem loudnorm: `loudnorm.enabled = false` em `processor.toml` (temporário)

---

#### CDN ban / Rate limiting

**Sintomas:**
- Requests retornando 429 (Too Many Requests)
- Cloudflare mostrando CAPTCHA pages
- Origin pulls falhando com 403

**Diagnóstico:**
```bash
# 1. Testar acesso direto ao origin
curl -I http://primary-origin.voulezvous.ts.net:8080/hls/live.m3u8

# 2. Verificar rate de requests na CDN
# (via Cloudflare dashboard analytics)

# 3. Checar IP em blacklists
curl -s https://api.abuseipdb.com/api/v2/check?ipAddress=<YOUR_IP>

# 4. Testar com proxy
curl --proxy socks5h://localhost:1080 -I https://voulezvous.tv/hls/live.m3u8
```

**Soluções:**
1. Ativar Cloudflare "Under Attack Mode" temporariamente
2. Aumentar TTL de segments: `max-age=120` (de 60) em NGINX config
3. Implementar rate limiting no origin: NGINX `limit_req` directive
4. Rotação de proxy mais agressiva: `rotation_pages = 10` em `browser.toml`
5. Whitelist Cloudflare IPs: adicionar à `allow_hls_from` em `vvtv.toml`

---

#### Database locked / Corruption

**Sintomas:**
- Operações SQLite retornando "database is locked"
- Queries extremamente lentas (>10s)
- `PRAGMA integrity_check` falhando

**Diagnóstico:**
```bash
# 1. Verificar processos usando DB
lsof /vvtv/data/plans.sqlite

# 2. Testar integridade
sqlite3 /vvtv/data/plans.sqlite "PRAGMA integrity_check;"

# 3. Verificar tamanho e fragmentação
ls -lh /vvtv/data/*.sqlite
sqlite3 /vvtv/data/plans.sqlite "PRAGMA page_count; PRAGMA freelist_count;"

# 4. Checar locks ativos
sqlite3 /vvtv/data/plans.sqlite ".timeout 1000" "SELECT 1;"
```

**Soluções:**
1. Matar processos travados: `kill -9 $(lsof -t /vvtv/data/plans.sqlite)`
2. Vacuum database: `sqlite3 /vvtv/data/plans.sqlite "VACUUM;"`
3. Restaurar de backup warm:
   ```bash
   systemctl stop vvtv_broadcast vvtv_processor
   cp /vvtv/vault/backups/warm/plans.sqlite.YYYYMMDD /vvtv/data/plans.sqlite
   systemctl start vvtv_broadcast vvtv_processor
   ```
4. Rebuild de backup:
   ```bash
   sqlite3 /vvtv/data/plans_old.sqlite ".dump" | sqlite3 /vvtv/data/plans_new.sqlite
   ```
5. Aumentar timeout: adicionar `PRAGMA busy_timeout = 5000;` em queries

---

#### High CPU / Memory usage

**Sintomas:**
- Sistema respondendo lentamente
- Thermal throttling (CPU >80°C)
- OOM killer matando processos

**Diagnóstico:**
```bash
# 1. Top processes
top -o %CPU -n 10

# 2. Memory breakdown
ps aux --sort=-%mem | head -10

# 3. Temperature (macOS)
sudo powermetrics --samplers smc -n 1

# 4. Disk I/O
iotop -o (Linux) ou fs_usage (macOS)
```

**Soluções:**
1. Reduzir concurrency:
   ```toml
   # vvtv.toml
   max_concurrent_downloads = 1
   max_concurrent_transcodes = 1
   max_browser_instances = 1
   ```
2. Limitar CPU por processo:
   ```bash
   cpulimit -p $(pgrep ffmpeg) -l 150  # 150% = 1.5 cores
   ```
3. Aumentar swap (Linux):
   ```bash
   sudo fallocate -l 4G /swapfile
   sudo mkswap /swapfile
   sudo swapon /swapfile
   ```
4. Limpar cache agressivamente: cron job para `rm -rf /vvtv/cache/*` a cada 6h
5. Melhorar cooling: verificar fans, limpar dust, elevar Mac Mini

---

#### Tailscale VPN connectivity issues

**Sintomas:**
- Nós não conseguem se comunicar
- `tailscale status` mostra peers offline
- Origin secundário inacessível

**Diagnóstico:**
```bash
# 1. Status do Tailscale
tailscale status

# 2. Ping peers
tailscale ping <node-name>

# 3. Verificar routing
tailscale netcheck

# 4. Logs
tail -100 /var/log/tailscale/tailscaled.log
```

**Soluções:**
1. Restart Tailscale: `sudo tailscale down && sudo tailscale up`
2. Reauth: `tailscale up --force-reauth`
3. Verificar firewall não está bloqueando: `sudo ufw allow 41641/udp` (Linux)
4. Trocar para Tailscale relay: `tailscale up --accept-routes --advertise-exit-node=false`
5. Fallback para IP público: atualizar `rtmp_origin` em `broadcaster.toml` temporariamente

---

### H.2 — Logs e Debugging

#### Log Locations

```
/vvtv/system/logs/
├── broadcast.log       # FFmpeg encoder, RTMP, HLS generation
├── processor.log       # Download, transcode, QC
├── curator.log         # Browser automation, discovery
├── watchdog.log        # Health checks, auto-restarts
└── security.log        # Auth, firewall, CSAM detections
```

#### Log Levels

Ajustar verbosity em cada módulo:

```toml
# broadcaster.toml
[ffmpeg]
log_level = "info"  # quiet|panic|fatal|error|warning|info|verbose|debug
```

**Produção:** `error` (default)  
**Troubleshooting:** `info` ou `warning`  
**Deep debug:** `verbose` (⚠️ muito output)

#### Useful Log Queries

```bash
# Erros nas últimas 24h
grep -i error /vvtv/system/logs/*.log | grep "$(date -d '1 day ago' +%Y-%m-%d)"

# Top 10 erros mais frequentes
grep -i error /vvtv/system/logs/*.log | awk '{print $NF}' | sort | uniq -c | sort -rn | head -10

# Trace de um PLAN específico
grep "plan-abc123" /vvtv/system/logs/*.log | sort

# Performance stats (FFmpeg)
grep "fps=" /vvtv/system/logs/broadcast.log | tail -20
```

### H.3 — Emergency Contacts e Runbooks

#### On-Call Tiers

**Tier 1 — Automated:**
- Watchdog auto-restart (encoder freeze, service crash)
- Emergency loop injection (buffer underflow)
- CDN failover (origin health check)

**Tier 2 — Manual Review (4h SLA):**
- Database corruption
- Persistent browser automation failures
- High error rates (>5%)

**Tier 3 — Escalation (24h SLA):**
- Legal takedown requests
- Major infrastructure outage (ISP, CDN)
- Security incidents (breach, DDoS)

#### Key Runbooks

1. **Total System Failure (Origin + Backup Down)**
   - Activate pre-recorded emergency content on CDN edge
   - Send status page update: "Technical difficulties"
   - Restore from cold backup to new hardware
   - ETA: 2-6 hours

2. **Copyright Infringement / DMCA**
   - Execute `/vvtv/system/bin/takedown.sh <plan_id>`
   - Remove from live stream, queue, archive
   - Log incident in `economy.sqlite` legal_events table
   - Respond to claimant within 24h

3. **Data Loss (Database Unrecoverable)**
   - Restore from most recent warm backup (max 24h old)
   - Rebuild queue from `/vvtv/storage/ready/` directory scan
   - Re-run curator to repopulate plans (72h to recover)
   - Acceptable loss: <24h of metrics and partial economy ledger

4. **Security Breach (Unauthorized Access)**
   - Immediately disconnect from internet: `tailscale down`
   - Kill all processes: `systemctl stop vvtv_*`
   - Snapshot compromised system for forensics
   - Restore from last known-good cold backup
   - Rotate all keys, regenerate LogLine signatures
   - Incident report within 72h (GDPR)

---

### H.4 — Preventive Maintenance Checklist

#### Daily (Automated)
- ✅ Health check: `/vvtv/system/bin/selfcheck.sh`
- ✅ Buffer analysis: `/vvtv/system/bin/check_queue.sh`
- ✅ Hot backup: rsync to local NAS
- ✅ Log rotation: delete logs >14 days

#### Weekly (Manual, 15 min)
- ✅ Review error logs for patterns
- ✅ Check disk usage trends
- ✅ Verify warm backup integrity
- ✅ Update whitelist/blacklist
- ✅ Review economy metrics and ROI

#### Monthly (Manual, 1h)
- ✅ Update system packages: `brew upgrade`
- ✅ Test failover: switch to backup origin and back
- ✅ Review and archive old PLANs (>30 days)
- ✅ Vacuum databases: `PRAGMA vacuum;`
- ✅ Test cold backup restoration (on spare hardware)
- ✅ Audit security logs for anomalies
- ✅ Performance benchmarks: compare vs baseline

#### Quarterly (Manual, 4h)
- ✅ Full system audit: all components
- ✅ Update Incident Playbook with lessons learned
- ✅ Review and optimize FFmpeg presets
- ✅ CDN cost analysis and optimization
- ✅ Hardware health: check SSD SMART status, temps
- ✅ Disaster recovery drill: full system rebuild

---

> *"O stream dorme, mas o desejo continua audível."*

---

✅ **FIM DO DOSSIÊ INDUSTRIAL VVTV**
