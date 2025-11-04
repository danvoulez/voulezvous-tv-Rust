📘 **VVTV Industrial Dossier — Full Technical Architecture**
------------------------------------------------------------

**VoulezVous.TV Autonomous Streaming System**

**Author:** Dan Voulez  
**Institution:** VoulezVous Foundation / LogLine OS  
**Revision:** v2.0 – 2025-10-22

Este dossiê é o manual completo de engenharia do sistema VoulezVous.TV: uma estação de streaming autônoma 24/7 que opera sem APIs, com navegador real, simulação humana, play-before-download, processamento automático, programação adaptativa com IA e ressurreição computável.

O sistema combina um **motor Rust determinístico** que executa 95% do processamento pesado com um **LLM Curador** que fornece 5% de refinamento inteligente, criando uma arquitetura híbrida de alta performance com capacidades adaptativas.

O sistema está dividido em **nove blocos** de engenharia detalhada, cobrindo desde a infraestrutura física até os protocolos de desligamento e revival, incluindo a arquitetura completa de business logic e programação adaptativa.

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
8. **[Bloco VII — Monetization & Adaptive Programming](#bloco-vii--monetization-analytics--adaptive-programming)** — Economia, analytics, IA adaptativa
9. **[Bloco VIII — Maintenance](#bloco-viii--maintenance-security--long-term-resilience)** — Backups, security, aging
10. **[Bloco IX — Decommission](#bloco-ix--decommission--resurrection-protocols)** — Desligamento e ressurreição
11. **[Apêndice A — Risk Register](#-apêndice-a--vvtv-risk-register)** — Matriz de riscos
12. **[Apêndice B — Incident Playbook](#-apêndice-b--vvtv-incident-playbook)** — Resposta a incidentes
13. **[Apêndice C — Business Logic Schema](#-apêndice-c--business-logic-schema)** — Configuração YAML completa
14. **[Apêndice D — LLM Integration Patterns](#-apêndice-d--llm-integration-patterns)** — Handlers, payloads, SLA

### Atalhos Rápidos

- **Hardware Mínimo:** [Seção 2.1](#21-hardware-recomendado)
- **Stack de Software:** [Seção 3.1](#31-os-e-configuração)
- **Estrutura de Diretórios:** [Seção 3.2](#32-estrutura-de-diretórios)
- **Business Logic Config:** [Seção 7.2](#72-configuração-de-negócio-business_logicyaml)
- **Adaptive Programming:** [Seção 7.6](#76-programação-adaptativa-engine)
- **LLM Integration:** [Seção 7.8](#78-integração-llm-e-circuit-breakers)
- **Play-Before-Download:** [Seção 3 - Bloco II](#3-play-before-download-pbd)
- **FFmpeg Pipelines:** [Seção 5 - Bloco III](#5-transcodificação--normalização)
- **RTMP/HLS Origin:** [Seção 5 - Bloco IV](#5-rtmphls-origin)
- **Troubleshooting:** [Apêndice B](#-apêndice-b--vvtv-incident-playbook)

* * *
## 🚀 
QUICK START GUIDE

### Visão Geral

Este guia permite iniciar um nó VVTV funcional em **~2 horas**. Para produção completa, siga os 9 blocos detalhados.

O sistema opera através de uma arquitetura híbrida onde configurações de negócio em YAML controlam um motor Rust determinístico, com suporte opcional a LLM para refinamento inteligente.

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

#### Passo 4: Configuração Principal

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
business_logic = "/vvtv/system/business_logic.yaml"

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

#### Passo 5: Configuração de Business Logic

Criar arquivo `/vvtv/system/business_logic.yaml`:

```yaml
policy_version: "2025.10"
env: "production"
knobs:
  boost_bucket: "music"
  music_mood_focus:
    - "focus"
    - "midnight"
  interstitials_ratio: 0.08
  plan_selection_bias: 0.0
scheduling:
  slot_duration_minutes: 15
  global_seed: 4242
selection:
  method: gumbel_top_k
  temperature: 0.85
  top_k: 12
  seed_strategy: slot_hash
exploration:
  epsilon: 0.12
autopilot:
  enabled: false
  max_daily_variation: 0.05
kpis:
  primary:
    - "selection_entropy"
  secondary:
    - "curator_apply_budget_used_pct"
```

#### Passo 6: Inicializar Bancos de Dados

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
    updated_at DATETIME,
    llm_rationale TEXT,
    selection_seed INTEGER
);
CREATE INDEX idx_plans_status ON plans(status);
CREATE INDEX idx_plans_score ON plans(curation_score DESC);
CREATE INDEX idx_plans_seed ON plans(selection_seed);
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
    node_origin TEXT,
    curator_intervention BOOLEAN DEFAULT 0,
    llm_confidence REAL
);
CREATE INDEX idx_queue_status ON playout_queue(status, created_at);
CREATE INDEX idx_queue_curator ON playout_queue(curator_intervention);
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
    vmaf_live REAL,
    selection_entropy REAL,
    curator_interventions_h INTEGER,
    llm_success_rate REAL
);
CREATE INDEX idx_metrics_ts ON metrics(ts DESC);
EOF
```

#### Passo 7: Validar Business Logic

```bash
# Validar configuração
vvtvctl business-logic validate

# Verificar status
vvtvctl business-logic show

# Testar seleção determinística
vvtvctl business-logic test-selection --dry-run
```

#### Passo 8: Script de Health Check

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

# Check business logic
BL_STATUS=$(vvtvctl business-logic show --format json | jq -r '.status')
echo "🧠 Business Logic: $BL_STATUS" | tee -a "$LOG_FILE"

# Check LLM integration (if enabled)
LLM_STATUS=$(vvtvctl curator status --format json | jq -r '.llm_circuit_breaker // "disabled"')
echo "🤖 LLM Status: $LLM_STATUS" | tee -a "$LOG_FILE"

if (( $(echo "$BUFFER_H < 2" | bc -l) )); then
    echo "⚠️  WARNING: Buffer below 2h!" | tee -a "$LOG_FILE"
fi

echo "✅ Health check complete" | tee -a "$LOG_FILE"
```

```bash
chmod +x /vvtv/system/bin/check_stream_health.sh
```

#### Passo 9: Configurar NGINX-RTMP

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
        
        location /business-logic {
            return 200 '{"business_logic":"active","adaptive_programming":"enabled"}';
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
curl http://localhost:8080/business-logic

# 4. Verificar Tailscale
tailscale status

# 5. Verificar Business Logic
vvtvctl business-logic validate
vvtvctl business-logic show

# 6. Health check completo
/vvtv/system/bin/check_stream_health.sh
```

### Próximos Passos

Após a instalação básica:

1. **Implementar módulos Rust** (seguir Bloco II-IX para detalhes)
2. **Configurar browser automation** (Chromium + CDP)
3. **Setup do processor** (download + transcode)
4. **Configurar broadcaster** (fila → RTMP)
5. **Ativar LLM integration** (opcional, para refinamento IA)
6. **Deploy de produção** (Railway, CDN, monitoramento)

### Comandos Úteis

```bash
# Business Logic
vvtvctl business-logic show --format json
vvtvctl business-logic reload
vvtvctl business-logic test-selection --plans 10

# Curator System
vvtvctl curator status
vvtvctl curator review --confidence-threshold 0.7

# Verificar status geral
/vvtv/system/bin/check_stream_health.sh

# Ver logs em tempo real
tail -f /vvtv/system/logs/*.log

# Inspecionar fila com business logic
sqlite3 /vvtv/data/queue.sqlite "SELECT plan_id, curation_score, curator_intervention, llm_confidence FROM playout_queue LIMIT 10;"

# Reiniciar encoder (quando implementado)
systemctl restart vvtv_broadcast

# Limpar cache
rm -rf /vvtv/cache/tmp_downloads/*
```

### Troubleshooting Rápido

| Problema | Solução |
|----------|---------|
| NGINX não inicia | Verificar porta 1935/8080 livre: `sudo lsof -i :1935` |
| Business Logic inválido | `vvtvctl business-logic validate` e corrigir YAML |
| LLM timeout | Verificar circuit breaker: `vvtvctl curator status` |
| Bancos corrompidos | Restaurar backup: `cp /vvtv/vault/data_backup.db /vvtv/data/` |
| Fila vazia | Ver [Apêndice B - Buffer Underflow](#-incident-type-buffer-underflow-fila-seca) |
| Stream congelado | Ver [Apêndice B - Stream Freeze](#-incident-type-stream-freeze--black-screen) |

### Suporte

- **Documentação completa:** Blocos I-IX deste dossiê
- **Business Logic:** [Apêndice C](#-apêndice-c--business-logic-schema)
- **LLM Integration:** [Apêndice D](#-apêndice-d--llm-integration-patterns)
- **Riscos e mitigações:** [Apêndice A](#-apêndice-a--vvtv-risk-register)
- **Resposta a incidentes:** [Apêndice B](#-apêndice-b--vvtv-incident-playbook)

* * *🧠 VVTV IND
USTRIAL DOSSIER
==========================

### **Bloco I — Infraestrutura Base e Filosofia de Engenharia**

* * *

1\. Filosofia Industrial do Sistema
-----------------------------------

O **VVTV (VoulezVous.TV)** é um sistema de transmissão contínua de vídeos adultos 24h/dia, que opera sem API, sem interface administrativa e sem dependência de nuvem.  
A máquina age diretamente no mundo físico — busca, planeja, baixa, edita e transmite através de uma arquitetura híbrida que combina determinismo computacional com inteligência adaptativa.

O design segue cinco princípios inegociáveis:

1.  **Autonomia mecânica total** — o sistema deve se recuperar, reiniciar, reagir, limpar, e continuar sozinho.
2.  **Imersão realista** — todas as interações com a web ocorrem como se um humano estivesse diante da tela.
3.  **Ciclo fechado** — nada depende de cron jobs externos ou orquestradores cloud.
4.  **Consistência industrial** — logs, buffers, cache, latência e limpeza seguem métricas fixas, nunca intuitivas.
5.  **Programação adaptativa** — o sistema aprende e evolui através de feedback loops e inteligência artificial.

O resultado é uma estação transmissora viva, que se comporta como um funcionário sem descanso, mas com a capacidade de aprender e se adaptar.

* * *

2\. Infraestrutura Física — Sala da Máquina
-------------------------------------------

### 2.1 Hardware Recomendado

| Função | Modelo | Especificação mínima | Observações |
| --- | --- | --- | --- |
| **Node Principal (Broadcast)** | Mac mini M1 (16 GB RAM, SSD 512 GB) | CPU ARM64, macOS 13+, Ethernet gigabit | Local: Loja VoulezVous |
| **Node de Curadoria** | Mac mini M1 (8 GB RAM, SSD 256 GB) | Opera browser automation + LLM integration | Conectado via Tailscale |
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
    *   `vvtv-core` (motor Rust principal)

**Desativar completamente:**

*   Spotlight, Siri, Sleep, Time Machine, Screensaver.

### 3.2 Estrutura de Diretórios

```
/vvtv/
├── system/
│   ├── bin/                    # binários internos
│   ├── scripts/                # automações shell/rust
│   ├── watchdog/               # monitoramento
│   ├── logs/                   # logs rotativos 7d
│   ├── business_logic.yaml     # configuração de negócio
│   └── vvtv.toml              # configuração principal
├── data/
│   ├── plans.sqlite           # planos de conteúdo
│   ├── queue.sqlite           # fila de exibição
│   ├── metrics.sqlite         # métricas e telemetria
│   └── curator_logs/          # logs do curator vigilante
├── cache/
│   ├── browser_profiles/      # perfis de navegador
│   ├── tmp_downloads/         # downloads temporários
│   └── ffmpeg_tmp/           # processamento temporário
├── storage/
│   ├── ready/                # conteúdo pronto para exibição
│   ├── edited/               # conteúdo processado
│   └── archive/              # arquivo histórico
└── broadcast/
    ├── rtmp.conf             # configuração RTMP
    ├── hls/                  # stream HLS ativo
    └── vod/                  # vídeo sob demanda
```

**Permissões:**

*   tudo roda como usuário `vvtv` (UID 9001).
*   `chown -R vvtv:vvtv /vvtv`
*   `chmod 755` nos binários, `chmod 600` nos bancos.

* * *

4\. Arquitetura de Software — O Cérebro Híbrido
-----------------------------------------------

### 4.1 Módulos Principais

| Módulo | Linguagem | Função | Integração IA |
| --- | --- | --- | --- |
| `discovery_browser` | Rust + JS (Chromium control) | busca, coleta e simulação humana | Circuit breaker para LLM hints |
| `planner` | Rust | cria e mantém base de planos | Gumbel-Top-k selection + LLM rerank |
| `human_sim` | Rust + JS | movimenta cursor, cliques, rolagem, delay humano | Padrões adaptativos |
| `realizer` | Rust | escolhe planos a realizar 4 h antes | Business logic integration |
| `processor` | Rust + FFmpeg | baixa, converte, normaliza | QC automático |
| `broadcaster` | Rust + Nginx-RTMP | transmite fila de exibição | Curator Vigilante monitoring |
| `business_logic` | Rust | carrega e aplica configurações YAML | Core do sistema adaptativo |
| `llm_orchestrator` | Rust | gerencia integração com LLM | Circuit breakers e fallbacks |
| `curator_vigilante` | Rust | monitora e intervém na programação | Token bucket e sinais estéticos |

Cada módulo comunica-se por **arquivos e bancos locais**, nunca por API.  
O sistema é um **pipeline de estados**, cada um alterando diretamente os registros em SQLite.

### 4.2 Fluxo Geral Híbrido

```
[BROWSER] → [PLANNER + LLM] → [CURATOR VIGILANTE] → [REALIZER] → [PROCESSOR] → [BROADCASTER]
     ↑              ↓                    ↓                                            ↓
[BUSINESS LOGIC] ← [FEEDBACK LOOP] ← [METRICS] ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ← ←
```

1.  O navegador encontra conteúdo e grava o _plan_.
2.  O planner pontua e seleciona usando Gumbel-Top-k + business logic.
3.  O LLM (opcional) fornece refinamento e reordenação.
4.  O Curator Vigilante monitora e intervém quando necessário.
5.  O realizer desperta planos a 4 h do slot.
6.  O processor baixa e edita.
7.  O broadcaster injeta na fila e exibe.
8.  Métricas alimentam o feedback loop para ajustes automáticos.

### 4.3 Linguagem e Padrões

*   Rust edition 2021
*   Async runtime: **tokio**
*   Logging: **tracing** (modo estruturado em produção)
*   CLI utilitária: `vvtvctl` (com subcomandos para business logic)
*   Configuração: `TOML` + `YAML` (business logic)
*   Serialização: `serde_json`
*   Jobs periódicos: `cron_rs`
*   Randomness: `ChaCha20Rng` (determinístico e auditável)
*   LLM Integration: `reqwest` + circuit breakers
*   Observabilidade: métricas via arquivo JSON local + JSONL logs

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
*   **Business Logic Security:**
    *   Configurações YAML assinadas digitalmente.
    *   Validação rigorosa de bounds e tipos.
    *   Audit trail completo de mudanças.
*   **LLM Security:**
    *   Token budgets e rate limiting.
    *   Circuit breakers para falhas.
    *   Nenhum PII enviado para serviços externos.

* * *

6\. Elementos Humanos e de Ergonomia
------------------------------------

*   Operador (quando presente) usa **luvas cinza-claro antiestáticas**.
*   Monitores devem ter temperatura de cor **5600 K**, brilho fixo 60 %.
*   A iluminação do ambiente deve ser **neutra**, sem tons quentes, para evitar fadiga.
*   Cada estação possui botão físico "STOP STREAM" vermelho, ligado ao script `/vvtv/system/bin/halt_stream.sh`.
*   A cor da unha (grafite fosco) repete-se nas alavancas do painel físico — consistência sensorial para manter o estado mental estável durante manutenção noturna.
*   **Dashboard de Business Logic** acessível via `vvtvctl business-logic show` para monitoramento em tempo real.

* * *

7\. Conclusão do Bloco I
------------------------

Este primeiro bloco define **o chão da fábrica híbrida**: onde a máquina vive, como respira, e quais condições físicas e lógicas garantem que ela nunca pare, enquanto evolui continuamente através de inteligência adaptativa.  
Nada aqui é teórico; são padrões operacionais absolutos que suportam tanto o determinismo Rust quanto a flexibilidade da programação adaptativa.  
A partir desse ponto, cada próximo bloco entrará no nível microscópico — automação inteligente, browser simulation, pipelines ffmpeg, fila adaptativa e controle de qualidade com IA.

* * *🧠 VVTV INDU
STRIAL DOSSIER
==========================

**Bloco VII — Monetization, Analytics & Adaptive Programming**
--------------------------------------------------------------

_(economia computável, leitura de audiência, receita distribuída e programação adaptativa baseada em desejo real e inteligência artificial)_

* * *

### 0\. Propósito do Bloco

O **Bloco VII** define o coração econômico e inteligente do VoulezVous.TV: como o sistema transforma cada minuto transmitido em valor mensurável, auditável e recorrente, enquanto adapta sua programação através de algoritmos de desejo computável, inteligência artificial e feedback loops automáticos.

O sistema opera através de uma **arquitetura híbrida** onde um **motor Rust determinístico** executa 95% do processamento pesado, enquanto um **LLM Curador** fornece 5% de refinamento e sugestões estéticas. Esta combinação permite economia viva com monetização adaptativa e rotinas de ajuste de programação em tempo real.

### 1\. Arquitetura de Programação Adaptativa

```
┌─────────────────────────────────────────────────────────────┐
│                  ARQUITETURA HÍBRIDA                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌────────────────┐         ┌──────────────────┐          │
│  │  Cartão Dono   │────────→│  Rust Engine     │          │
│  │  (YAML)        │         │  (Determinístico)│          │
│  │  business_logic│         │  95% do trabalho │          │
│  └────────────────┘         └──────────────────┘          │
│         │                            ↓                      │
│         │                    ┌──────────────────┐          │
│         │                    │   LLM Curador    │          │
│         └───────────────────→│   (Conselheiro)  │          │
│                              │   5% sugestões   │          │
│                              └──────────────────┘          │
│                                       ↓                     │
│                              ┌──────────────────┐          │
│                              │   Autopilot      │          │
│                              │   (D+1 feedback) │          │
│                              └──────────────────┘          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Princípio Fundamental:** O motor Rust executa todo o trabalho pesado de forma determinística e auditável, enquanto o LLM atua como "azeite" - o refinamento que suaviza e aprimora as decisões sem comprometer a estabilidade do sistema.

### 2\. Configuração de Negócio (business_logic.yaml)

O sistema é controlado por um arquivo de configuração YAML que define todos os parâmetros de negócio e comportamento adaptativo:

```yaml
policy_version: "2025.10"
env: "production"

# Controles de Programação
knobs:
  boost_bucket: "music"                    # Categoria prioritária
  music_mood_focus: ["focus", "midnight"] # Moods musicais preferidos
  interstitials_ratio: 0.08               # 8% de micro-anúncios
  plan_selection_bias: 0.0                # Bias de seleção (-0.2 a +0.2)

# Agendamento Temporal
scheduling:
  slot_duration_minutes: 15               # Janelas de 15 minutos
  global_seed: 4242                       # Seed para reprodutibilidade

# Algoritmo de Seleção
selection:
  method: gumbel_top_k                    # Método de seleção
  temperature: 0.85                       # Temperatura Gumbel (0.1-2.0)
  top_k: 12                              # Top-K candidatos
  seed_strategy: slot_hash                # Estratégia de seed

# Exploração vs Exploitação
exploration:
  epsilon: 0.12                          # 12% de exploração aleatória

# Sistema Autopilot
autopilot:
  enabled: false                         # Ajustes automáticos D+1
  max_daily_variation: 0.05              # Máx 5% variação/dia

# KPIs Principais
kpis:
  primary: ["selection_entropy"]          # Diversidade de seleção
  secondary: ["curator_apply_budget_used_pct"] # Uso do budget curator
```

Este "Cartão Perfurado do Dono" é carregado pelo módulo `BusinessLogic::load_from_file` em `vvtv-core/src/business_logic/mod.rs`, que converte o YAML em tipos Rust com validação rigorosa de bounds e restrições operacionais.

### 3\. Motor de Seleção Determinística

#### 3.1 Algoritmo Gumbel-Top-k

O sistema utiliza o algoritmo **Gumbel-Top-k** para seleção de conteúdo, que combina qualidade (scores de curadoria) com diversidade controlada:

```rust
// Implementação em vvtv-core/src/plan/selection/mod.rs
fn gumbel_topk_indices(scores: &[f64], k: usize, rng: &mut ChaCha20Rng) -> Vec<usize> {
    let gumbel_scores: Vec<(f64, usize)> = scores
        .iter()
        .enumerate()
        .map(|(i, &score)| {
            let gumbel_noise = rng.sample(Gumbel::new(0.0, 1.0).unwrap());
            (score + gumbel_noise, i)
        })
        .collect();
    
    let mut sorted = gumbel_scores;
    sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    sorted.into_iter().take(k).map(|(_, idx)| idx).collect()
}
```

#### 3.2 Geração de Seeds Determinísticas

Cada slot de 15 minutos possui um seed único e reproduzível:

```rust
fn generate_slot_seed_robust(
    now: DateTime<Utc>,
    slot_duration: Duration,
    window_id: u64,
    global_seed: u64,
) -> u64 {
    let slot_start = now.duration_trunc(slot_duration).unwrap();
    let slot_timestamp = slot_start.timestamp() as u64;
    
    let mut hasher = DefaultHasher::new();
    hasher.write_u64(global_seed);
    hasher.write_u64(window_id);
    hasher.write_u64(slot_timestamp);
    hasher.finish()
}
```

#### 3.3 Integração no Planner

O `Planner` aplica a lógica de negócio em cada ciclo:

```rust
let ordered = match method {
    SelectionMethod::GumbelTopK => {
        let temperature = self.business_logic.selection_temperature().max(1e-3);
        let scaled_scores: Vec<f64> = ordered.iter()
            .map(|(_, score, _)| *score / temperature)
            .collect();
        
        let seed = generate_slot_seed_robust(
            now,
            self.business_logic.slot_duration(),
            self.business_logic.window_id(),
            self.business_logic.global_seed(),
        );
        
        let mut rng = ChaCha20Rng::seed_from_u64(seed);
        let indices = gumbel_topk_indices(&scaled_scores, top_k, &mut rng);
        
        // Logs estruturados para auditoria
        tracing::info!(
            target: "planner.selection",
            seed = seed,
            temperature = temperature,
            top_k = top_k,
            indices = ?indices,
            "Gumbel-Top-k selection completed"
        );
        
        indices.into_iter()
            .map(|index| ordered[index].clone())
            .collect::<Vec<_>>()
    }
    _ => {
        // Fallback para seleção simples por score
        let mut copy = ordered.to_vec();
        copy.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        copy.truncate(top_k);
        copy
    }
};
```

### 4\. Integração LLM e Circuit Breakers

#### 4.1 Arquitetura de Resiliência

O sistema LLM opera com circuit breakers para garantir que falhas externas não afetem a operação:

```rust
pub struct LlmHook {
    handler: Arc<dyn LlmHookHandler>,
    allowed_actions: Vec<String>,
    budget_tokens: u32,
    deadline: Duration,
    breaker: CircuitBreaker,
}

impl LlmHook {
    pub async fn invoke(&mut self, request: LlmHookRequest) -> LlmHookOutcome {
        if self.breaker.is_open() {
            return self.fallback("circuit_breaker_open");
        }
        
        let fut = self.handler.handle(request);
        match timeout(self.deadline, fut).await {
            Ok(Ok(outcome)) => {
                self.breaker.record(Utc::now(), true);
                outcome
            }
            Ok(Err(err)) => {
                warn!(target: "llm", hook = ?self.kind, "handler error: {err}");
                self.breaker.record(Utc::now(), false);
                self.fallback("handler_error")
            }
            Err(_) => {
                warn!(target: "llm", hook = ?self.kind, "timeout after {:?}", self.deadline);
                self.breaker.record(Utc::now(), false);
                self.fallback("timeout")
            }
        }
    }
}
```

#### 4.2 Circuit Breaker Implementation

```rust
pub struct CircuitBreaker {
    window_size: usize,
    failure_threshold: f64,
    recent_results: VecDeque<(DateTime<Utc>, bool)>,
    state: CircuitBreakerState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,    // Normal operation
    HalfOpen,  // Testing if service recovered
    Open,      // Blocking requests
}
```

#### 4.3 Consumo no Planner

O LLM é consultado opcionalmente para refinamento:

```rust
// Em Planner::apply_llm
let llm_invocations: Vec<LlmInvocation> = candidates.iter()
    .map(|candidate| LlmInvocation {
        plan_id: candidate.plan_id.clone(),
        score: candidate.score,
        rationale: candidate.rationale.clone(),
        tags: candidate.tags.clone(),
        kind: candidate.kind.clone(),
    })
    .collect();

let rerank_result = self.llm_orchestrator
    .rerank_candidates(llm_invocations)
    .await;

match rerank_result.mode {
    LlmMode::Apply if rerank_result.order.is_some() => {
        // Reordenar candidatos conforme sugestão LLM
        let order = rerank_result.order.unwrap();
        // ... aplicar reordenação determinística
    }
    _ => {
        // Manter ordem original (modo AdviceOnly ou falha)
    }
}
```

### 5\. Curator Vigilante System

#### 5.1 Monitoramento Inteligente

O Curator Vigilante monitora a programação e intervém quando detecta padrões problemáticos:

```rust
pub struct CuratorVigilante {
    config: CuratorVigilanteConfig,
    token_bucket: TokenBucket,
    log_writer: JsonlWriter,
}

impl CuratorVigilante {
    pub fn review(&mut self, candidates: &[SelectedCandidate]) -> CuratorReview {
        let now = Utc::now();
        let signals = self.evaluate_signals(now, candidates);
        
        let triggered = signals.iter().filter(|signal| signal.triggered).count();
        let confidence = if signals.is_empty() {
            0.0
        } else {
            triggered as f64 / signals.len() as f64
        };
        
        let decision = if confidence >= self.config.confidence_threshold 
            && self.token_bucket.try_consume(1) {
            CuratorDecision::Apply
        } else {
            CuratorDecision::Advice
        };
        
        // Log da avaliação
        self.log_evaluation(&signals, confidence, &decision);
        
        CuratorReview {
            decision,
            confidence,
            signals,
            order: self.generate_reorder_if_needed(candidates, &decision),
        }
    }
}
```

#### 5.2 Sinais de Qualidade

O sistema avalia múltiplos sinais estéticos e de diversidade:

```rust
fn evaluate_signals(&self, now: DateTime<Utc>, candidates: &[SelectedCandidate]) -> Vec<CuratorSignal> {
    let mut signals = Vec::new();
    
    // Sinal: Duplicação de tags
    let tag_counts = self.count_tags(candidates);
    let max_tag_count = tag_counts.values().max().unwrap_or(&0);
    if *max_tag_count > 3 {
        signals.push(CuratorSignal {
            name: "tag_duplication".to_string(),
            triggered: true,
            confidence: (*max_tag_count as f64 - 3.0) / 5.0,
            description: format!("Tag repetida {} vezes", max_tag_count),
        });
    }
    
    // Sinal: Baixa diversidade de scores
    let score_variance = self.calculate_score_variance(candidates);
    if score_variance < 0.1 {
        signals.push(CuratorSignal {
            name: "low_score_diversity".to_string(),
            triggered: true,
            confidence: (0.1 - score_variance) / 0.1,
            description: "Scores muito similares".to_string(),
        });
    }
    
    // Sinal: Concentração temporal
    let temporal_clustering = self.detect_temporal_clustering(candidates);
    if temporal_clustering > 0.7 {
        signals.push(CuratorSignal {
            name: "temporal_clustering".to_string(),
            triggered: true,
            confidence: temporal_clustering,
            description: "Conteúdo muito concentrado temporalmente".to_string(),
        });
    }
    
    signals
}
```

#### 5.3 Token Bucket Rate Limiting

```rust
pub struct TokenBucket {
    capacity: u32,
    tokens: u32,
    refill_rate_per_hour: u32,
    last_refill: DateTime<Utc>,
}

impl TokenBucket {
    pub fn try_consume(&mut self, tokens: u32) -> bool {
        self.refill();
        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }
    
    fn refill(&mut self) {
        let now = Utc::now();
        let hours_elapsed = (now - self.last_refill).num_seconds() as f64 / 3600.0;
        let tokens_to_add = (hours_elapsed * self.refill_rate_per_hour as f64) as u32;
        
        if tokens_to_add > 0 {
            self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
            self.last_refill = now;
        }
    }
}
```

### 6\. Programação Adaptativa Engine

#### 6.1 Adaptive Programming Rules

O sistema adapta sua programação baseado em métricas de audiência em tempo real:

```rust
pub fn apply_adaptive_rules(&mut self, metrics: &AudienceMetrics) -> AdaptiveAdjustments {
    let mut adjustments = AdaptiveAdjustments::default();
    
    // Regra: Baixa retenção → aumentar diversidade
    if metrics.retention_30min < 0.6 {
        adjustments.diversity_boost = 0.2;
        adjustments.rationale.push("Low retention detected - increasing diversity".to_string());
    }
    
    // Regra: Alta retenção → manter padrão atual
    if metrics.retention_30min > 0.8 {
        adjustments.temperature_reduction = 0.1;
        adjustments.rationale.push("High retention - reducing exploration".to_string());
    }
    
    // Regra: Pico geográfico → adaptar idioma/cultura
    if let Some(dominant_region) = metrics.dominant_region() {
        match dominant_region.as_str() {
            "BR" | "PT" => {
                adjustments.language_preference = Some("pt".to_string());
                adjustments.cultural_boost = 0.15;
            }
            "US" | "CA" => {
                adjustments.language_preference = Some("en".to_string());
            }
            _ => {}
        }
    }
    
    // Regra: Horário noturno → conteúdo mais calmo
    let hour = Utc::now().hour();
    if hour >= 22 || hour <= 6 {
        adjustments.mood_filter = Some("calm".to_string());
        adjustments.energy_reduction = 0.3;
    }
    
    adjustments
}
```

#### 6.2 Curadoria por Desejo Computável

Cada vídeo possui um `desire_vector` — uma matriz simbólica extraída por IA local:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct DesireVector {
    pub energy: f64,        // 0.0-1.0: calmo → energético
    pub sensuality: f64,    // 0.0-1.0: sutil → explícito
    pub proximity: f64,     // 0.0-1.0: distante → íntimo
    pub warmth: f64,        // 0.0-1.0: frio → quente (cromático)
    pub rhythm: f64,        // 0.0-1.0: lento → rápido (corporal)
    pub presence: f64,      // 0.0-1.0: ausente → presente (auditiva)
}

impl DesireVector {
    pub fn similarity(&self, other: &DesireVector) -> f64 {
        let diff_sum = (self.energy - other.energy).powi(2)
            + (self.sensuality - other.sensuality).powi(2)
            + (self.proximity - other.proximity).powi(2)
            + (self.warmth - other.warmth).powi(2)
            + (self.rhythm - other.rhythm).powi(2)
            + (self.presence - other.presence).powi(2);
        
        1.0 - (diff_sum / 6.0).sqrt()
    }
}
```

O sistema correlaciona os vetores dos vídeos mais assistidos por região e gera **tendências de desejo** semanais que retroalimentam o planner.

### 7\. Estrutura Econômica Computável

#### 7.1 Ledger Econômico Local

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
| `business_logic_version` | TEXT | versão da configuração usada |
| `llm_contribution` | BOOLEAN | se LLM influenciou a decisão |

**Hash de Auditoria:**

```
sha256(timestamp + event_type + context + value_eur + business_logic_version)
```

→ assinado computavelmente com chave do LogLine ID.

#### 7.2 Fontes de Receita Adaptativas

**Exibição Passiva (Baseline):**
*   Cada espectador gera valor baseado em `view_seconds × trust_score × adaptive_rate`.
*   **Adaptive rate:** varia de €0.0008 a €0.0015/minuto baseado em engagement e qualidade da programação.
*   Multiplicador automático via business logic e feedback do Curator.

**Inserções Estéticas Inteligentes:**
*   Micro-interlúdios de 3–6s posicionados pelo algoritmo adaptativo.
*   Frequência controlada por `interstitials_ratio` no business_logic.yaml.
*   LLM sugere timing e estilo baseado no contexto do conteúdo.

**Premium Slots Dinâmicos:**
*   Preços ajustados automaticamente baseados em métricas de audiência.
*   Algoritmo de leilão interno para slots de alta demanda.
*   Contratos `.lll` com SLA de audiência garantida.

#### 7.3 Custos e Equilíbrio Inteligente

| Categoria | Fonte | Custo Base | Custo Adaptativo |
| --- | --- | --- | --- |
| Armazenamento | Railway + B2 | €0.02/h | +20% com LLM ativo |
| Banda CDN | Cloudflare | €0.05/h | Varia com qualidade adaptativa |
| Energia (Lisboa node) | local | €0.01/h | +15% com processamento IA |
| LLM API calls | OpenAI/Anthropic | €0.00/h | €0.03-0.08/h quando ativo |
| Manutenção | manual/logline | €0.03/h | Reduz com automação |

**Custo total adaptativo:** €0.11-0.19/h  
**Receita alvo adaptativa:** €0.28-0.45/h → margem líquida 150-180%.

### 8\. CLI e Operação do Sistema

#### 8.1 Comandos Business Logic

```bash
# Visualizar configuração atual
vvtvctl business-logic show
vvtvctl business-logic show --format json

# Validar configuração
vvtvctl business-logic validate
vvtvctl business-logic validate --file /path/to/new_config.yaml

# Recarregar configuração (hot reload)
vvtvctl business-logic reload
vvtvctl business-logic reload --file /path/to/new_config.yaml

# Testar seleção
vvtvctl business-logic test-selection --plans 20 --dry-run
vvtvctl business-logic test-selection --temperature 0.9 --top-k 15
```

#### 8.2 Comandos Curator System

```bash
# Status do Curator Vigilante
vvtvctl curator status
vvtvctl curator status --format json

# Forçar revisão manual
vvtvctl curator review --confidence-threshold 0.8
vvtvctl curator review --dry-run

# Histórico de intervenções
vvtvctl curator history --last 24h
vvtvctl curator history --export /path/to/report.jsonl

# Token bucket status
vvtvctl curator tokens
vvtvctl curator tokens --refill 3
```

#### 8.3 Comandos LLM Integration

```bash
# Status do circuit breaker
vvtvctl llm status
vvtvctl llm status --detailed

# Testar conectividade
vvtvctl llm test --endpoint https://api.openai.com/v1/chat/completions
vvtvctl llm test --dry-run

# Estatísticas de uso
vvtvctl llm stats --last 7d
vvtvctl llm stats --export /path/to/usage.json
```

### 9\. Métricas e Observabilidade

#### 9.1 KPIs Principais

**Métricas de Seleção:**
- `selection_entropy`: Diversidade da programação (0.0-1.0, alvo >0.7)
- `gumbel_temperature_effective`: Temperatura efetiva aplicada
- `top_k_utilization`: Utilização do espaço de candidatos

**Métricas do Curator:**
- `curator_interventions_per_hour`: Intervenções por hora (alvo <2)
- `curator_confidence_avg`: Confiança média das decisões
- `token_bucket_utilization`: Uso do budget de intervenções

**Métricas LLM:**
- `llm_success_rate`: Taxa de sucesso das chamadas (alvo >95%)
- `llm_latency_p95`: Latência P95 das chamadas (alvo <2s)
- `circuit_breaker_state`: Estado do circuit breaker

**Métricas de Negócio:**
- `revenue_per_hour_adaptive`: Receita adaptativa por hora
- `engagement_score_weighted`: Score de engagement ponderado
- `retention_improvement_rate`: Taxa de melhoria de retenção

#### 9.2 Dashboards e Relatórios

**Dashboard Principal** (`/vvtv/monitor/business_logic_dashboard.html`):
- Gráfico de seleção em tempo real
- Status dos circuit breakers
- Métricas de receita adaptativa
- Heatmap de intervenções do Curator

**Relatórios Automáticos:**
- `business_logic_daily.json`: Resumo diário de operações
- `curator_interventions_weekly.jsonl`: Log semanal de intervenções
- `llm_usage_monthly.json`: Uso mensal de LLM e custos
- `adaptive_performance_quarterly.json`: Performance adaptativa trimestral

### 10\. Fluxo Integrado Completo

#### 10.1 Ciclo de Vida de uma Decisão

1. **Carregamento de Configuração:**
   ```rust
   let business_logic = BusinessLogic::load_from_file("/vvtv/system/business_logic.yaml")?;
   ```

2. **Seleção de Candidatos:**
   ```rust
   let candidates = planner.score_candidates(&plans)?;
   let selected = planner.apply_gumbel_topk(&candidates, &business_logic)?;
   ```

3. **Consulta LLM (Opcional):**
   ```rust
   let llm_result = llm_orchestrator.rerank_candidates(&selected).await?;
   let refined = apply_llm_suggestions(&selected, &llm_result)?;
   ```

4. **Revisão do Curator:**
   ```rust
   let curator_review = curator_vigilante.review(&refined)?;
   let final_selection = apply_curator_adjustments(&refined, &curator_review)?;
   ```

5. **Persistência e Auditoria:**
   ```rust
   plan_store.store_decisions(&final_selection, &business_logic.version())?;
   audit_logger.log_selection_cycle(&final_selection, &metrics)?;
   ```

#### 10.2 Feedback Loop Automático

```rust
// Executado a cada 4 horas
pub async fn adaptive_feedback_cycle(&mut self) -> Result<()> {
    // 1. Coletar métricas de audiência
    let metrics = self.metrics_collector.collect_last_4h().await?;
    
    // 2. Avaliar performance da programação
    let performance = self.evaluate_programming_performance(&metrics)?;
    
    // 3. Gerar ajustes sugeridos
    let adjustments = self.generate_adaptive_adjustments(&performance)?;
    
    // 4. Aplicar ajustes dentro dos limites de segurança
    if adjustments.is_safe() && self.business_logic.autopilot_enabled() {
        self.apply_adjustments(&adjustments).await?;
        self.log_autopilot_action(&adjustments)?;
    }
    
    // 5. Atualizar métricas de feedback
    self.update_feedback_metrics(&performance, &adjustments)?;
    
    Ok(())
}
```

### 11\. Segurança e Compliance

#### 11.1 Auditoria Completa

Todas as decisões do sistema são auditáveis:

```rust
#[derive(Serialize)]
pub struct DecisionAuditLog {
    pub timestamp: DateTime<Utc>,
    pub business_logic_version: String,
    pub selection_seed: u64,
    pub gumbel_temperature: f64,
    pub candidates_count: usize,
    pub llm_consulted: bool,
    pub llm_applied: bool,
    pub curator_intervened: bool,
    pub final_selection: Vec<String>, // plan_ids
    pub rationale: String,
    pub signature: String, // LogLine ID signature
}
```

#### 11.2 Limites de Segurança

O sistema possui limites rígidos para prevenir comportamento errático:

```rust
pub struct SafetyLimits {
    pub max_temperature: f64,           // 2.0
    pub min_temperature: f64,           // 0.1
    pub max_daily_config_changes: u32,  // 3
    pub max_curator_interventions_h: u32, // 5
    pub max_llm_budget_eur_h: f64,      // 0.50
    pub min_selection_entropy: f64,     // 0.3
}
```

#### 11.3 Rollback Automático

Em caso de métricas anômalas, o sistema reverte automaticamente:

```rust
pub async fn emergency_rollback(&mut self, reason: &str) -> Result<()> {
    warn!(target: "business_logic", "Emergency rollback triggered: {}", reason);
    
    // 1. Carregar última configuração estável
    let stable_config = self.load_last_stable_config()?;
    
    // 2. Desativar LLM e Curator temporariamente
    self.disable_ai_systems().await?;
    
    // 3. Aplicar configuração de emergência
    self.apply_emergency_config(&stable_config)?;
    
    // 4. Notificar operadores
    self.send_emergency_notification(reason).await?;
    
    // 5. Log do incidente
    self.log_emergency_rollback(reason, &stable_config)?;
    
    Ok(())
}
```

### 12\. Conclusão do Bloco VII

O **Bloco VII** transforma o VoulezVous.TV em um organismo econômico e inteligente que:

- **Adapta-se continuamente** através de feedback loops e IA
- **Mantém estabilidade** através de limites de segurança e circuit breakers
- **Gera valor** através de monetização adaptativa e otimização automática
- **Permanece auditável** através de logs estruturados e assinaturas criptográficas
- **Evolui autonomamente** dentro de parâmetros seguros definidos pelo operador

A combinação de determinismo Rust com inteligência adaptativa cria um sistema que é simultaneamente previsível e evolutivo, capaz de operar 24/7 enquanto melhora continuamente sua performance econômica e estética.

Cada decisão é rastreável, cada ajuste é justificado, e cada evolução é controlada — criando uma televisão verdadeiramente inteligente que paga suas próprias contas e recompensa o público pela atenção.

* * *🧠
 VVTV INDUSTRIAL DOSSIER
==========================

**Bloco II — Browser Automation & Human Simulation Engineering**
----------------------------------------------------------------

_(sem APIs; play-before-download; aparência humana realista; integração LLM para hints de curadoria)_

* * *

### 0\. Objetivo

Projetar e padronizar a **camada de navegação autônoma inteligente** que:

1.  encontra vídeos/músicas na web com hints opcionais de LLM,
2.  **dá play antes de baixar** (para garantir a mesma rendition HD que o player está tocando),
3.  extrai o alvo real do mídia (manifest/segmento/progressivo),
4.  salva **apenas plano** até a janela T-4h,
5.  opera com **simulação humana** robusta (sem APIs formais, sem endpoints),
6.  integra-se com o sistema de business logic para curadoria adaptativa.

* * *

1) Stack e Processo de Execução Híbrido
---------------------------------------

**Engine:** Chromium (>= 118) via DevTools Protocol (CDP).  
**Controle:** Rust + `chromiumoxide` ou `headless_chrome` (alternativa: Playwright via `playwright-rust`).  
**Execução:** headless por padrão; "headed" para QA.  
**LLM Integration:** Circuit breakers para hints de curadoria e detecção de conteúdo.

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
*   **LLM budget**: máximo €0.05/hora para hints de curadoria.

**Ciclo do worker híbrido:**

```
init_profile → load_business_logic → open_start_url → simulate_human_idle(3–8s) → 
search(term + llm_hints) → scroll_collect(results ~ N) → open_candidate → 
llm_content_analysis(optional) → play_before_download() → capture_target() → 
record_plan(with_llm_metadata) → close_tab → next
```

* * *

2) Fingerprinting & Disfarce Adaptativo
---------------------------------------

**User-Agent Pool (rotativo com IA):**

*   Safari-like (Mac) e Chrome-stable (Win/Mac).
*   Alternar a cada 6–12 páginas.
*   LLM pode sugerir padrões de rotação baseados em detecção de bloqueio.

**Navigator Patches (JS injetado no `document_start`):**

```js
Object.defineProperty(navigator, 'webdriver', { get: () => false });
Object.defineProperty(Notification, 'permission', { get: () => 'default' });
window.chrome = { runtime: {} };
const origQuery = window.navigator.permissions.query;
window.navigator.permissions.query = (p)=>p.name==='notifications'
  ? Promise.resolve({ state:'prompt' })
  : origQuery(p);

// LLM-enhanced fingerprint randomization
if (window.vvtv_llm_hints && window.vvtv_llm_hints.fingerprint_strategy) {
    // Apply LLM-suggested fingerprint modifications
    const strategy = window.vvtv_llm_hints.fingerprint_strategy;
    if (strategy.canvas_noise) {
        // Inject canvas fingerprint noise
    }
    if (strategy.webgl_params) {
        // Modify WebGL parameters
    }
}
```

**Viewport aleatório (dentro de ranges "humanos"):**

*   1366×768, 1440×900, 1536×864, 1920×1080 (± 0–16px jitter).
*   `deviceScaleFactor` ∈ \[1.0, 2.0\].
*   LLM pode sugerir viewports específicos para diferentes tipos de site.

**Input realista com padrões adaptativos:**

*   Mouse path em **Bezier** com velocidade variável (seção 4).
*   Teclado com cadência 140–220 cpm, jitter 15–35 ms/char, erro a cada 80–130 chars.
*   Padrões de movimento podem ser ajustados baseados em feedback do Curator Vigilante.

**Rede:**

*   Proxy pool (residenciais/rotativos).
*   IP "fresco" a cada 20–40 páginas ou quando detectar bloqueio.
*   Circuit breaker para rotação automática em caso de detecção.

**Cookies/Storage:**

*   Perfil persistente por 24 h (para parecer retorno).
*   Limpeza seletiva por domínio "sensível".
*   LLM pode sugerir estratégias de limpeza baseadas no comportamento do site.

* * *

3) Play-Before-Download (PBD) com Análise Inteligente
-----------------------------------------------------

**Princípio:** só baixar **após** o player estar reproduzindo a **rendition** desejada (HD). O que for baixado deve ser **bit-exato** ao que o humano está vendo. LLM pode fornecer hints sobre qualidade e conteúdo.

**Fluxo Geral Híbrido:**

1.  Abrir página de vídeo.
2.  **Análise LLM opcional** do conteúdo da página (título, descrição, thumbnails).
3.  **Tornar visível** o elemento `<video>`/player (scroll, foco).
4.  **Click Play** como humano; aguardar `readyState >= 3`.
5.  **Forçar HD** (UI: clicar engrenagem → 1080p/720p; ou via teclado, se existir).
6.  Esperar **5–12 s** de playback para garantir troca de rendition/adaptive bitrate.
7.  **Análise de qualidade LLM** (opcional): verificar se o conteúdo corresponde aos critérios de curadoria.
8.  **Capturar alvo real**:
    *   **HLS**: capturar `master.m3u8` via **Network.observe**; escolher a variant com `BANDWIDTH` e `RESOLUTION` maiores; baixar **media playlist** vigente.
    *   **DASH**: capturar `manifest.mpd`; escolher `AdaptationSet`/`Representation` com maior `height`.
    *   **Progressivo**: capturar `media.mp4` do `<source>` ou do request principal.
9.  Registrar **plan** (sem baixar) — `url_manifest`, `rendition`, `duration_est`, `title`, `tags`, `llm_analysis`.
10. Fechar aba.

**LLM Content Analysis (Opcional):**

```rust
pub async fn analyze_content_quality(
    &self,
    page_content: &PageContent,
    video_metadata: &VideoMetadata,
) -> Result<ContentAnalysis> {
    if !self.llm_enabled || self.circuit_breaker.is_open() {
        return Ok(ContentAnalysis::default());
    }
    
    let request = LlmRequest {
        prompt: format!(
            "Analyze this video content for curation:\nTitle: {}\nDescription: {}\nDuration: {}s\nTags: {:?}\n\nRate aesthetic quality (0-1), content appropriateness (0-1), and suggest tags.",
            video_metadata.title,
            video_metadata.description,
            video_metadata.duration_s,
            video_metadata.tags
        ),
        max_tokens: 150,
        temperature: 0.3,
    };
    
    match timeout(Duration::from_secs(3), self.llm_client.analyze(request)).await {
        Ok(Ok(analysis)) => {
            self.circuit_breaker.record_success();
            Ok(analysis)
        }
        Ok(Err(e)) => {
            self.circuit_breaker.record_failure();
            warn!("LLM analysis failed: {}", e);
            Ok(ContentAnalysis::default())
        }
        Err(_) => {
            self.circuit_breaker.record_failure();
            warn!("LLM analysis timeout");
            Ok(ContentAnalysis::default())
        }
    }
}
```

**Heurística de seleção HD com IA:**

*   HLS: pick `RESOLUTION >= 1080p` se `BANDWIDTH` >= 4500 kbps; senão 720p ≥ 2500 kbps.
*   DASH: maior `height`, codec `avc1`/`h264` preferido.
*   Progressivo: priorizar `itag`/`qualityLabel` quando exposto.
*   LLM pode sugerir ajustes baseados no tipo de conteúdo detectado.

**Validações mínimas (durante PBD):**

*   `currentTime` cresce estável.
*   `videoWidth/Height` bate com a rendition escolhida.
*   Buffer ahead ≥ 3 s.
*   Análise LLM (se ativa) não detecta conteúdo inadequado.

### Discovery Loop Inteligente

**Objetivo:** manter descoberta contínua sem supervisão humana, convertendo resultados em PLANs auditáveis com hints de IA.

**Componentes principais:**

1.  **ContentSearcher** — motores Google/Bing/DuckDuckGo com scraping JS e heurísticas (schema.org `VideoObject`, duração mínima, indicadores "creative commons").
2.  **LLM Query Enhancer** — melhora queries de busca baseadas em tendências e feedback.
3.  **DiscoveryLoop** — automatiza `search → rate limit → abrir candidato → LLM analysis → Play-Before-Download → registrar plan`.
4.  **SqlitePlanStore** — operando com PRAGMAs (WAL, cache, mmap) e origin tag `discovery-loop`, incluindo metadados LLM.
5.  **CLI operacional** — `vvtvctl discover --query "creative commons documentary" --llm-enhance --max-plans 10 --dry-run`.

**Configuração (`browser.toml`):**

```toml
[discovery]
search_engine = "google"
search_delay_ms = [2000, 5000]
scroll_iterations = 3
max_results_per_search = 20
candidate_delay_ms = [8000, 15000]
filter_domains = ["youtube.com", "vimeo.com", "dailymotion.com"]

[llm_integration]
enabled = true
content_analysis = true
query_enhancement = true
max_budget_eur_per_hour = 0.05
circuit_breaker_threshold = 0.1
timeout_seconds = 3

[adaptive_patterns]
learn_from_curator = true
adjust_timing_based_on_success = true
rotate_strategies_on_failure = true
```

**LLM Query Enhancement:**

```rust
pub async fn enhance_search_query(&self, base_query: &str, context: &SearchContext) -> Result<String> {
    if !self.config.llm_integration.enabled {
        return Ok(base_query.to_string());
    }
    
    let enhancement_request = LlmRequest {
        prompt: format!(
            "Enhance this search query for finding high-quality creative content:\nBase query: '{}'\nContext: Recent successful finds were about {}\nCurrent time: {} (consider seasonal/temporal relevance)\nSuggest 2-3 enhanced queries that would find similar high-quality content.",
            base_query,
            context.recent_successful_tags.join(", "),
            context.current_time.format("%Y-%m-%d %H:%M")
        ),
        max_tokens: 100,
        temperature: 0.7,
    };
    
    match self.llm_client.enhance_query(enhancement_request).await {
        Ok(enhanced) => {
            info!(target: "discovery", "Query enhanced: '{}' -> '{}'", base_query, enhanced);
            Ok(enhanced)
        }
        Err(e) => {
            warn!(target: "discovery", "Query enhancement failed: {}", e);
            Ok(base_query.to_string())
        }
    }
}
```

### Resiliência antibot com IA

- `vvtv-core/src/browser/fingerprint.rs` injeta ruído Canvas/WebGL/Audio antes de cada navegação.
- `browser/retry.rs` + `ip_rotator.rs` categorizam erros, aplicam backoff e registram rotações de proxy.
- `browser/llm_antibot.rs` usa LLM para detectar padrões de bloqueio e sugerir contramedidas.
- `configs/browser.toml` expõe chaves `[fingerprint]`, `[proxy]`, `[retry]`, `[llm_antibot]` para tunning por ambiente.

**LLM Antibot Detection:**

```rust
pub async fn detect_antibot_patterns(&self, page_content: &str, response_headers: &HeaderMap) -> AntibotAnalysis {
    let indicators = vec![
        page_content.contains("captcha"),
        page_content.contains("blocked"),
        page_content.contains("suspicious activity"),
        response_headers.get("cf-ray").is_some(),
        response_headers.get("x-rate-limit").is_some(),
    ];
    
    if indicators.iter().filter(|&&x| x).count() >= 2 {
        if let Ok(llm_analysis) = self.analyze_blocking_pattern(page_content).await {
            return AntibotAnalysis {
                blocked: true,
                confidence: llm_analysis.confidence,
                suggested_action: llm_analysis.suggested_action,
                wait_time: llm_analysis.suggested_wait_time,
            };
        }
    }
    
    AntibotAnalysis::default()
}
```

### QA & Observabilidade Inteligente

- `vvtv-core/src/monitor.rs` grava métricas (`MetricsStore`) incluindo performance LLM e gera dashboards HTML (`DashboardGenerator`).
- `vvtvctl qa smoke-test|report` executa roteiros headless/headed; dados armazenados em `metrics.sqlite`.
- `docs/qa/nightly-smoke.md` descreve checklist noturno, coleta de evidências e mitigação de falhas.
- Métricas LLM: success rate, latency, cost per hour, circuit breaker state.

**Enhanced Metrics:**

```sql
CREATE TABLE browser_llm_metrics (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    llm_calls_total INTEGER,
    llm_calls_successful INTEGER,
    llm_avg_latency_ms REAL,
    llm_cost_eur_hour REAL,
    circuit_breaker_state TEXT,
    content_analysis_accuracy REAL,
    query_enhancement_success_rate REAL,
    antibot_detection_rate REAL
);
```

### Otimizações de performance com IA

- `vvtv-core/src/processor/mod.rs` ativa VideoToolbox automaticamente em Apple Silicon (`VVTV_FORCE_APPLE_SILICON`).
- `vvtv-core/src/sqlite.rs` e `sql/*.sql` inicializam WAL + PRAGMAs (`cache_size`, `mmap_size`, `busy_timeout`).
- `scripts/optimize_databases.sh` automatiza `wal_checkpoint`, `PRAGMA optimize`, `VACUUM`, `ANALYZE`.
- `vvtvctl completions <shell>` gera autocompletar para operadores/CI.
- LLM caching para reduzir custos: respostas similares são cached por 1h.

* * *

4) Simulação Humana (Biomecânica Adaptativa)
--------------------------------------------

### 4.1 Mouse com Padrões Adaptativos

**Modelo:** curvas **cúbicas de Bézier** com ruído per-ponto e padrões que se adaptam baseados no feedback do Curator.

*   Velocidade média: 500–1200 px/s.
*   Micro-oscilações laterais (±1–3 px) a cada 12–25 ms.
*   "Hesitação" antes do clique: pausa 120–450 ms.
*   **Adaptação**: padrões de movimento ajustados baseados em taxa de sucesso por site.

**Pseudo (Rust-ish) com adaptação:**

```rust
fn move_mouse_adaptive(from: Point, to: Point, dur_ms: u32, site_context: &SiteContext) -> Result<()> {
    let base_cps = pick_control_points(from, to);
    
    // Adapt control points based on site success rate
    let adapted_cps = if site_context.success_rate < 0.7 {
        // More human-like movement for difficult sites
        add_extra_curves(base_cps, site_context.difficulty_level)
    } else {
        base_cps
    };
    
    let steps = dur_ms / 12;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let p = cubic_bezier(from, adapted_cps.0, adapted_cps.1, to, t);
        
        // Adaptive jitter based on site antibot sensitivity
        let jitter_intensity = site_context.antibot_sensitivity * 0.8;
        let jitter = randn2d(0.0, jitter_intensity);
        
        send_mouse_move(p.x + jitter.x, p.y + jitter.y);
        sleep_ms(12);
    }
    
    Ok(())
}
```

**Click Adaptativo:**

*   `mousedown` → 30–70 ms → `mouseup`.
*   Botão esquerdo 98%, direito 2% (raras inspeções).
*   Timing adaptado baseado na responsividade detectada do site.

### 4.2 Scroll Inteligente

*   Página: "rajadas" de 200–800 px; pausa 120–300 ms entre rajadas.
*   Próximo ao player: scroll **lento** (80–140 px) com pausas maiores (200–500 ms).
*   Anti-padrão: sempre dar **duas** micro rolagens residuais antes do play.
*   **LLM hint**: pode sugerir padrões de scroll específicos para diferentes tipos de layout.

### 4.3 Teclado com Correção Adaptativa

*   Cadência 140–220 cpm; jitter 15–35 ms/char.
*   Erro intencional a cada 80–130 chars → backspace → correção.
*   Hotkeys toleradas: `Space` (play/pause), `ArrowLeft/Right` (seek curto), **não usar** `F12`.
*   **Adaptação**: cadência ajustada baseada na responsividade do site.

### 4.4 Ociosidade & Multitarefa Inteligente

*   Ociosidade ocasional: 1,5–4,5 s.
*   Troca de abas "falsas" (abrir resultados paralelos) 1 a cada 5–8 páginas.
*   Pequenas movimentações "sem propósito" a cada 20–35 s (efeito atenção dispersa).
*   **LLM guidance**: pode sugerir padrões de comportamento específicos para diferentes tipos de site.

### 4.5 Probabilidade de erro simulada com aprendizado

*   Clique em área vazia: 1–2% das vezes.
*   Scroll overshoot: 5–8%.
*   Segunda tentativa de play: 10–15% (players que não respondem ao primeiro clique).
*   **Adaptação**: taxas de erro ajustadas baseadas no feedback de sucesso por domínio.

* * *

5) Coleta & Normalização de Metadados (sem API, com IA)
------------------------------------------------------

**Extração DOM (JS) com análise LLM:**

*   `document.title` (fallback `<meta property="og:title">`).
*   `video.duration` quando acessível; senão, estimativa por playback (10–20 s).
*   `textContent` de `<h1>`, `<h2>`, breadcrumbs.
*   Tags/categorias via seletores comuns (chips, anchors com `/tag/`).
*   Resolução via `video.videoWidth/Height` ou label UI ("1080p/720p").
*   **LLM enhancement**: análise de conteúdo para extração de tags semânticas e classificação de mood.

**Sanitização com IA:**

*   Remover emojis, múltiplos espaços, `\n`, tracking params (`utm_*`, `ref`).
*   Normalizar idioma para en-US/pt-PT when needed (título duplicado → manter original).
*   **LLM processing**: limpeza inteligente de títulos, detecção de spam, normalização de tags.

**Registro de PLAN (SQLite) com metadados LLM:**

```sql
CREATE TABLE plans (
    plan_id TEXT PRIMARY KEY,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    kind TEXT NOT NULL,
    title TEXT,
    source_url TEXT,
    resolution_observed TEXT,
    curation_score REAL DEFAULT 0.5,
    duration_est_s INTEGER,
    expected_bitrate INTEGER,
    status TEXT DEFAULT 'planned',
    license_proof TEXT,
    hd_missing BOOLEAN DEFAULT 0,
    node_origin TEXT,
    updated_at DATETIME,
    -- LLM Integration Fields
    llm_analyzed BOOLEAN DEFAULT 0,
    llm_aesthetic_score REAL,
    llm_content_tags TEXT, -- JSON array
    llm_mood_classification TEXT,
    llm_quality_assessment TEXT,
    llm_rationale TEXT,
    selection_seed INTEGER,
    business_logic_version TEXT
);
```

* * *

6) Seletores & Estratégias de Player com IA
-------------------------------------------

**Detecção do alvo com aprendizado:**

*   `<video>` direto? Usar.
*   Player frameworks comuns:
    *   **video.js** → `.vjs-tech` (source em `<video>`).
    *   **hls.js** → observar `Network` por `.m3u8`.
    *   **dash.js/shaka** → `.mpd` requests.
    *   **iframes** → focar dentro do frame; repetir heurística.
*   **LLM assistance**: pode sugerir seletores específicos para sites não reconhecidos.

**Botões críticos (seletores aproximados com adaptação):**

*   Play: `.play, .vjs-play-control, button[aria-label*="Play"]`
*   Qualidade: `.quality, .settings, .vjs-menu-button`
*   Maximize/Mute: `.fullscreen, .mute`
*   **Adaptive selectors**: aprendizado de novos seletores baseado em sucesso/falha.

**Pop-ups/consent com IA:**

*   Detectar overlays com `position:fixed`/z-index alto → clicar "accept/close" por árvore de botões prováveis.
*   **LLM analysis**: pode analisar texto de pop-ups para determinar a melhor ação.

**Fallbacks inteligentes:**

*   Se nenhum seletor reagir:
    1.  `Space` (teclado).
    2.  Click no centro do player (50% width/height).
    3.  **LLM suggestion**: análise da página para sugerir estratégias alternativas.
    4.  Recarregar a página 1x.

* * *

7) Captura da Fonte Real do Vídeo (sem API, com validação IA)
------------------------------------------------------------

### 7.1 Via DevTools Protocol (preferencial) com análise

*   Ativar `Network.enable`.
*   Filtrar requests por `m3u8|mpd|.mp4|.webm`.
*   Para **HLS**:
    *   guardar `master.m3u8`, resolver **variant** correta por resolução/bitrate,
    *   capturar **media playlist** atual (onde o player migrou) → **URL final do plano**.
*   Para **DASH**:
    *   parse do MPD; preferir maior `height`/`bandwidth`.
*   Para **progressivo**:
    *   URL do `GET` com `Content-Type: video/*`, `Content-Length` razoável.
*   **LLM validation**: verificar se a URL capturada corresponde ao conteúdo esperado.

### 7.2 Via Proxy (sites anti-devtools) com detecção inteligente

*   Executar navegador com proxy local (mitm).
*   Extrair manifests das conexões TLS de vídeo (mitm com domínio permitido).
*   Persistir somente a URL final; **não baixar** no momento do plano.
*   **LLM analysis**: detectar padrões de anti-devtools e sugerir contramedidas.

**Critérios de aceitação da captura com IA:**

*   Reproduzindo há ≥ 5 s **após** mudar qualidade para HD.
*   Taxa de buffer estável.
*   Nenhum erro do player nos últimos 3 s.
*   **LLM validation**: análise de qualidade e adequação do conteúdo.

* * *

8) Plano de Erros & Recuperação Inteligente
-------------------------------------------

**Categorias com IA:**

*   _Não encontrou player_: tentar 3 layouts; LLM pode sugerir seletores alternativos; cair para próximo candidato.
*   _Play não inicia_: clicar 2–3x; espaço; LLM pode analisar a página para detectar bloqueios; reload 1x.
*   _HD indisponível_: aceitar 720p; marcar flag `hd_missing`; LLM pode sugerir estratégias alternativas.
*   _Bloqueio/antibot_: trocar IP/proxy; alternar UA; LLM pode analisar padrões de bloqueio; dormir 5–15 min.
*   _Manifest inconsistente_: repetir coleta; LLM pode validar consistência; se falhar, descartar plano.

**Regras de backoff adaptativas:**

*   1ª falha do domínio: retry em 10–20 min.
*   2ª: retry 45–90 min.
*   3ª: blacklist 24 h.
*   **LLM learning**: padrões de falha são analisados para melhorar estratégias futuras.

**Error Analysis com LLM:**

```rust
pub async fn analyze_failure_pattern(&self, failures: &[BrowserFailure]) -> FailureAnalysis {
    if failures.len() < 3 || !self.llm_enabled {
        return FailureAnalysis::default();
    }
    
    let failure_summary = failures.iter()
        .map(|f| format!("Domain: {}, Error: {}, Time: {}", f.domain, f.error_type, f.timestamp))
        .collect::<Vec<_>>()
        .join("\n");
    
    let analysis_request = LlmRequest {
        prompt: format!(
            "Analyze these browser automation failures and suggest improvements:\n{}\n\nIdentify patterns and suggest specific countermeasures.",
            failure_summary
        ),
        max_tokens: 200,
        temperature: 0.3,
    };
    
    match self.llm_client.analyze_failures(analysis_request).await {
        Ok(analysis) => analysis,
        Err(e) => {
            warn!("LLM failure analysis failed: {}", e);
            FailureAnalysis::default()
        }
    }
}
```

* * *

9) Qualidade Visual "Humana" com Adaptação IA
---------------------------------------------

*   **Cursor sempre visível** em modo QA; oculto em headless.
*   **Scroll elástico**: última rolagem sempre menor que a penúltima.
*   **Dwell-time** em thumbnails: 400–900 ms antes de abrir.
*   **Movimento "okulomotor"**: pequeno "8" com amplitude 6–10 px perto de elementos clicáveis (sugere leitura).
*   **Padrão noturno**: iniciar ciclos intensos às 02:00–06:00 locais.
*   **LLM guidance**: padrões de movimento podem ser ajustados baseados em análise de sucesso por tipo de site.

> Detalhe obsessivo solicitado: **cor da unha** do operador: _grafite fosco_.  
> No QA headed, plano de fundo do cursor deve ser neutro para evitar reflexo na inspeção visual do movimento.
> LLM pode sugerir ajustes de cor baseados em diferentes ambientes de teste.

* * *

10) Segurança, Privacidade, Conformidade com IA
-----------------------------------------------

*   **Áudio mudo** sempre.
*   **Sem formular senhas**.
*   **Sem uploads**.
*   **Consentimento/Idade**: só aceitar fontes com política explícita; registrar no plano `license_hint`; LLM pode analisar termos de uso.
*   **Isolamento**: perfis por domínio; storage quotas.
*   **Atualizações**: engine travada em versão testada (rolling update semanal, nunca em horário de pico).
*   **LLM Privacy**: nenhum PII enviado para serviços externos; apenas metadados de conteúdo.
*   **Data Retention**: logs LLM mantidos por 7 dias; análises agregadas por 30 dias.

* * *

11) Métricas locais (sem spans, sem rede, com IA)
------------------------------------------------

_(opcional, para tuning off-line — gravadas em `metrics.sqlite`)_

*   `pages_per_hour`, `videos_seen`, `plans_created`
*   `pbd_success_rate`, `hd_hit_rate`, `avg_play_wait_s`
*   `antibot_incidents`, `proxy_rotations`
*   **LLM metrics**: `llm_calls_per_hour`, `llm_success_rate`, `llm_cost_eur_per_hour`
*   **Adaptive metrics**: `pattern_learning_accuracy`, `site_adaptation_success_rate`

Coleta a cada 10 min; retém 14 dias; sem PII.

**Enhanced Metrics Schema:**

```sql
CREATE TABLE browser_metrics_enhanced (
    ts DATETIME DEFAULT CURRENT_TIMESTAMP,
    pages_per_hour INTEGER,
    videos_seen INTEGER,
    plans_created INTEGER,
    pbd_success_rate REAL,
    hd_hit_rate REAL,
    avg_play_wait_s REAL,
    antibot_incidents INTEGER,
    proxy_rotations INTEGER,
    -- LLM Integration Metrics
    llm_calls_per_hour INTEGER,
    llm_success_rate REAL,
    llm_avg_latency_ms REAL,
    llm_cost_eur_per_hour REAL,
    circuit_breaker_state TEXT,
    -- Adaptive Learning Metrics
    pattern_learning_accuracy REAL,
    site_adaptation_success_rate REAL,
    adaptive_selector_discoveries INTEGER,
    failure_pattern_recognitions INTEGER
);
```

* * *

12) Testes & QA com IA
---------------------

**Smoke (por domínio) com validação LLM:**

*   Encontrar player em ≤ 6 s.
*   Abrir qualidade e selecionar ≥ 720p.
*   Play estável ≥ 8 s.
*   Capturar URL final válida (200 OK no HEAD).
*   **LLM validation**: verificar qualidade e adequação do conteúdo.
*   Criar **PLAN** com `status=planned` e metadados LLM.

**Load (noturno) com análise adaptativa:**

*   2 abas por nó, 2 nós → ≥ 80 planos/h.
*   CPU ≤ 60%, RAM ≤ 2.5 GB/instância.
*   **LLM budget**: ≤ €0.05/h por nó.

**Anti-bot com aprendizado:**

*   Trocar IP; novo UA; novo viewport → player ainda reproduz?
*   Falhou 3x seguidas? LLM analisa padrão → sugere estratégia → blacklist 24 h.
*   **Pattern learning**: sucessos e falhas alimentam modelo de adaptação.

**Qualidade do movimento com feedback:**

*   Distância média por clique 200–900 px.
*   Erro propositado 1–2% cliques.
*   Dwell médio em cards 600 ms ± 200.
*   **Adaptive tuning**: parâmetros ajustados baseados em taxa de sucesso.

* * *

13) Pseudocódigo Integrador (Rust-like) com IA
----------------------------------------------

```rust
async fn collect_plan_with_ai(url: &str, llm_client: &LlmClient) -> Option<Plan> {
    let mut c = Browser::spawn(profile());
    c.goto(url)?;
    human::idle(ms(2000..6000));

    // Optional LLM content pre-analysis
    let page_content = c.get_page_content()?;
    let llm_hints = if llm_client.is_available() {
        llm_client.analyze_page_content(&page_content).await.ok()
    } else {
        None
    };

    let player = find_player_adaptive(&c, &llm_hints)?;
    human::move_to(&player.center());
    human::click();

    wait::until_video_ready(&c, secs(3..8))?;
    player.open_quality_menu()?;
    player.select_hd_or_720p()?;
    wait::steady_playback(&c, secs(5..12))?;

    let media = capture_media_target(&c)?; // m3u8/mpd/mp4 via CDP/proxy
    let meta = read_basic_meta(&c, &player)?;
    
    // Optional LLM content analysis
    let llm_analysis = if let Some(ref hints) = llm_hints {
        llm_client.analyze_video_quality(&meta, &media).await.ok()
    } else {
        None
    };

    Some(Plan {
        plan_id: uuid(),
        title: meta.title,
        url: media.url,
        kind: meta.kind,
        duration_s: meta.duration_est,
        resolution: media.resolution,
        curation_score: calculate_score(&meta, &llm_analysis),
        status: "planned",
        llm_analyzed: llm_analysis.is_some(),
        llm_aesthetic_score: llm_analysis.as_ref().map(|a| a.aesthetic_score),
        llm_content_tags: llm_analysis.as_ref().map(|a| serde_json::to_string(&a.tags).ok()).flatten(),
        llm_mood_classification: llm_analysis.as_ref().map(|a| a.mood.clone()),
        llm_quality_assessment: llm_analysis.as_ref().map(|a| a.quality_assessment.clone()),
        llm_rationale: llm_analysis.as_ref().map(|a| a.rationale.clone()),
    })
}
```

* * *

14) Entregáveis deste Bloco
---------------------------

*   **Especificação operacional** (este documento) com integração LLM completa.
*   **Templates** de seletores por player comum com aprendizado adaptativo.
*   **Implementação** do motor de movimento (Bezier + jitter) com padrões adaptativos.
*   **Capturador** CDP + Proxy fallback com validação LLM.
*   **Normalizador** de metadados DOM com análise semântica IA.
*   **LLM Integration Layer** com circuit breakers e cost control.
*   **Adaptive Learning System** para melhoria contínua de padrões.
*   **Test Kit** de QA (scripts de smoke/load) com validação IA.

* * *

15) Ready-for-Build Checklist
-----------------------------

*    Chromium com flags aprovadas.
*    Controller Rust compilado com integração LLM.
*    Proxy MITM funcional (fallback).
*    Heurísticas de player testadas (video.js / hls.js / dash.js) com adaptação.
*    Movimento humano com Bezier e jitter ativo e adaptativo.
*    Play-before-download confirmando HD/720p com validação LLM.
*    PLAN gravado sem baixar nada, incluindo metadados IA.
*    LLM integration com circuit breakers e budget control ativo.
*    Adaptive learning system funcionando.
*    Limpeza de perfil e quotas validadas.
*    Métricas locais ligadas (opcional) incluindo métricas LLM.
*    QA noturno executado e aprovado com validação IA.

* * *🧠
 VVTV INDUSTRIAL DOSSIER
==========================

**Bloco III — Processor & Media Engineering**
---------------------------------------------

_(T-4h executor; play-before-download real; captura bit-exata; transcode/normalização; packaging; integridade; staging para exibição 24/7)_

* * *

### 0\. Objetivo deste bloco

Padronizar **toda a fase T-4h**: transformar **PLANOS** em **mídia pronta** para a fila de transmissão.  
Inclui: reabrir a página, **dar play antes de baixar** (para capturar a **mesma rendition HD** que o player está tocando), baixar/compilar mídia, normalizar áudio, transcodificar/empacotar nos perfis operacionais, validar integridade e **entregar ao playout**.

O sistema integra-se com o business logic para aplicar configurações de qualidade adaptativas e utiliza feedback do Curator Vigilante para ajustes de processamento.

* * *

1) Posição no ciclo e gatilhos
------------------------------

**Entrada:** linhas `plans` com `status='selected'` (escolhidos pelo Realizer quando `time_to_slot <= 240 min`).  
**Saída:** artefatos em `/vvtv/storage/ready/<plan_id>/` e registro na `playout_queue` com `status='queued'`.

**Gatilhos do Processor:**

*   Timer de orquestração a cada 2–5 min.
*   Lote máximo por execução: **N=6** itens.
*   Concurrency: **2** downloads + **2** transcodes simultâneos por nó (cap CPU ≤ 75%).
*   **Business Logic Integration**: parâmetros de qualidade ajustados dinamicamente.

* * *

2) Reabertura e Play-Before-Download (PBD) no T-4h
--------------------------------------------------

Mesmo que o PLAN tenha URL de manifesto, **reabra a página** e **dê play** para confirmar a rendition.  
Nada de API. Tudo via navegador com validação opcional por LLM.

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
    *   **LLM validation** (opcional): verificar se conteúdo corresponde ao esperado.
7.  **Fechar a aba** (manter apenas o alvo de mídia).
8.  Proceder ao **download**.

> Regra: **O que baixamos é o que o humano estaria vendo naquele instante.** Se a rendition cair de 1080p para 720p por instabilidade, o PBD repete a tentativa por até 2 ciclos antes de aceitar 720p.

**PBD com Business Logic:**

```rust
pub async fn execute_pbd_with_business_logic(
    &self,
    plan: &Plan,
    business_logic: &BusinessLogic,
) -> Result<MediaCapture> {
    let quality_target = business_logic.get_quality_target_for_plan(plan);
    let retry_strategy = business_logic.get_retry_strategy();
    
    for attempt in 1..=retry_strategy.max_attempts {
        match self.attempt_pbd(plan, &quality_target).await {
            Ok(capture) => {
                if self.validate_capture_quality(&capture, &quality_target)? {
                    return Ok(capture);
                }
            }
            Err(e) => {
                warn!(
                    target: "processor.pbd",
                    plan_id = %plan.plan_id,
                    attempt = attempt,
                    error = %e,
                    "PBD attempt failed"
                );
            }
        }
        
        if attempt < retry_strategy.max_attempts {
            tokio::time::sleep(retry_strategy.backoff_duration(attempt)).await;
        }
    }
    
    Err(ProcessorError::PbdFailed {
        plan_id: plan.plan_id.clone(),
        attempts: retry_strategy.max_attempts,
    })
}
```

* * *

3) Download — HLS/DASH/Progressivo
----------------------------------

### 3.1 Estrutura de staging

```
/vvtv/cache/tmp_downloads/<plan_id>/
  ├── source/            # bruto: .m3u8/.mpd + segments ou .mp4 progressivo
  ├── remux/             # MP4 remuxado (sem reencode) se compatível
  ├── logs/              # logs de processamento
  └── business_logic/    # metadados de configuração aplicada
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

**Comando base (fetch + rewrite) com business logic:**

```bash
# exemplo: usar aria2c para segmentos + script de rewrite
aria2c -j8 -x8 -s16 -d "/vvtv/cache/tmp_downloads/<plan_id>/source" \
  --max-download-limit=${BL_BANDWIDTH_LIMIT} \
  --retry-wait=${BL_RETRY_WAIT} \
  -i segments.txt
# segments.txt contém todas as URLs absolutas da media playlist (+ a própria .m3u8)
```

Reescrever a playlist para apontar para `seg_<nnnn>.*` locais com metadados de business logic.

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

### 3.5 Verificações de integridade com business logic

*   `sha256sum` do conjunto (manifest + lista de arquivos).
*   Amostra de `ffprobe` (tempo, streams, codecs).
*   Duração mínima: vídeo ≥ 60 s; música ≥ 90 s (ajustável por business logic).
*   **Quality validation**: VMAF/SSIM thresholds definidos no business_logic.yaml.

**Falhas & backoff adaptativo**

*   1ª falha: retry em 3 min;
*   2ª: 15 min;
*   3ª: plano `rejected` (log motivo).
*   **Business logic**: backoff times ajustados baseados na prioridade do conteúdo.

* * *

4) Remux vs Transcode — decisão de custo inteligente
---------------------------------------------------

**Objetivo:** evitar reencode sempre que possível, com decisões baseadas em business logic.

*   Se codecs **compatíveis** com nosso playout: **remux** (cópia de vídeo/áudio).
*   Se incompatíveis (ex.: áudio `mp3` em HLS `fmp4` com `aac` requerido): transcode seletivo.
*   **Business logic**: thresholds de qualidade podem forçar transcode mesmo com codecs compatíveis.

### 4.1 Sinais de compatibilidade (para remux) com business logic

*   Vídeo `avc1/h264` (high/baseline/main), profile ≤ High, level ≤ 4.2.
*   Áudio `aac` LC 44.1/48 kHz estéreo.
*   Container: MP4/TS/fMP4 aceitos.
*   **Quality check**: VMAF ≥ threshold definido no business_logic.yaml.
*   **Aesthetic check**: passa nos critérios do Curator Vigilante.

### 4.2 Comandos típicos com configuração adaptativa

**Remux para MP4 (sem reencode) com metadados:**

```bash
ffmpeg -hide_banner -y -i source.mp4 \
  -map 0:v:0 -map 0:a:0 -c copy -movflags +faststart \
  -metadata business_logic_version="${BL_VERSION}" \
  -metadata processing_mode="remux" \
  "/vvtv/cache/tmp_downloads/<plan_id>/remux/master.mp4"
```

**Remux de HLS (concatenação de TS) → MP4:**

```bash
ffmpeg -hide_banner -y -i "index.m3u8" \
  -c copy -movflags +faststart \
  -metadata business_logic_version="${BL_VERSION}" \
  "/vvtv/cache/tmp_downloads/<plan_id>/remux/master.mp4"
```

Se `-c copy` falhar (timestamps fora/streams incompatíveis), cair para transcode (Seção 5).

* * *

5) Transcodificação & Normalização Adaptativa
---------------------------------------------

### 5.1 Alvos de entrega (VVTV) com business logic

*   **master.mp4** (mezzanine local)
*   **hls\_720p** (CBR-ish ~ 3.0–3.5 Mbps, ajustável via business logic)
*   **hls\_480p** (CBR-ish ~ 1.2–1.6 Mbps, ajustável via business logic)
*   **áudio normalizado** (LUFS alvo configurável, padrão -14)

### 5.2 Normalização de áudio (EBU R128 — two-pass) com configuração

**Passo 1: medir**

```bash
ffmpeg -hide_banner -y -i master_or_source.mp4 \
  -af "loudnorm=I=${BL_TARGET_LUFS}:TP=-1.5:LRA=11:print_format=json" -f null - 2> loud.json
```

Extrair `measured_I`, `measured_TP`, `measured_LRA`, `measured_thresh`.

**Passo 2: aplicar com business logic**

```bash
ffmpeg -hide_banner -y -i master_or_source.mp4 \
  -af "loudnorm=I=${BL_TARGET_LUFS}:TP=-1.5:LRA=11:measured_I=<I>:measured_TP=<TP>:measured_LRA=<LRA>:measured_thresh=<THR>:linear=true:print_format=summary" \
  -c:v copy -c:a aac -b:a ${BL_AUDIO_BITRATE} \
  -metadata loudness_target="${BL_TARGET_LUFS}" \
  "/vvtv/storage/ready/<plan_id>/master_normalized.mp4"
```

Se `-c:v copy` falhar por incompatibilidade, usar transcode total (5.3).

### 5.3 Transcode de vídeo (x264) com parâmetros adaptativos

**Preset padrão 1080p → mezzanine com business logic:**

```bash
ffmpeg -hide_banner -y -i source_or_remux.mp4 \
  -c:v libx264 -preset ${BL_ENCODE_PRESET} -crf ${BL_CRF_VALUE} -tune film \
  -profile:v high -level 4.2 -pix_fmt yuv420p \
  -x264-params keyint=${BL_KEYINT}:min-keyint=${BL_MIN_KEYINT}:scenecut=40:vbv-maxrate=${BL_VBV_MAXRATE}:vbv-bufsize=${BL_VBV_BUFSIZE} \
  -c:a aac -b:a ${BL_AUDIO_BITRATE} -ar 48000 \
  -metadata business_logic_version="${BL_VERSION}" \
  -metadata encode_preset="${BL_ENCODE_PRESET}" \
  -metadata crf_value="${BL_CRF_VALUE}" \
  "/vvtv/storage/ready/<plan_id>/master.mp4"
```

**HLS 720p / 480p (CBR-ish com fMP4) adaptativo:**

```bash
# 720p com parâmetros do business logic
ffmpeg -hide_banner -y -i "/vvtv/storage/ready/<plan_id>/master.mp4" \
  -vf "scale=-2:720:flags=bicubic" \
  -c:v libx264 -preset ${BL_HLS_PRESET} -profile:v high -level 4.0 -pix_fmt yuv420p \
  -b:v ${BL_720P_BITRATE} -maxrate ${BL_720P_MAXRATE} -bufsize ${BL_720P_BUFSIZE} -g ${BL_GOP_SIZE} -keyint_min ${BL_MIN_KEYINT} \
  -c:a aac -b:a ${BL_HLS_AUDIO_BITRATE} -ar 48000 \
  -f hls -hls_time ${BL_HLS_SEGMENT_TIME} -hls_playlist_type vod -hls_segment_type fmp4 \
  -hls_flags independent_segments \
  -master_pl_name master.m3u8 \
  -hls_segment_filename "/vvtv/storage/ready/<plan_id>/hls_720p_%04d.m4s" \
  "/vvtv/storage/ready/<plan_id>/hls_720p.m3u8"

# 480p com parâmetros do business logic
ffmpeg -hide_banner -y -i "/vvtv/storage/ready/<plan_id>/master.mp4" \
  -vf "scale=-2:480:flags=bicubic" \
  -c:v libx264 -preset ${BL_HLS_PRESET} -profile:v main -level 3.1 -pix_fmt yuv420p \
  -b:v ${BL_480P_BITRATE} -maxrate ${BL_480P_MAXRATE} -bufsize ${BL_480P_BUFSIZE} -g ${BL_GOP_SIZE} -keyint_min ${BL_MIN_KEYINT} \
  -c:a aac -b:a ${BL_HLS_AUDIO_BITRATE_LOW} -ar 48000 \
  -f hls -hls_time ${BL_HLS_SEGMENT_TIME} -hls_playlist_type vod -hls_segment_type fmp4 \
  -hls_flags independent_segments \
  -hls_segment_filename "/vvtv/storage/ready/<plan_id>/hls_480p_%04d.m4s" \
  "/vvtv/storage/ready/<plan_id>/hls_480p.m3u8"
```

> Observação: para manter **bit-exatidão** do PBD, se a rendition capturada já for 1080p/720p compatível, **pular reencode** e somente **empacotar** (5.4).

### 5.4 Empacotamento sem reencode com business logic

**HLS a partir de MP4 compatível:**

```bash
ffmpeg -hide_banner -y -i "/vvtv/storage/ready/<plan_id>/master_normalized.mp4" \
  -c copy -f hls -hls_time ${BL_HLS_SEGMENT_TIME} -hls_playlist_type vod -hls_segment_type fmp4 \
  -hls_flags independent_segments \
  -hls_segment_filename "/vvtv/storage/ready/<plan_id>/hls_source_%04d.m4s" \
  "/vvtv/storage/ready/<plan_id>/hls_source.m3u8"
```

* * *

6) Estrutura final de entrega (por plano) com metadados
------------------------------------------------------

```
/vvtv/storage/ready/<plan_id>/
  ├── master.mp4                 # mezzanine (ou master_normalized.mp4)
  ├── hls_720p.m3u8
  ├── hls_720p_0001.m4s ...
  ├── hls_480p.m3u8
  ├── hls_480p_0001.m4s ...
  ├── (hls_source.m3u8 + m4s)    # quando empacotado do source sem reencode
  ├── checksums.json             # hashes dos artefatos
  ├── manifest.json              # metadata consolidada do processamento
  └── business_logic_applied.json # configurações aplicadas
```

**`manifest.json` (exemplo) com business logic:**

```json
{
  "plan_id": "<uuid>",
  "source": {"type":"HLS","url":"<media_playlist_url>"},
  "captured_profile": {"resolution":"1080p","codec":"avc1"},
  "processing": {
    "audio_lufs_target": -14,
    "transcode": "copy|x264",
    "business_logic_version": "2025.10",
    "encode_preset": "slow",
    "crf_value": 20,
    "quality_tier": "high"
  },
  "artifacts": {
    "master": "master.mp4",
    "hls": ["hls_720p.m3u8", "hls_480p.m3u8"]
  },
  "durations": {"measured_s": 213},
  "quality_metrics": {
    "vmaf_score": 91.2,
    "ssim_score": 0.94,
    "psnr_score": 42.1
  },
  "hashes": {"master.mp4":"<sha256>"},
  "created_at": "<iso8601>",
  "curator_reviewed": false,
  "llm_validated": true
}
```

**`business_logic_applied.json`:**

```json
{
  "version": "2025.10",
  "applied_at": "2025-10-22T14:30:00Z",
  "parameters": {
    "target_lufs": -14.0,
    "encode_preset": "slow",
    "crf_value": 20,
    "hls_segment_time": 4,
    "quality_tier": "high",
    "vmaf_threshold": 85,
    "ssim_threshold": 0.92
  },
  "overrides": [],
  "curator_hints": []
}
```

* * *

7) Integridade, validações e QC com business logic
--------------------------------------------------

**Checks mínimos:**

*   `ffprobe` confirma **1 stream de vídeo** + **1 de áudio**, sem erros.
*   Duração ±5% da estimativa.
*   **Keyframes** ~ a cada 2 s–4 s (para zapping suave).
*   Áudio estéreo 44.1/48 kHz; **loudness** atingido (verificação com `loudnorm` summary).
*   **Checksum** SHA-256 por arquivo.
*   **Quality metrics**: VMAF/SSIM/PSNR dentro dos thresholds do business logic.

**Quality Validation com Business Logic:**

```rust
pub fn validate_quality_metrics(
    &self,
    metrics: &QualityMetrics,
    business_logic: &BusinessLogic,
) -> Result<QualityValidation> {
    let thresholds = business_logic.get_quality_thresholds();
    
    let mut validation = QualityValidation::new();
    
    // VMAF validation
    if metrics.vmaf_score < thresholds.vmaf_min {
        validation.add_failure(QualityFailure::VmafTooLow {
            actual: metrics.vmaf_score,
            required: thresholds.vmaf_min,
        });
    }
    
    // SSIM validation
    if metrics.ssim_score < thresholds.ssim_min {
        validation.add_failure(QualityFailure::SsimTooLow {
            actual: metrics.ssim_score,
            required: thresholds.ssim_min,
        });
    }
    
    // Loudness validation
    let lufs_diff = (metrics.lufs_measured - thresholds.target_lufs).abs();
    if lufs_diff > thresholds.lufs_tolerance {
        validation.add_failure(QualityFailure::LoudnessOutOfRange {
            actual: metrics.lufs_measured,
            target: thresholds.target_lufs,
            tolerance: thresholds.lufs_tolerance,
        });
    }
    
    // Duration validation
    let duration_diff_pct = ((metrics.duration_actual - metrics.duration_expected).abs() / metrics.duration_expected) * 100.0;
    if duration_diff_pct > thresholds.duration_tolerance_pct {
        validation.add_failure(QualityFailure::DurationMismatch {
            actual: metrics.duration_actual,
            expected: metrics.duration_expected,
            tolerance_pct: thresholds.duration_tolerance_pct,
        });
    }
    
    Ok(validation)
}
```

**Arquivo `checksums.json` com business logic:**

```json
{
  "master.mp4": "sha256:...",
  "hls_720p_0001.m4s": "sha256:...",
  "hls_480p.m3u8": "sha256:...",
  "business_logic_applied.json": "sha256:...",
  "manifest.json": "sha256:...",
  "validation_timestamp": "2025-10-22T14:35:00Z",
  "business_logic_version": "2025.10"
}
```

* * *

8) Atualizações de banco e staging para fila
--------------------------------------------

**`plans` → estados com business logic:**

*   `selected` → `downloading` → `processing` → `validating` → `ready`

**`playout_queue` (inserção) com metadados:**

```sql
INSERT INTO playout_queue (
    plan_id, asset_path, duration_s, status, curation_score, priority,
    created_at, node_origin, business_logic_version, quality_tier,
    vmaf_score, ssim_score, lufs_measured, curator_reviewed, llm_validated
) VALUES (?, ?, ?, 'queued', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
```

`asset_path` aponta para `master.mp4` **ou** `hls_720p.m3u8` (política preferida: usar HLS baseado no business logic).

**Queue Selection com Business Logic:**

```rust
pub fn select_next_for_queue(&self, business_logic: &BusinessLogic) -> Result<Vec<ProcessedPlan>> {
    let selection_strategy = business_logic.get_queue_selection_strategy();
    
    let query = match selection_strategy {
        QueueSelectionStrategy::HighestQuality => {
            "SELECT * FROM processed_plans WHERE status = 'ready' ORDER BY vmaf_score DESC, ssim_score DESC LIMIT ?"
        }
        QueueSelectionStrategy::Balanced => {
            "SELECT * FROM processed_plans WHERE status = 'ready' ORDER BY (curation_score * 0.6 + vmaf_score/100.0 * 0.4) DESC LIMIT ?"
        }
        QueueSelectionStrategy::FastestProcessing => {
            "SELECT * FROM processed_plans WHERE status = 'ready' ORDER BY processing_time_s ASC LIMIT ?"
        }
    };
    
    let plans = self.db.prepare(query)?
        .query_map([business_logic.get_batch_size()], |row| {
            Ok(ProcessedPlan::from_row(row)?)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    
    Ok(plans)
}
```

* * *

9) Recursos, limites e escalonamento com business logic
------------------------------------------------------

*   **CPU cap** por transcode: 300% (3 cores) com `nice + ionice`.
*   **RAM alvo** por ffmpeg: ≤ 1.0 GB (ajustável via business logic).
*   **IO**: segment size 4–6 s para discos SSD; evita milhares de arquivos microsegmentados.
*   **Concorrência adaptativa**:
    *   `N_downloads = 2`, `N_transcodes = 2` por nó (ajustável via business logic).
    *   Evitar baixar e transcodificar o **mesmo plano** em paralelo (lock por `plan_id`).
*   **Quality vs Speed tradeoff**: business logic pode ajustar presets baseado na demanda.

**Banda mínima por transcode** (HLS 720p): ~4 Mbps internos.  
Desacoplar downloads dos transcodes (queue interna) para evitar disputa de disco.

**Resource Management com Business Logic:**

```rust
pub struct ProcessorResourceManager {
    business_logic: Arc<BusinessLogic>,
    active_downloads: Arc<Semaphore>,
    active_transcodes: Arc<Semaphore>,
    cpu_monitor: CpuMonitor,
    memory_monitor: MemoryMonitor,
}

impl ProcessorResourceManager {
    pub async fn acquire_download_slot(&self) -> Result<DownloadPermit> {
        let max_downloads = self.business_logic.get_max_concurrent_downloads();
        let permit = self.active_downloads.acquire_many(1).await?;
        
        // Check system resources
        if self.cpu_monitor.current_usage() > self.business_logic.get_cpu_limit() {
            return Err(ProcessorError::ResourceExhausted("CPU limit exceeded".to_string()));
        }
        
        Ok(DownloadPermit { permit })
    }
    
    pub async fn acquire_transcode_slot(&self, quality_tier: QualityTier) -> Result<TranscodePermit> {
        let max_transcodes = match quality_tier {
            QualityTier::High => self.business_logic.get_max_concurrent_high_quality_transcodes(),
            QualityTier::Standard => self.business_logic.get_max_concurrent_standard_transcodes(),
            QualityTier::Fast => self.business_logic.get_max_concurrent_fast_transcodes(),
        };
        
        let permit = self.active_transcodes.acquire_many(1).await?;
        
        // Adaptive resource allocation
        if self.memory_monitor.available_gb() < self.business_logic.get_min_memory_gb() {
            return Err(ProcessorError::ResourceExhausted("Memory limit exceeded".to_string()));
        }
        
        Ok(TranscodePermit { permit, quality_tier })
    }
}
```

* * *

10) Tratamento de falhas (árvore de decisão) com business logic
--------------------------------------------------------------

1.  **PBD falhou (não tocou HD):**
    *   Tentar 720p; se ainda falhar → próximo plano.
    *   **Business logic**: pode ajustar thresholds de qualidade baseado na demanda.
2.  **Manifest inconsistente:**
    *   Recoletar; se não fechar, **reject**.
    *   **LLM analysis**: pode sugerir estratégias alternativas.
3.  **Download parcial:**
    *   Retomar; se 3 tentativas falharem, **reject**.
    *   **Adaptive retry**: intervalos ajustados pelo business logic.
4.  **Remux falhou:**
    *   Transcode total (5.3) com parâmetros do business logic.
5.  **Transcode falhou:**
    *   Repetir com `-preset faster`; se falhar, **quarentena** do plano.
    *   **Quality degradation**: business logic pode permitir qualidade menor em emergências.
6.  **QC reprovado (áudio/loudness/keyframes):**
    *   Reprocessar só áudio ou só gop; 1 retry.
    *   **Curator escalation**: falhas repetidas são reportadas ao Curator Vigilante.

Todos os "reject/quarentena" ficam listados em `/vvtv/system/logs/processor_failures.log` (rotativo 7d) com contexto de business logic.

**Failure Analysis com Business Logic:**

```rust
pub async fn analyze_processing_failure(
    &self,
    failure: &ProcessingFailure,
    business_logic: &BusinessLogic,
) -> FailureAnalysis {
    let mut analysis = FailureAnalysis::new();
    
    // Categorize failure
    match &failure.error_type {
        ProcessingErrorType::PbdFailed => {
            if business_logic.allow_quality_degradation() {
                analysis.suggested_action = SuggestedAction::RetryWithLowerQuality;
            } else {
                analysis.suggested_action = SuggestedAction::Skip;
            }
        }
        ProcessingErrorType::QualityCheckFailed { metric, actual, required } => {
            let tolerance = business_logic.get_quality_tolerance_for_emergency();
            if *actual >= (*required * tolerance) {
                analysis.suggested_action = SuggestedAction::AcceptWithWarning;
            } else {
                analysis.suggested_action = SuggestedAction::Reprocess;
            }
        }
        ProcessingErrorType::ResourceExhausted => {
            analysis.suggested_action = SuggestedAction::RetryLater;
            analysis.retry_delay = business_logic.get_resource_retry_delay();
        }
        _ => {
            analysis.suggested_action = SuggestedAction::Escalate;
        }
    }
    
    // Check for patterns
    if self.failure_tracker.has_pattern(&failure.plan_id, &failure.error_type) {
        analysis.pattern_detected = true;
        analysis.suggested_action = SuggestedAction::Quarantine;
    }
    
    analysis
}
```

* * *

11) Segurança operacional com business logic
--------------------------------------------

*   **Sem persistir cookies** de "fontes adultas" pós-download (limpeza por domínio).
*   **Sem abrir arquivos externos** durante transcode além dos previstos.
*   **TMP sandboxado** por `plan_id`.
*   **Remoção de EXIF/metadata** no `master.mp4` (usar `-map_metadata -1` se necessário).
*   **Business logic validation**: todas as configurações são validadas antes da aplicação.
*   **Audit trail**: todas as decisões de processamento são logadas com contexto de business logic.

**Security Validation:**

```rust
pub fn validate_processing_security(
    &self,
    plan: &Plan,
    business_logic: &BusinessLogic,
) -> Result<SecurityValidation> {
    let mut validation = SecurityValidation::new();
    
    // Validate source URL
    if !business_logic.is_domain_allowed(&plan.source_domain) {
        validation.add_violation(SecurityViolation::UnauthorizedDomain {
            domain: plan.source_domain.clone(),
        });
    }
    
    // Validate file size limits
    if plan.estimated_size_mb > business_logic.get_max_file_size_mb() {
        validation.add_violation(SecurityViolation::FileSizeExceeded {
            actual: plan.estimated_size_mb,
            limit: business_logic.get_max_file_size_mb(),
        });
    }
    
    // Validate processing parameters
    let processing_params = business_logic.get_processing_params_for_plan(plan);
    if !self.validate_ffmpeg_params(&processing_params) {
        validation.add_violation(SecurityViolation::UnsafeProcessingParams);
    }
    
    Ok(validation)
}
```

* * *

12) QA — checklist por item com business logic
----------------------------------------------

*    Página reaberta e **play** confirmado
*    Qualidade HD/720p forçada (ou conforme business logic)
*    Fonte capturada (HLS/DASH/progressivo)
*    Download completo e íntegro
*    Áudio normalizado para **target LUFS** (configurável via business logic)
*    Entrega HLS/MP4 conforme política de business logic
*    **Quality metrics** dentro dos thresholds (VMAF/SSIM/PSNR)
*    Checksums gerados e validados
*    **Business logic applied** e documentado
*    Plano atualizado: `ready`
*    Inserção na `playout_queue: queued` com metadados completos
*    **Curator notification** (se configurado)
*    **LLM validation** (se habilitado)

**QA Automation com Business Logic:**

```rust
pub async fn run_qa_checklist(
    &self,
    plan_id: &str,
    business_logic: &BusinessLogic,
) -> Result<QaReport> {
    let mut report = QaReport::new(plan_id);
    
    // Check 1: File existence and integrity
    let files_check = self.verify_output_files(plan_id).await?;
    report.add_check("files_integrity", files_check);
    
    // Check 2: Quality metrics
    let quality_check = self.verify_quality_metrics(plan_id, business_logic).await?;
    report.add_check("quality_metrics", quality_check);
    
    // Check 3: Audio normalization
    let audio_check = self.verify_audio_normalization(plan_id, business_logic).await?;
    report.add_check("audio_normalization", audio_check);
    
    // Check 4: Business logic compliance
    let bl_check = self.verify_business_logic_compliance(plan_id, business_logic).await?;
    report.add_check("business_logic_compliance", bl_check);
    
    // Check 5: Metadata completeness
    let metadata_check = self.verify_metadata_completeness(plan_id).await?;
    report.add_check("metadata_completeness", metadata_check);
    
    // Generate overall score
    report.calculate_overall_score();
    
    // Log results
    info!(
        target: "processor.qa",
        plan_id = plan_id,
        overall_score = report.overall_score,
        passed_checks = report.passed_checks,
        total_checks = report.total_checks,
        "QA checklist completed"
    );
    
    Ok(report)
}
```

* * *

13) Pseudocódigo integrador (Rust) com business logic
-----------------------------------------------------

```rust
async fn realize_plan_with_business_logic(
    plan: Plan,
    business_logic: Arc<BusinessLogic>,
) -> Result<()> {
    // 1) Security validation
    let security_validation = validate_processing_security(&plan, &business_logic)?;
    if !security_validation.is_valid() {
        return Err(ProcessorError::SecurityViolation(security_validation));
    }
    
    // 2) Resource acquisition
    let _download_permit = resource_manager.acquire_download_slot().await?;
    
    // 3) PBD with business logic
    let media = browser::reopen_and_capture_media_with_bl(&plan.url, &business_logic).await?;
    
    // 4) Download with adaptive parameters
    let download_params = business_logic.get_download_params_for_plan(&plan);
    let src_dir = download::fetch_with_params(&plan.id, &media, &download_params).await?;
    
    // 5) Quality decision
    let quality_tier = business_logic.determine_quality_tier(&plan);
    let _transcode_permit = resource_manager.acquire_transcode_slot(quality_tier).await?;
    
    // 6) Remux / Transcode decision with business logic
    let processing_decision = decide_processing_strategy(&plan.id, &src_dir, &business_logic)?;
    let master = match processing_decision {
        ProcessingStrategy::Remux => mediaops::remux_with_metadata(&plan.id, &src_dir, &business_logic)?,
        ProcessingStrategy::Transcode => mediaops::transcode_with_bl(&plan.id, &src_dir, &business_logic)?,
    };
    
    // 7) Normalize audio with business logic target
    let target_lufs = business_logic.get_target_lufs_for_plan(&plan);
    let master_norm = audio::loudnorm_with_target(master, target_lufs).await?;
    
    // 8) Package with business logic profiles
    let profiles = business_logic.get_output_profiles_for_plan(&plan);
    let outputs = hls::package_with_profiles(&master_norm, &profiles).await?;
    
    // 9) Quality validation
    let quality_metrics = qc::measure_quality(&master_norm, &outputs).await?;
    let quality_validation = validate_quality_metrics(&quality_metrics, &business_logic)?;
    if !quality_validation.is_acceptable() {
        return Err(ProcessorError::QualityCheckFailed(quality_validation));
    }
    
    // 10) QA checklist
    let qa_report = run_qa_checklist(&plan.id, &business_logic).await?;
    if qa_report.overall_score < business_logic.get_min_qa_score() {
        return Err(ProcessorError::QaFailed(qa_report));
    }
    
    // 11) Stage & DB with full metadata
    db::plans::set_status_with_metadata(&plan.id, "ready", &quality_metrics, &business_logic.version()).await?;
    db::queue::enqueue_with_metadata(&plan.id, &outputs, &quality_metrics, &business_logic).await?;
    
    // 12) Curator notification (if enabled)
    if business_logic.notify_curator_on_completion() {
        curator::notify_plan_ready(&plan.id, &qa_report).await?;
    }
    
    Ok(())
}
```

* * *

14) Entregáveis deste bloco
---------------------------

*   Especificação de PBD no T-4h com business logic integration.
*   Scripts `ffmpeg` para **remux/transcode/normalização/packaging** com parâmetros adaptativos.
*   Rotinas de **download HLS/DASH/progressivo** com configuração inteligente.
*   `manifest.json` + `checksums.json` + `business_logic_applied.json` por plano.
*   Checklist de **QC** automatizado com thresholds configuráveis.
*   **Quality validation** com VMAF/SSIM/PSNR integration.
*   **Resource management** adaptativo baseado em business logic.
*   **Failure analysis** e recovery strategies inteligentes.
*   Pseudocódigo de integração completa com business logic.

* * *

15) Ready-for-Build
-------------------

*    Worker Processor com limites de CPU/IO adaptativos.
*    PBD revalidado no T-4h com business logic integration.
*    HLS/DASH/Progressivo cobertos com parâmetros configuráveis.
*    Normalização EBU R128 validada com target LUFS configurável.
*    Packaging HLS rodando (segment time configurável).
*    QC automatizado ativo com thresholds de business logic.
*    **Quality metrics** (VMAF/SSIM/PSNR) integrados.
*    **Resource management** com business logic constraints.
*    **Security validation** e audit trail completos.
*    Integração com `playout_queue` concluída com metadados completos.
*    **Curator integration** para notificações e escalation.
*    **LLM validation** hooks implementados.

* * *🧠 VVT
V INDUSTRIAL DOSSIER
==========================

**Bloco IV — Queue & Playout Engineering**
------------------------------------------

_(Gestão de fila adaptativa, "curation bump", watchdogs inteligentes, buffer ≥ 4 h, RTMP/HLS origin, failover e métricas com business logic)_

* * *

### 0\. Objetivo

Definir a engenharia de **fila e exibição contínua adaptativa**: manter sempre pelo menos **4 horas de conteúdo pronto**, garantir continuidade 24/7, controlar prioridades de exibição através de algoritmos inteligentes, reações automáticas a falhas, e operar o playout com redundância e programação adaptativa baseada em business logic.

* * *

1) Fila Computável Adaptativa
-----------------------------

**Tabela:** `playout_queue.sqlite`

| Campo | Tipo | Descrição |
| --- | --- | --- |
| `id` | INTEGER PK | Sequência automática |
| `plan_id` | TEXT | Referência ao plano processado |
| `asset_path` | TEXT | Caminho do arquivo (HLS/MP4) |
| `duration_s` | INT | Duração real medida |
| `status` | TEXT | `queued` / `playing` / `played` / `failed` |
| `curation_score` | FLOAT | Peso de relevância estética |
| `priority` | INT | 0 = normal, 1 = bump, 2 = urgent |
| `created_at` / `updated_at` | DATETIME | Auditoria temporal |
| `node_origin` | TEXT | Identificação do nó de produção |
| `business_logic_version` | TEXT | Versão da configuração aplicada |
| `gumbel_seed` | INTEGER | Seed usado na seleção |
| `selection_temperature` | FLOAT | Temperatura aplicada |
| `curator_intervention` | BOOLEAN | Se Curator Vigilante interveniu |
| `llm_confidence` | FLOAT | Confiança da análise LLM |
| `quality_tier` | TEXT | high/standard/fast |
| `vmaf_score` | FLOAT | Score de qualidade visual |
| `adaptive_weight` | FLOAT | Peso adaptativo calculado |

**Política de limpeza:** remover registros `played` > 72 h e manter backup diário (`.sql.gz`) com metadados de business logic.

* * *

2) Política FIFO + "Curation Bump" + Adaptive Programming
---------------------------------------------------------

A ordem de exibição segue FIFO **com desvios controlados inteligentes**:

1.  A fila é lida em ordem de `created_at` com **peso adaptativo**.
2.  Um algoritmo de _curation bump_ aumenta a prioridade de itens com `curation_score > threshold` configurável.
3.  **Adaptive ratio**: proporção música/vídeo ajustada dinamicamente baseada em métricas de audiência.
4.  **Gumbel influence**: itens selecionados com alta temperatura Gumbel recebem boost temporal.
5.  **Curator override**: Curator Vigilante pode reordenar até 4 posições.
6.  **Quality preference**: business logic pode priorizar qualidade vs diversidade.

**Algoritmo de Seleção Adaptativa:**

```rust
pub fn select_next_items_adaptive(
    &self,
    business_logic: &BusinessLogic,
    audience_metrics: &AudienceMetrics,
) -> Result<Vec<QueueItem>> {
    let selection_params = business_logic.get_queue_selection_params();
    let adaptive_adjustments = self.calculate_adaptive_adjustments(audience_metrics, business_logic)?;
    
    // Base query with business logic filters
    let mut query = QueryBuilder::new()
        .select_from("playout_queue")
        .where_status("queued")
        .where_quality_tier_in(&selection_params.allowed_quality_tiers);
    
    // Apply adaptive filters
    if adaptive_adjustments.prefer_high_quality {
        query = query.order_by("vmaf_score DESC, curation_score DESC");
    } else if adaptive_adjustments.prefer_diversity {
        query = query.order_by("selection_temperature DESC, created_at ASC");
    } else {
        // Standard FIFO with curation bump
        query = query.order_by("priority DESC, (curation_score * adaptive_weight) DESC, created_at ASC");
    }
    
    let candidates = query.limit(selection_params.batch_size * 2).execute()?;
    
    // Apply music/video ratio
    let target_music_ratio = adaptive_adjustments.music_ratio.unwrap_or(selection_params.default_music_ratio);
    let selected = self.apply_content_ratio(candidates, target_music_ratio)?;
    
    // Apply curator interventions
    if business_logic.curator_enabled() {
        let curator_review = self.curator_vigilante.review_queue_selection(&selected)?;
        if curator_review.should_intervene() {
            return Ok(self.apply_curator_reorder(selected, curator_review));
        }
    }
    
    Ok(selected)
}

fn calculate_adaptive_adjustments(
    &self,
    metrics: &AudienceMetrics,
    business_logic: &BusinessLogic,
) -> Result<AdaptiveAdjustments> {
    let mut adjustments = AdaptiveAdjustments::default();
    
    // Retention-based adjustments
    if metrics.retention_30min < 0.6 {
        adjustments.prefer_diversity = true;
        adjustments.music_ratio = Some(0.15); // More music for variety
        info!(target: "queue.adaptive", "Low retention detected, increasing diversity");
    } else if metrics.retention_30min > 0.8 {
        adjustments.prefer_high_quality = true;
        adjustments.music_ratio = Some(0.08); // Less music, focus on quality
        info!(target: "queue.adaptive", "High retention detected, prioritizing quality");
    }
    
    // Time-based adjustments
    let current_hour = Utc::now().hour();
    if current_hour >= 22 || current_hour <= 6 {
        adjustments.prefer_calm_content = true;
        adjustments.energy_threshold = Some(0.4); // Lower energy content at night
    }
    
    // Geographic adjustments
    if let Some(dominant_region) = metrics.get_dominant_region() {
        match dominant_region.as_str() {
            "BR" | "PT" => {
                adjustments.cultural_preference = Some("latin".to_string());
                adjustments.music_ratio = Some(0.12); // Slightly more music for Latin audience
            }
            "US" | "CA" => {
                adjustments.cultural_preference = Some("western".to_string());
            }
            _ => {}
        }
    }
    
    // Business logic constraints
    adjustments.apply_business_logic_constraints(business_logic);
    
    Ok(adjustments)
}
```

* * *

3) Buffer de Segurança Inteligente
----------------------------------

*   **Meta mínima adaptativa:** 4-8 h de duração somada em `queued` (ajustável via business logic).
*   **Alerta amarelo:** < threshold configurável (padrão 2 h).
*   **Emergência:** < threshold crítico (padrão 1 h) → acionar modo _loop replay_ inteligente.
*   **Atualização:** verificar a cada 60 s ou após cada playout concluído.
*   **Predictive buffering:** algoritmo prevê demanda baseado em padrões históricos.

**Buffer Management com Business Logic:**

```rust
pub struct BufferManager {
    business_logic: Arc<BusinessLogic>,
    metrics_collector: MetricsCollector,
    emergency_content: EmergencyContentPool,
}

impl BufferManager {
    pub async fn check_buffer_health(&self) -> Result<BufferHealth> {
        let current_buffer = self.calculate_current_buffer().await?;
        let thresholds = self.business_logic.get_buffer_thresholds();
        let predicted_consumption = self.predict_consumption_rate().await?;
        
        let health = if current_buffer.duration_hours < thresholds.critical {
            BufferHealth::Critical {
                current: current_buffer.duration_hours,
                predicted_empty_in: current_buffer.duration_hours / predicted_consumption,
                action_required: BufferAction::ActivateEmergencyLoop,
            }
        } else if current_buffer.duration_hours < thresholds.warning {
            BufferHealth::Warning {
                current: current_buffer.duration_hours,
                predicted_empty_in: current_buffer.duration_hours / predicted_consumption,
                action_required: BufferAction::IncreaseProcessing,
            }
        } else if current_buffer.duration_hours > thresholds.optimal_max {
            BufferHealth::Excessive {
                current: current_buffer.duration_hours,
                action_required: BufferAction::ReduceProcessing,
            }
        } else {
            BufferHealth::Healthy {
                current: current_buffer.duration_hours,
                predicted_empty_in: current_buffer.duration_hours / predicted_consumption,
            }
        };
        
        // Log buffer status
        info!(
            target: "buffer.health",
            duration_hours = current_buffer.duration_hours,
            item_count = current_buffer.item_count,
            consumption_rate = predicted_consumption,
            health_status = ?health,
            "Buffer health check completed"
        );
        
        Ok(health)
    }
    
    pub async fn handle_buffer_emergency(&self) -> Result<()> {
        warn!(target: "buffer.emergency", "Buffer emergency detected, activating emergency measures");
        
        // 1. Activate emergency content loop
        let emergency_items = self.emergency_content.get_loop_content(
            self.business_logic.get_emergency_loop_duration()
        )?;
        
        for item in emergency_items {
            self.queue_emergency_item(item).await?;
        }
        
        // 2. Boost processing priority
        self.boost_processing_priority().await?;
        
        // 3. Notify operators if configured
        if self.business_logic.notify_on_emergency() {
            self.send_emergency_notification().await?;
        }
        
        // 4. Adjust business logic for faster processing
        let emergency_adjustments = BusinessLogicAdjustments {
            reduce_quality_for_speed: true,
            increase_concurrency: true,
            skip_optional_processing: true,
        };
        
        self.apply_temporary_adjustments(emergency_adjustments).await?;
        
        Ok(())
    }
}
```

* * *

4) Watchdogs Inteligentes
------------------------

### 4.1 Loop Principal Adaptativo

Verifica com inteligência baseada em business logic:

*   Streaming ativo (`ffprobe` no RTMP);
*   Buffer ≥ mínimo adaptativo;
*   Nenhum processo `ffmpeg` travado;
*   **Quality metrics** dentro dos thresholds;
*   **Audience engagement** não degradando;
*   **Business logic compliance** mantida.

### 4.2 Reação a Falhas Inteligente

| Falha | Ação Padrão | Ação com Business Logic |
| --- | --- | --- |
| RTMP inativo > 5 s | Reiniciar nginx-rtmp + ffmpeg | + Verificar quality tier, ajustar bitrate |
| CPU > 90 % por 5 min | Suspender novos downloads | + Reduzir quality tier temporariamente |
| Fila vazia | Entrar em loop local de vídeos reservas | + Selecionar baseado em audience metrics |
| Falha de mídia | Marcar `failed`, logar motivo, seguir próximo | + Analisar padrão, ajustar seleção futura |
| Disco < 5 % livre | Pausar processor até limpeza | + Priorizar cleanup baseado em business logic |
| Quality degradation | Log warning | + Ajustar parâmetros automaticamente |
| Audience drop > 30% | Log metric | + Ativar modo diversidade, boost music ratio |

**Intelligent Watchdog Implementation:**

```rust
pub struct IntelligentWatchdog {
    business_logic: Arc<BusinessLogic>,
    metrics_collector: MetricsCollector,
    failure_analyzer: FailureAnalyzer,
    auto_recovery: AutoRecoverySystem,
}

impl IntelligentWatchdog {
    pub async fn run_monitoring_cycle(&self) -> Result<()> {
        let monitoring_config = self.business_logic.get_monitoring_config();
        
        // Collect current metrics
        let system_metrics = self.collect_system_metrics().await?;
        let stream_metrics = self.collect_stream_metrics().await?;
        let audience_metrics = self.collect_audience_metrics().await?;
        
        // Analyze health with business logic context
        let health_analysis = self.analyze_system_health(
            &system_metrics,
            &stream_metrics,
            &audience_metrics,
            &monitoring_config,
        ).await?;
        
        // Take actions based on analysis
        for issue in health_analysis.issues {
            match issue.severity {
                IssueSeverity::Critical => {
                    self.handle_critical_issue(&issue).await?;
                }
                IssueSeverity::Warning => {
                    self.handle_warning_issue(&issue).await?;
                }
                IssueSeverity::Info => {
                    self.log_info_issue(&issue);
                }
            }
        }
        
        // Update business logic if needed
        if health_analysis.suggests_bl_adjustment {
            self.suggest_business_logic_adjustment(&health_analysis).await?;
        }
        
        Ok(())
    }
    
    async fn handle_critical_issue(&self, issue: &SystemIssue) -> Result<()> {
        match &issue.issue_type {
            IssueType::StreamDown => {
                self.auto_recovery.restart_stream_with_fallback().await?;
            }
            IssueType::QualityDegraded { current, threshold } => {
                if self.business_logic.allow_automatic_quality_adjustment() {
                    self.adjust_quality_parameters(*current, *threshold).await?;
                } else {
                    self.escalate_to_operator(issue).await?;
                }
            }
            IssueType::AudienceDropped { drop_percentage } => {
                if *drop_percentage > 50.0 {
                    self.activate_emergency_programming().await?;
                }
            }
            _ => {
                self.escalate_to_operator(issue).await?;
            }
        }
        
        Ok(())
    }
}
```

**Serviço:** `intelligent_watchdogd` (ciclo adaptativo 15-60 s baseado em load) + log rotativo 7 dias com contexto de business logic.

* * *

5) RTMP/HLS Origin com Configuração Adaptativa
----------------------------------------------

### 5.1 RTMP Source com Business Logic

```nginx
# /vvtv/broadcast/nginx.conf - Generated from business_logic.yaml
rtmp {
  server {
    listen 1935;
    chunk_size ${BL_RTMP_CHUNK_SIZE};
    
    application live {
      live on;
      record off;
      
      # Adaptive recording based on business logic
      ${BL_RECORDING_ENABLED ? "record all;" : ""}
      ${BL_RECORDING_ENABLED ? "record_path /vvtv/broadcast/recordings;" : ""}
      
      # HLS output with adaptive parameters
      hls on;
      hls_path /vvtv/broadcast/hls;
      hls_fragment ${BL_HLS_SEGMENT_DURATION};
      hls_playlist_length ${BL_HLS_PLAYLIST_LENGTH};
      
      # Quality-based transcoding
      ${BL_ENABLE_TRANSCODING ? "exec ffmpeg -i rtmp://localhost/live/$name" : ""}
      ${BL_ENABLE_TRANSCODING ? "  -c:v libx264 -preset ${BL_TRANSCODE_PRESET}" : ""}
      ${BL_ENABLE_TRANSCODING ? "  -b:v ${BL_TRANSCODE_BITRATE} -c:a aac" : ""}
      ${BL_ENABLE_TRANSCODING ? "  -f flv rtmp://localhost/live_transcoded/$name;" : ""}
    }
    
    # Transcoded stream application (if enabled)
    application live_transcoded {
      live on;
      
      # Adaptive HLS output
      hls on;
      hls_path /vvtv/broadcast/hls_transcoded;
      hls_fragment ${BL_HLS_SEGMENT_DURATION};
      hls_playlist_length ${BL_HLS_PLAYLIST_LENGTH};
      hls_variant _low BANDWIDTH=${BL_LOW_BANDWIDTH};
      hls_variant _mid BANDWIDTH=${BL_MID_BANDWIDTH};
      hls_variant _high BANDWIDTH=${BL_HIGH_BANDWIDTH};
    }
  }
}
```

### 5.2 HLS Output Adaptativo

```
/vvtv/broadcast/hls/
  ├── live.m3u8                    # Master playlist
  ├── live_${BL_QUALITY_TIER}.m3u8 # Quality-specific playlists
  ├── segment_${QUALITY}_*.ts      # Adaptive segments
  └── metadata/
      ├── business_logic_applied.json
      └── stream_metrics.json
```

Rotacionar segmentos a cada `${BL_HLS_SEGMENT_DURATION}` s e manter playlist com duração configurável.  
O broadcaster inicia novo segmento enquanto transmite o anterior, com parâmetros adaptativos.

**Dynamic HLS Configuration:**

```rust
pub struct AdaptiveHlsConfig {
    business_logic: Arc<BusinessLogic>,
    audience_metrics: Arc<AudienceMetrics>,
}

impl AdaptiveHlsConfig {
    pub fn generate_nginx_config(&self) -> Result<String> {
        let bl = &self.business_logic;
        let metrics = &self.audience_metrics;
        
        // Adapt segment duration based on audience behavior
        let segment_duration = if metrics.get_average_session_duration() < 300.0 {
            // Short sessions - use shorter segments for faster startup
            bl.get_hls_segment_duration_min()
        } else {
            // Longer sessions - use standard segments for efficiency
            bl.get_hls_segment_duration_standard()
        };
        
        // Adapt playlist length based on buffer requirements
        let playlist_length = if metrics.get_buffer_health() < 0.5 {
            bl.get_hls_playlist_length_min()
        } else {
            bl.get_hls_playlist_length_standard()
        };
        
        // Generate adaptive bitrate ladder
        let bitrate_ladder = self.generate_adaptive_bitrate_ladder(metrics)?;
        
        let config = format!(
            include_str!("templates/nginx_rtmp.conf.template"),
            segment_duration = segment_duration,
            playlist_length = playlist_length,
            bitrate_ladder = bitrate_ladder,
            chunk_size = bl.get_rtmp_chunk_size(),
            enable_recording = bl.is_recording_enabled(),
            enable_transcoding = bl.is_transcoding_enabled(),
        );
        
        Ok(config)
    }
    
    fn generate_adaptive_bitrate_ladder(&self, metrics: &AudienceMetrics) -> Result<String> {
        let mut ladder = Vec::new();
        
        // Base quality tiers from business logic
        let quality_tiers = self.business_logic.get_quality_tiers();
        
        for tier in quality_tiers {
            // Adapt bitrates based on audience bandwidth distribution
            let adapted_bitrate = self.adapt_bitrate_for_audience(tier.base_bitrate, metrics);
            
            ladder.push(format!(
                "hls_variant _{} BANDWIDTH={} RESOLUTION={}x{}",
                tier.name.to_lowercase(),
                adapted_bitrate,
                tier.width,
                tier.height
            ));
        }
        
        Ok(ladder.join("\n      "))
    }
}
```

* * *

6) Motor de Playout Adaptativo
------------------------------

**Input:** fila `queued` com seleção inteligente.  
**Output:** RTMP stream com qualidade adaptativa.

```bash
# Adaptive FFmpeg command generation
ffmpeg -re -i "${SELECTED_ASSET_PATH}" \
  -c:v ${BL_VIDEO_CODEC} -preset ${BL_ENCODE_PRESET} \
  -b:v ${BL_ADAPTIVE_BITRATE} -maxrate ${BL_MAX_BITRATE} \
  -bufsize ${BL_BUFFER_SIZE} -g ${BL_GOP_SIZE} \
  -c:a ${BL_AUDIO_CODEC} -b:a ${BL_AUDIO_BITRATE} \
  -ar ${BL_AUDIO_SAMPLE_RATE} \
  -metadata business_logic_version="${BL_VERSION}" \
  -metadata stream_quality_tier="${BL_QUALITY_TIER}" \
  -f flv rtmp://localhost/live/main
```

**Ciclo Adaptativo:**

1.  Selecionar próximo `queued` usando algoritmo adaptativo.
2.  Aplicar parâmetros de business logic para encoding.
3.  Atualizar status → `playing` com metadados.
4.  Executar comando ffmpeg com monitoramento de qualidade.
5.  Coletar métricas de stream (bitrate, fps, quality).
6.  Atualizar `played` com métricas de performance.
7.  Analisar performance e ajustar próximos parâmetros.
8.  Recalcular buffer e retomar com configuração adaptada.

**Adaptive Playout Engine:**

```rust
pub struct AdaptivePlayoutEngine {
    business_logic: Arc<BusinessLogic>,
    quality_monitor: QualityMonitor,
    audience_feedback: AudienceFeedback,
    encoder_controller: EncoderController,
}

impl AdaptivePlayoutEngine {
    pub async fn play_next_item(&self) -> Result<PlayoutResult> {
        // 1. Select next item with adaptive algorithm
        let next_item = self.select_next_item_adaptive().await?;
        
        // 2. Determine encoding parameters based on business logic and current conditions
        let encoding_params = self.determine_encoding_params(&next_item).await?;
        
        // 3. Start playback with monitoring
        let playback_session = self.start_playback_with_monitoring(&next_item, &encoding_params).await?;
        
        // 4. Monitor quality and audience response during playback
        let monitoring_task = tokio::spawn({
            let quality_monitor = self.quality_monitor.clone();
            let audience_feedback = self.audience_feedback.clone();
            let session_id = playback_session.id.clone();
            
            async move {
                Self::monitor_playback_quality(quality_monitor, audience_feedback, session_id).await
            }
        });
        
        // 5. Wait for playback completion
        let playback_result = playback_session.wait_for_completion().await?;
        
        // 6. Collect monitoring results
        let quality_metrics = monitoring_task.await??;
        
        // 7. Update database with results
        self.update_playback_results(&next_item, &playback_result, &quality_metrics).await?;
        
        // 8. Learn from this playback for future adaptations
        self.update_adaptive_parameters(&playback_result, &quality_metrics).await?;
        
        Ok(PlayoutResult {
            item: next_item,
            encoding_params,
            playback_result,
            quality_metrics,
        })
    }
    
    async fn determine_encoding_params(&self, item: &QueueItem) -> Result<EncodingParams> {
        let base_params = self.business_logic.get_encoding_params_for_tier(&item.quality_tier);
        
        // Adapt based on current system load
        let system_load = self.get_current_system_load().await?;
        let adapted_params = if system_load > 0.8 {
            base_params.reduce_quality_for_performance()
        } else if system_load < 0.4 {
            base_params.increase_quality_for_better_output()
        } else {
            base_params
        };
        
        // Adapt based on audience feedback
        let audience_metrics = self.audience_feedback.get_current_metrics().await?;
        let final_params = if audience_metrics.quality_complaints > 0.1 {
            adapted_params.increase_quality_for_satisfaction()
        } else if audience_metrics.buffering_events > 0.05 {
            adapted_params.reduce_bitrate_for_stability()
        } else {
            adapted_params
        };
        
        Ok(final_params)
    }
}
```

* * *

7) Failover Local Inteligente
-----------------------------

*   **Hot standby:** segundo processo ffmpeg (`rtmp://localhost/live/failover`) aguardando fila duplicada.
*   **Intelligent switching:** detecção de queda do stream principal > threshold configurável (padrão 3 s).
*   **Quality-aware failover:** failover considera qualidade do stream backup.
*   **Automatic recovery:** tentativa de reiniciar primário em background com parâmetros adaptativos.
*   **Seamless transition:** crossfade entre streams quando possível.

**Backup:** últimas 4-8 horas gravadas em `/vvtv/storage/archive/live_<ts>.mp4` (configurável via business logic).

**Intelligent Failover System:**

```rust
pub struct IntelligentFailoverSystem {
    business_logic: Arc<BusinessLogic>,
    primary_monitor: StreamMonitor,
    backup_monitor: StreamMonitor,
    switch_controller: SwitchController,
}

impl IntelligentFailoverSystem {
    pub async fn monitor_and_failover(&self) -> Result<()> {
        let failover_config = self.business_logic.get_failover_config();
        
        loop {
            let primary_health = self.primary_monitor.check_health().await?;
            let backup_health = self.backup_monitor.check_health().await?;
            
            match (primary_health.status, backup_health.status) {
                (StreamStatus::Healthy, _) => {
                    // Primary is healthy, ensure it's active
                    if self.switch_controller.current_active() != StreamSource::Primary {
                        self.switch_to_primary_with_validation().await?;
                    }
                }
                (StreamStatus::Degraded, StreamStatus::Healthy) => {
                    // Primary degraded but backup healthy
                    if self.should_failover_on_degradation(&primary_health, &failover_config) {
                        self.failover_to_backup("Primary stream degraded").await?;
                    }
                }
                (StreamStatus::Failed, StreamStatus::Healthy) => {
                    // Primary failed, backup healthy
                    self.failover_to_backup("Primary stream failed").await?;
                }
                (StreamStatus::Failed, StreamStatus::Failed) => {
                    // Both failed - emergency mode
                    self.activate_emergency_mode().await?;
                }
                _ => {
                    // Other combinations - log and continue monitoring
                    info!(
                        target: "failover",
                        primary_status = ?primary_health.status,
                        backup_status = ?backup_health.status,
                        "Monitoring stream health"
                    );
                }
            }
            
            tokio::time::sleep(Duration::from_secs(failover_config.check_interval_seconds)).await;
        }
    }
    
    async fn failover_to_backup(&self, reason: &str) -> Result<()> {
        warn!(target: "failover", reason = reason, "Initiating failover to backup stream");
        
        // 1. Switch traffic to backup
        self.switch_controller.switch_to_backup().await?;
        
        // 2. Attempt to restart primary in background
        let restart_task = tokio::spawn({
            let primary_monitor = self.primary_monitor.clone();
            let business_logic = self.business_logic.clone();
            
            async move {
                Self::attempt_primary_restart(primary_monitor, business_logic).await
            }
        });
        
        // 3. Monitor backup quality
        self.monitor_backup_quality().await?;
        
        // 4. Wait for primary restart (with timeout)
        let restart_timeout = self.business_logic.get_failover_config().primary_restart_timeout;
        match tokio::time::timeout(restart_timeout, restart_task).await {
            Ok(Ok(())) => {
                info!(target: "failover", "Primary stream restarted successfully");
                // Will switch back on next health check
            }
            Ok(Err(e)) => {
                warn!(target: "failover", error = %e, "Primary stream restart failed");
            }
            Err(_) => {
                warn!(target: "failover", "Primary stream restart timed out");
            }
        }
        
        Ok(())
    }
}
```

* * *

8) Sincronização de Nós Inteligente
-----------------------------------

*   **Mestre:** nó broadcast com business logic authority.
*   **Espelho:** nó backup Railway com configuração sincronizada.
*   **Sync adaptativo:** `rsync` com bandwidth limit baseado em business logic.
*   **Verificação inteligente:** comparar `checksums.json` + `business_logic_applied.json`.
*   **Conflict resolution:** business logic define prioridades de sincronização.

**Smart Sync Implementation:**

```rust
pub struct SmartSyncManager {
    business_logic: Arc<BusinessLogic>,
    sync_config: SyncConfig,
    conflict_resolver: ConflictResolver,
}

impl SmartSyncManager {
    pub async fn sync_with_backup(&self) -> Result<SyncResult> {
        let sync_params = self.business_logic.get_sync_parameters();
        
        // 1. Check what needs syncing
        let sync_analysis = self.analyze_sync_requirements().await?;
        
        // 2. Prioritize sync items based on business logic
        let prioritized_items = self.prioritize_sync_items(sync_analysis.items, &sync_params)?;
        
        // 3. Execute sync with adaptive bandwidth
        let bandwidth_limit = self.calculate_adaptive_bandwidth_limit().await?;
        
        for item in prioritized_items {
            match item.item_type {
                SyncItemType::ReadyContent => {
                    self.sync_ready_content(&item, bandwidth_limit).await?;
                }
                SyncItemType::BusinessLogicConfig => {
                    self.sync_business_logic_config(&item).await?;
                }
                SyncItemType::QueueState => {
                    self.sync_queue_state(&item).await?;
                }
                SyncItemType::Metrics => {
                    self.sync_metrics(&item).await?;
                }
            }
        }
        
        // 4. Verify sync integrity
        let verification_result = self.verify_sync_integrity().await?;
        
        Ok(SyncResult {
            items_synced: prioritized_items.len(),
            bytes_transferred: verification_result.bytes_transferred,
            duration: verification_result.duration,
            integrity_check: verification_result.integrity_check,
        })
    }
}
```

**Sync command adaptativo:**

```bash
# Adaptive rsync with business logic parameters
rsync -av --delete \
  --bwlimit=${BL_SYNC_BANDWIDTH_LIMIT} \
  --timeout=${BL_SYNC_TIMEOUT} \
  --exclude-from=${BL_SYNC_EXCLUDE_FILE} \
  --include="*.json" --include="business_logic_applied.json" \
  /vvtv/storage/ready/ backup@railway:/vvtv/storage/ready/
```

**Cron adaptativo:** intervalo baseado em business logic (padrão 1 h, pode ser 15 min - 4 h).

* * *

9) Métricas Locais Inteligentes
-------------------------------

`metrics.sqlite` (sem rede, com business logic context):

| Métrica | Unidade | Intervalo | Fonte | Business Logic Context |
| --- | --- | --- | --- | --- |
| `buffer_duration_h` | horas | 60 s | soma fila | Target vs actual |
| `queue_length` | count | 60 s | SQL count | Optimal range |
| `played_last_hour` | count | 1 h | eventos | Expected throughput |
| `failures_last_hour` | count | 1 h | watchdog | Failure tolerance |
| `avg_cpu_load` | % | 5 min | `sysctl` | CPU limits |
| `avg_temp_c` | °C | 5 min | sensor | Thermal thresholds |
| `selection_entropy` | 0-1 | 15 min | planner | Diversity target |
| `curator_interventions` | count | 1 h | curator | Intervention budget |
| `llm_success_rate` | % | 1 h | llm | Circuit breaker threshold |
| `adaptive_adjustments` | count | 1 h | business logic | Adaptation frequency |
| `quality_tier_distribution` | % | 1 h | queue | Quality balance |
| `audience_retention_30min` | % | 30 min | stream | Retention target |

Arquivado em JSON diário (14 dias) com contexto de business logic.

**Enhanced Metrics Collection:**

```rust
pub struct IntelligentMetricsCollector {
    business_logic: Arc<BusinessLogic>,
    db: SqliteConnection,
    collectors: HashMap<String, Box<dyn MetricCollector>>,
}

impl IntelligentMetricsCollector {
    pub async fn collect_and_analyze(&self) -> Result<MetricsReport> {
        let collection_config = self.business_logic.get_metrics_config();
        let mut report = MetricsReport::new();
        
        // Collect base metrics
        for (name, collector) in &self.collectors {
            let metric_value = collector.collect().await?;
            report.add_metric(name.clone(), metric_value);
        }
        
        // Add business logic context
        report.add_context("business_logic_version", self.business_logic.version());
        report.add_context("collection_timestamp", Utc::now().to_rfc3339());
        
        // Analyze against business logic targets
        let analysis = self.analyze_metrics_against_targets(&report)?;
        report.set_analysis(analysis);
        
        // Store in database
        self.store_metrics_report(&report).await?;
        
        // Trigger alerts if needed
        if analysis.has_critical_issues() {
            self.trigger_alerts(&analysis).await?;
        }
        
        // Suggest business logic adjustments if patterns detected
        if let Some(suggestions) = analysis.get_bl_adjustment_suggestions() {
            self.log_bl_suggestions(&suggestions);
        }
        
        Ok(report)
    }
    
    fn analyze_metrics_against_targets(&self, report: &MetricsReport) -> Result<MetricsAnalysis> {
        let targets = self.business_logic.get_metric_targets();
        let mut analysis = MetricsAnalysis::new();
        
        // Buffer analysis
        if let Some(buffer_duration) = report.get_metric("buffer_duration_h") {
            let target_range = targets.buffer_duration_range;
            if buffer_duration < target_range.min {
                analysis.add_issue(MetricIssue::BufferTooLow {
                    current: buffer_duration,
                    target_min: target_range.min,
                });
            } else if buffer_duration > target_range.max {
                analysis.add_issue(MetricIssue::BufferTooHigh {
                    current: buffer_duration,
                    target_max: target_range.max,
                });
            }
        }
        
        // Selection entropy analysis
        if let Some(entropy) = report.get_metric("selection_entropy") {
            if entropy < targets.min_selection_entropy {
                analysis.add_issue(MetricIssue::LowDiversity {
                    current: entropy,
                    target: targets.min_selection_entropy,
                });
                
                // Suggest business logic adjustment
                analysis.suggest_bl_adjustment(BlAdjustmentSuggestion::IncreaseTemperature {
                    current_temp: self.business_logic.selection_temperature(),
                    suggested_temp: (self.business_logic.selection_temperature() * 1.1).min(2.0),
                });
            }
        }
        
        // Quality distribution analysis
        if let Some(quality_dist) = report.get_metric("quality_tier_distribution") {
            let target_dist = targets.quality_tier_distribution;
            if (quality_dist - target_dist).abs() > 0.15 {
                analysis.add_issue(MetricIssue::QualityDistributionSkewed {
                    current: quality_dist,
                    target: target_dist,
                });
            }
        }
        
        Ok(analysis)
    }
}
```

* * *

10) Procedimentos Manuais de Operador com Business Logic
--------------------------------------------------------

1.  **STOP STREAM:** `sudo /vvtv/system/bin/halt_stream.sh --preserve-business-logic` (graceful com contexto).
2.  **INSPECIONAR FILA:** `vvtvctl queue status --show-business-logic --format table`.
3.  **FORÇAR BUFFER:** `/vvtv/system/bin/fill_buffer.sh --target-hours ${BL_BUFFER_TARGET} --quality-tier ${BL_PREFERRED_TIER}`.
4.  **AJUSTAR BUSINESS LOGIC:** `vvtvctl business-logic adjust --temperature 0.9 --music-ratio 0.12 --dry-run`.
5.  **LIMPAR ARQUIVOS VELHOS:** `find /vvtv/storage/archive -mtime +${BL_ARCHIVE_RETENTION_DAYS} -delete`.
6.  **REINICIAR WATCHDOG:** `sudo service intelligent_watchdogd restart`.
7.  **ANALISAR MÉTRICAS:** `vvtvctl metrics analyze --last 24h --compare-to-targets`.
8.  **CURATOR STATUS:** `vvtvctl curator status --show-interventions --show-token-bucket`.

**Enhanced Operator Commands:**

```bash
# Business Logic Operations
vvtvctl business-logic show --detailed
vvtvctl business-logic validate --file /path/to/new_config.yaml
vvtvctl business-logic reload --confirm
vvtvctl business-logic test-selection --plans 20 --show-rationale

# Queue Management with Business Logic
vvtvctl queue status --show-adaptive-weights
vvtvctl queue reorder --use-curator --dry-run
vvtvctl queue optimize --target-diversity 0.8
vvtvctl queue emergency-fill --hours 6

# Adaptive System Control
vvtvctl adaptive status --show-adjustments
vvtvctl adaptive reset --component selection
vvtvctl adaptive tune --metric retention_30min --target 0.75

# Quality and Performance
vvtvctl quality analyze --last 4h --show-trends
vvtvctl performance optimize --cpu-target 70 --quality-tier high
vvtvctl failover test --duration 30s --validate-recovery
```

* * *

11) Ambiente Físico de Exibição Inteligente
-------------------------------------------

*   Monitores em loop: TV OLED 42″ + HDMI direto do Mac Mini.
*   Brilho adaptativo: 60-80% baseado em business logic e hora do dia.
*   Som mutado com monitoramento de áudio via software.
*   LEDs adaptativos: azuis = stream ok; verdes = high quality; amarelos = adaptive mode; vermelhos = falha.
*   Botão físico "RESET STREAM" (aciona GPIO + script de restart com business logic preservation).
*   **Dashboard inteligente**: mostra métricas de business logic, adaptive adjustments, curator status.
*   Operador em plantão usa luvas antirreflexo cinza-claro e unhas grafite fosco (para não gerar reflexos nas telas quando faz manutenção ao vivo).
*   **Ambient feedback**: iluminação do ambiente se adapta ao mood do conteúdo sendo transmitido (opcional, configurável via business logic).

**Smart Environment Controller:**

```rust
pub struct SmartEnvironmentController {
    business_logic: Arc<BusinessLogic>,
    display_controller: DisplayController,
    led_controller: LedController,
    ambient_controller: Option<AmbientController>,
}

impl SmartEnvironmentController {
    pub async fn update_environment(&self, current_content: &ContentInfo) -> Result<()> {
        let env_config = self.business_logic.get_environment_config();
        
        // Update display brightness based on time and content
        let brightness = self.calculate_adaptive_brightness(current_content).await?;
        self.display_controller.set_brightness(brightness).await?;
        
        // Update LED status based on system health
        let system_health = self.get_system_health().await?;
        let led_pattern = self.determine_led_pattern(&system_health);
        self.led_controller.set_pattern(led_pattern).await?;
        
        // Update ambient lighting if enabled
        if let Some(ambient) = &self.ambient_controller {
            if env_config.ambient_lighting_enabled {
                let ambient_config = self.calculate_ambient_config(current_content);
                ambient.apply_config(ambient_config).await?;
            }
        }
        
        Ok(())
    }
    
    fn calculate_adaptive_brightness(&self, content: &ContentInfo) -> Result<f32> {
        let base_brightness = 0.7; // 70% base
        let time_adjustment = self.get_time_based_brightness_adjustment();
        let content_adjustment = match content.mood.as_str() {
            "calm" | "intimate" => -0.1,
            "energetic" | "bright" => 0.1,
            _ => 0.0,
        };
        
        let final_brightness = (base_brightness + time_adjustment + content_adjustment)
            .max(0.4)
            .min(1.0);
        
        Ok(final_brightness)
    }
    
    fn determine_led_pattern(&self, health: &SystemHealth) -> LedPattern {
        match health.overall_status {
            SystemStatus::Optimal => LedPattern::SteadyBlue,
            SystemStatus::HighQuality => LedPattern::SteadyGreen,
            SystemStatus::Adaptive => LedPattern::PulsingYellow,
            SystemStatus::Degraded => LedPattern::SlowBlinkingYellow,
            SystemStatus::Critical => LedPattern::FastBlinkingRed,
            SystemStatus::Failed => LedPattern::SteadyRed,
        }
    }
}
```

* * *

12) Conclusão do Bloco IV
-------------------------

Este bloco estabelece o **sistema circulatório inteligente** do VVTV: a fila adaptativa, o ritmo de exibição baseado em business logic, e a autocorreção constante com aprendizado.  

Com os módulos de browser, processor, business logic e broadcaster integrados, a máquina pode funcionar sozinha por meses sem intervenção humana, adaptando-se continuamente às condições de audiência, qualidade de sistema e objetivos de negócio.

O sistema não apenas mantém a transmissão - ele aprende, evolui e otimiza sua performance automaticamente, mantendo sempre o equilíbrio entre qualidade técnica, diversidade de conteúdo e satisfação da audiência.

* * *🧠 V
VTV INDUSTRIAL DOSSIER
==========================

**Bloco V — Quality Control & Visual Consistency**
--------------------------------------------------

_(padrões de imagem, curva de loudness, cortes automáticos, métricas perceptuais e coerência estética no streaming remoto com IA)_

* * *

### 0\. Objetivo

Garantir **padrão técnico e sensorial contínuo** na transmissão global via link público (HLS/RTMP).  
Todo espectador, independentemente da casa, deve perceber uma imagem limpa, ritmo suave, áudio balanceado e **identidade estética VoulezVous** persistente, mesmo com vídeos de origens distintas.

O sistema integra business logic para thresholds adaptativos, LLM para análise estética, e Curator Vigilante para intervenções de qualidade.

* * *

1) Pipeline de Qualidade — Fases Inteligentes
---------------------------------------------

1.  **Pré-QC** — verificação técnica após transcode (bitrate, codecs, duração) com thresholds de business logic.
2.  **Mid-QC** — checagem perceptual (ruído, saturação, flicker, loudness) com VMAF/SSIM adaptativos.
3.  **Aesthetic-QC** — consistência cromática e identidade com análise LLM opcional.
4.  **Live-QC** — monitoramento do stream ativo (telemetria e capturas) com feedback adaptativo.
5.  **Curator-QC** — revisão inteligente por Curator Vigilante quando necessário.

* * *

2) Pré-QC (Verificação Técnica) com Business Logic
--------------------------------------------------

### 2.1 ffprobe automático adaptativo

Cada vídeo no `/vvtv/storage/ready/<plan_id>/` passa por validação com thresholds configuráveis:

```bash
ffprobe -hide_banner -v error -show_streams -show_format -of json master.mp4 > qc_pre.json
```

Campos avaliados com business logic:

*   Resolução (≥ threshold configurável, padrão 720p, preferido 1080p)
*   Framerate (≈ 23–30 fps estável, tolerância configurável)
*   Codec (`avc1`, `aac`) com fallbacks permitidos
*   Duração coerente (± tolerância % configurável)
*   Bitrate nominal dentro da faixa configurável

### 2.2 Thresholds adaptativos de erro

```rust
pub struct AdaptiveQcThresholds {
    business_logic: Arc<BusinessLogic>,
}

impl AdaptiveQcThresholds {
    pub fn get_thresholds_for_content(&self, content: &ContentInfo) -> QcThresholds {
        let base_thresholds = self.business_logic.get_base_qc_thresholds();
        
        // Adapt based on content priority
        let priority_multiplier = match content.priority {
            ContentPriority::High => 1.2,      // Stricter thresholds
            ContentPriority::Standard => 1.0,   // Base thresholds
            ContentPriority::Emergency => 0.8,  // Relaxed thresholds
        };
        
        // Adapt based on system load
        let load_multiplier = if self.get_system_load() > 0.8 {
            0.9  // Slightly relaxed under high load
        } else {
            1.0
        };
        
        QcThresholds {
            fps_tolerance: base_thresholds.fps_tolerance * priority_multiplier * load_multiplier,
            bitrate_range: base_thresholds.bitrate_range.scale(priority_multiplier),
            lufs_tolerance: base_thresholds.lufs_tolerance * priority_multiplier,
            vmaf_min: base_thresholds.vmaf_min * priority_multiplier,
            ssim_min: base_thresholds.ssim_min * priority_multiplier,
        }
    }
}
```

Falhas → reencode automático com parâmetros ajustados ou escalation para Curator.

* * *3) M
id-QC (Perceptual) com IA
-----------------------------

### 3.1 Análise de ruído e flicker com thresholds adaptativos

Algoritmo VMAF + SSIM com referência neutra e thresholds de business logic:

```bash
ffmpeg -i master.mp4 -i reference.mp4 \
  -lavfi "ssim;[0:v][1:v]libvmaf=model_path=vmaf_v0.6.1.json:log_path=vmaf_log.json" \
  -f null -
```

Rejeitar vídeos com thresholds adaptativos:
*   SSIM < threshold configurável (padrão 0.92, pode ser 0.88-0.95)
*   VMAF < threshold configurável (padrão 85, pode ser 80-90)

### 3.2 Detecção de black frames ou stuck frames

```bash
ffmpeg -i master.mp4 \
  -vf "blackdetect=d=${BL_BLACK_DETECT_DURATION}:pix_th=${BL_BLACK_DETECT_THRESHOLD}" \
  -f null -
```

→ se > threshold % do total de frames = black, marcar `qc_warning` ou `qc_fail` baseado em business logic.

### 3.3 Pico de áudio e ruído com análise adaptativa

FFT + RMS com thresholds configuráveis:

```bash
ffmpeg -i master.mp4 \
  -af "astats=metadata=1:reset=1,loudnorm=I=${BL_TARGET_LUFS}:print_format=summary" \
  -f null -
```

Ações baseadas em business logic:
*   Picos > threshold dB → compressão adicional ou rejeição
*   RMS < threshold dB → ganho automático ou escalation
*   LUFS fora da tolerância → renormalização ou aprovação condicional

* * *

4) Aesthetic-QC (Identidade VoulezVous) com LLM
-----------------------------------------------

Mesmo sendo conteúdo variado, o canal precisa manter **uma assinatura sensorial** com análise inteligente.

### 4.1 Paleta cromática e temperatura com IA

O motor extrai 5 cores dominantes por vídeo e analisa com LLM opcional:

```bash
ffmpeg -i master.mp4 -vf "palettegen=max_colors=5" palette.png
```

**LLM Aesthetic Analysis:**

```rust
pub async fn analyze_aesthetic_quality(
    &self,
    video_path: &str,
    palette_path: &str,
) -> Result<AestheticAnalysis> {
    if !self.llm_enabled || self.circuit_breaker.is_open() {
        return Ok(AestheticAnalysis::default());
    }
    
    let color_analysis = self.extract_color_metrics(palette_path)?;
    let visual_features = self.extract_visual_features(video_path)?;
    
    let request = LlmRequest {
        prompt: format!(
            "Analyze this video's aesthetic quality for VoulezVous brand:\n\
            Dominant colors: {:?}\n\
            Brightness avg: {:.2}\n\
            Contrast ratio: {:.2}\n\
            Saturation avg: {:.2}\n\
            \n\
            VoulezVous brand guidelines:\n\
            - Warm, intimate atmosphere (temperature 4000-6500K)\n\
            - Subtle magenta/amber tones preferred\n\
            - Avoid harsh greens or cold blues\n\
            - Saturation 0.6-0.8 (vivid but not neon)\n\
            - Contrast 1.0-1.2 (gentle enhancement)\n\
            \n\
            Rate aesthetic fit (0-1), suggest corrections if needed, classify mood.",
            color_analysis.dominant_colors,
            visual_features.brightness_avg,
            visual_features.contrast_ratio,
            visual_features.saturation_avg
        ),
        max_tokens: 200,
        temperature: 0.3,
    };
    
    match timeout(Duration::from_secs(5), self.llm_client.analyze(request)).await {
        Ok(Ok(analysis)) => {
            self.circuit_breaker.record_success();
            Ok(analysis)
        }
        Ok(Err(e)) => {
            self.circuit_breaker.record_failure();
            warn!("LLM aesthetic analysis failed: {}", e);
            Ok(AestheticAnalysis::fallback_analysis(&color_analysis, &visual_features))
        }
        Err(_) => {
            self.circuit_breaker.record_failure();
            warn!("LLM aesthetic analysis timeout");
            Ok(AestheticAnalysis::fallback_analysis(&color_analysis, &visual_features))
        }
    }
}
```

Regras com business logic:
*   Temperatura entre range configurável (padrão 4000 K - 6500 K)
*   Evitar tons esverdeados; priorizar magenta, âmbar, bege, e bronze
*   Saturação média configurável (padrão 0.6 – 0.8)
*   Preto nunca absoluto (mínimo luma configurável, padrão 0.02)

### 4.2 Correção automática adaptativa

```bash
ffmpeg -i master.mp4 \
  -vf "eq=contrast=${BL_CONTRAST_ADJUSTMENT}:saturation=${BL_SATURATION_ADJUSTMENT}:gamma=${BL_GAMMA_ADJUSTMENT}" \
  output.mp4
```

Ajuste adaptativo baseado em:
*   Análise LLM (se disponível)
*   Business logic preferences
*   Histórico de correções bem-sucedidas
*   Feedback do Curator Vigilante

* * *5) Lo
udness e Curva Sonora Global Adaptativa
--------------------------------------------

Todos os vídeos do canal precisam **soar como um único programa** com parâmetros adaptativos.

**Normalização absoluta** com target configurável + **curva de equalização** adaptativa:

```bash
ffmpeg -i master_normalized.mp4 \
  -af "firequalizer=gain_entry='entry(31,${BL_EQ_31HZ});entry(250,${BL_EQ_250HZ});entry(4000,${BL_EQ_4KHZ});entry(10000,${BL_EQ_10KHZ})':gain_scale=linear" \
  -c:v copy -c:a aac -b:a ${BL_AUDIO_BITRATE} final.mp4
```

Resultado adaptativo:
*   Target LUFS configurável (padrão -14, pode ser -12 a -16)
*   EQ curve baseada em business logic e análise de audiência
*   Sem agudos agressivos (threshold configurável)
*   Sem subgrave de distorção (threshold configurável)
*   Sem jumps entre clipes (crossfade automático)

* * *

6) Transições e continuidade inteligente
----------------------------------------

### 6.1 Fade computável adaptativo

Entre vídeos, **fade** com duração configurável via business logic:

```bash
ffmpeg -i prev.mp4 -i next.mp4 -filter_complex \
"[0:v]fade=t=out:st=4.6:d=${BL_FADE_DURATION}[v0];[1:v]fade=t=in:st=0:d=${BL_FADE_DURATION}[v1];[v0][v1]concat=n=2:v=1:a=0[v]" \
-map "[v]" -c:v libx264 output.mp4
```

### 6.2 Crossfade de áudio adaptativo

```bash
-af "acrossfade=d=${BL_CROSSFADE_DURATION}:c1=${BL_CROSSFADE_CURVE1}:c2=${BL_CROSSFADE_CURVE2}"
```

Parâmetros adaptativos baseados em:
*   Mood do conteúdo (calm = fade longo, energetic = fade curto)
*   Business logic preferences
*   Análise LLM de compatibilidade entre conteúdos adjacentes

* * *

7) Monitoramento em produção (Live-QC) inteligente
--------------------------------------------------

### 7.1 Captura periódica do streaming público com análise

O sistema acessa o **mesmo link HLS/RTMP que o público vê** com frequência adaptativa:

```
https://voulezvous.tv/live.m3u8
```

Intervalo baseado em business logic (padrão 5 min, pode ser 1-15 min):

*   `ffprobe` → checa bitrate, fps, resolução contra thresholds
*   Captura frame atual e salva: `/vvtv/monitor/captures/<timestamp>.jpg`
*   FFT do áudio → monitora pico e ruído contra limites configuráveis
*   **LLM analysis** (opcional): verifica qualidade visual do frame capturado

### 7.2 Telemetria adaptativa

Registra métricas com thresholds de business logic:

```rust
pub struct LiveQcMetrics {
    pub stream_bitrate_mbps: f64,
    pub audio_peak_db: f64,
    pub audio_lufs: f64,
    pub uptime_hours: f64,
    pub vmaf_live: f64,
    pub avg_latency_s: f64,
    pub business_logic_version: String,
    pub quality_tier_active: String,
    pub adaptive_adjustments_count: u32,
}

impl LiveQcMetrics {
    pub fn evaluate_against_targets(&self, business_logic: &BusinessLogic) -> QcEvaluation {
        let targets = business_logic.get_live_qc_targets();
        let mut evaluation = QcEvaluation::new();
        
        // Bitrate evaluation
        if self.stream_bitrate_mbps < targets.bitrate_range.min {
            evaluation.add_issue(QcIssue::BitrateToolow {
                current: self.stream_bitrate_mbps,
                target_min: targets.bitrate_range.min,
            });
        } else if self.stream_bitrate_mbps > targets.bitrate_range.max {
            evaluation.add_issue(QcIssue::BitrateTooHigh {
                current: self.stream_bitrate_mbps,
                target_max: targets.bitrate_range.max,
            });
        }
        
        // Audio evaluation
        if self.audio_peak_db > targets.max_audio_peak_db {
            evaluation.add_issue(QcIssue::AudioPeakTooHigh {
                current: self.audio_peak_db,
                target_max: targets.max_audio_peak_db,
            });
        }
        
        let lufs_diff = (self.audio_lufs - targets.target_lufs).abs();
        if lufs_diff > targets.lufs_tolerance {
            evaluation.add_issue(QcIssue::LufsOutOfRange {
                current: self.audio_lufs,
                target: targets.target_lufs,
                tolerance: targets.lufs_tolerance,
            });
        }
        
        // Quality evaluation
        if self.vmaf_live < targets.min_vmaf_live {
            evaluation.add_issue(QcIssue::VmafTooLow {
                current: self.vmaf_live,
                target_min: targets.min_vmaf_live,
            });
        }
        
        // Latency evaluation
        if self.avg_latency_s > targets.max_latency_s {
            evaluation.add_issue(QcIssue::LatencyTooHigh {
                current: self.avg_latency_s,
                target_max: targets.max_latency_s,
            });
        }
        
        evaluation
    }
}
```

Resultados plotados no **Dashboard Local** (`/vvtv/monitor/dashboard.html`) com contexto de business logic.

* * *

8) Reação Automática a Problemas com IA
---------------------------------------

| Falha detectada | Ação Padrão | Ação com Business Logic | LLM Enhancement |
| --- | --- | --- | --- |
| Bitrate caiu < threshold | Reiniciar playout encoder | Ajustar quality tier automaticamente | Analisar causa raiz |
| Resolução < threshold | Pular para próximo item | Verificar se é aceitável para emergency mode | Sugerir correções |
| VMAF < threshold em N amostras | Reprocessar vídeo | Ajustar thresholds ou aceitar com warning | Análise de qualidade visual |
| Loudness > threshold LUFS | Aplicar compressão | Ajustar target LUFS dinamicamente | Detectar padrões de áudio |
| Freeze de frame > threshold s | Recarregar stream | Failover inteligente com quality preservation | Análise de estabilidade |

**Intelligent Problem Resolution:**

```rust
pub struct IntelligentProblemResolver {
    business_logic: Arc<BusinessLogic>,
    llm_analyzer: Option<LlmAnalyzer>,
    action_history: ActionHistory,
}

impl IntelligentProblemResolver {
    pub async fn resolve_qc_issue(&self, issue: &QcIssue) -> Result<ResolutionAction> {
        // Get business logic guidance
        let bl_guidance = self.business_logic.get_resolution_guidance(issue);
        
        // Check action history for patterns
        let historical_success = self.action_history.get_success_rate_for_issue_type(&issue.issue_type);
        
        // Get LLM analysis if available and issue is complex
        let llm_analysis = if self.should_use_llm_for_issue(issue) {
            self.llm_analyzer.as_ref()
                .and_then(|analyzer| analyzer.analyze_qc_issue(issue).await.ok())
        } else {
            None
        };
        
        // Determine best action
        let action = match issue {
            QcIssue::BitrateToolow { current, target_min } => {
                if bl_guidance.allow_quality_degradation && historical_success > 0.8 {
                    ResolutionAction::AdjustEncodingParams {
                        target_bitrate: *target_min,
                        preserve_quality: false,
                    }
                } else {
                    ResolutionAction::RestartEncoder
                }
            }
            QcIssue::VmafTooLow { current, target_min } => {
                if let Some(analysis) = &llm_analysis {
                    if analysis.suggests_reprocessing {
                        ResolutionAction::ReprocessWithHigherQuality
                    } else if analysis.suggests_acceptance {
                        ResolutionAction::AcceptWithWarning {
                            reason: analysis.acceptance_reason.clone(),
                        }
                    } else {
                        ResolutionAction::SkipContent
                    }
                } else {
                    // Fallback to business logic
                    if *current >= (*target_min * bl_guidance.quality_tolerance) {
                        ResolutionAction::AcceptWithWarning {
                            reason: "Within business logic tolerance".to_string(),
                        }
                    } else {
                        ResolutionAction::ReprocessWithHigherQuality
                    }
                }
            }
            _ => {
                ResolutionAction::EscalateToOperator
            }
        };
        
        // Record action for future learning
        self.action_history.record_action(issue, &action).await?;
        
        Ok(action)
    }
}
```

* * *9) Teste Vi
sual Periódico (Operator Mode) com IA
------------------------------------------------

A cada intervalo configurável (padrão 24 h, pode ser 6-72 h) o sistema mostra localmente uma sequência de amostras capturadas do stream real com análise inteligente.

**Automated Visual Assessment:**

```rust
pub async fn run_automated_visual_assessment(&self) -> Result<VisualAssessmentReport> {
    let assessment_config = self.business_logic.get_visual_assessment_config();
    let samples = self.collect_stream_samples(assessment_config.sample_count).await?;
    
    let mut report = VisualAssessmentReport::new();
    
    for sample in samples {
        // Technical analysis
        let technical_metrics = self.analyze_technical_quality(&sample).await?;
        
        // LLM aesthetic analysis (if enabled)
        let aesthetic_analysis = if assessment_config.llm_analysis_enabled {
            self.llm_analyzer.analyze_visual_sample(&sample).await.ok()
        } else {
            None
        };
        
        // Curator review (if configured)
        let curator_review = if assessment_config.curator_review_enabled {
            self.curator_vigilante.review_visual_sample(&sample).await.ok()
        } else {
            None
        };
        
        let sample_assessment = SampleAssessment {
            timestamp: sample.timestamp,
            technical_metrics,
            aesthetic_analysis,
            curator_review,
            overall_score: self.calculate_overall_score(&technical_metrics, &aesthetic_analysis, &curator_review),
        };
        
        report.add_sample_assessment(sample_assessment);
    }
    
    // Generate recommendations
    report.generate_recommendations(&self.business_logic);
    
    Ok(report)
}
```

**Assessment Questions (Automated + Optional Human):**

1.  **Brilho** está consistente? (automated via histogram analysis + optional LLM)
2.  **Cores** dentro do perfil VV? (automated via color space analysis + LLM aesthetic check)
3.  **Corte** suave entre vídeos? (automated via transition analysis)
4.  **Som** uniforme? (automated via loudness analysis)
5.  **Foco humano** (movimento, nitidez) mantido? (LLM visual analysis)
6.  **Sensação geral** (intimidade, calor, continuidade)? (LLM mood analysis + curator review)

Respostas alimentam um log qualitativo (`qc_aesthetic_score`) que ajusta o "curation score" futuro e parâmetros de business logic.

* * *

10) Relatório Global de Qualidade com IA
----------------------------------------

Gerado a cada intervalo configurável (padrão 24 h):

```json
{
  "report_timestamp": "2025-10-22T00:00:00Z",
  "business_logic_version": "2025.10",
  "assessment_period_hours": 24,
  "total_videos_processed": 48,
  "quality_metrics": {
    "passed": 45,
    "failed": 3,
    "avg_vmaf": 91.2,
    "avg_ssim": 0.94,
    "avg_loudness_lufs": -14.1,
    "avg_temp_k": 5100
  },
  "aesthetic_analysis": {
    "signature_deviation": 0.07,
    "llm_analysis_count": 42,
    "llm_success_rate": 0.95,
    "aesthetic_score_avg": 0.83
  },
  "adaptive_adjustments": {
    "quality_tier_changes": 3,
    "threshold_adjustments": 1,
    "emergency_mode_activations": 0
  },
  "curator_interventions": {
    "total_reviews": 8,
    "interventions_applied": 2,
    "token_bucket_utilization": 0.33
  },
  "recommendations": [
    "Consider increasing VMAF threshold to 87 for higher quality",
    "LLM aesthetic analysis showing consistent brand alignment",
    "Curator intervention rate within optimal range"
  ]
}
```

Se `signature_deviation > threshold` configurável, sinaliza "drift estético" → revisão manual ou ajuste automático de business logic.

* * *

11) Identidade e Branding Subconsciente com IA
----------------------------------------------

*   Todos os vídeos devem compartilhar **leve tonalidade âmbar ou magenta** (configurável).
*   Transições suaves, sem logos fixos.
*   A textura de luz deve parecer **"quente, íntima e calma"** (verificável via LLM).
*   Nenhum clipe deve parecer abrupto, frio ou mecânico (detectável via análise automática).
*   **LLM Brand Consistency Check**: análise contínua de aderência à identidade visual.

**Brand Consistency Monitor:**

```rust
pub struct BrandConsistencyMonitor {
    business_logic: Arc<BusinessLogic>,
    llm_analyzer: Option<LlmAnalyzer>,
    brand_profile: VvBrandProfile,
}

impl BrandConsistencyMonitor {
    pub async fn check_brand_consistency(&self, content_batch: &[ContentSample]) -> Result<BrandConsistencyReport> {
        let brand_config = self.business_logic.get_brand_consistency_config();
        let mut report = BrandConsistencyReport::new();
        
        for sample in content_batch {
            // Technical brand metrics
            let color_analysis = self.analyze_color_consistency(sample).await?;
            let mood_analysis = self.analyze_mood_consistency(sample).await?;
            
            // LLM brand analysis (if enabled)
            let llm_brand_analysis = if brand_config.llm_analysis_enabled {
                self.llm_analyzer.as_ref()
                    .and_then(|analyzer| analyzer.analyze_brand_consistency(sample, &self.brand_profile).await.ok())
            } else {
                None
            };
            
            let consistency_score = self.calculate_brand_consistency_score(
                &color_analysis,
                &mood_analysis,
                &llm_brand_analysis,
            );
            
            report.add_sample_score(sample.id.clone(), consistency_score);
            
            // Flag deviations
            if consistency_score < brand_config.min_consistency_score {
                report.add_deviation(BrandDeviation {
                    sample_id: sample.id.clone(),
                    deviation_type: self.classify_deviation(&color_analysis, &mood_analysis),
                    severity: self.calculate_deviation_severity(consistency_score, brand_config.min_consistency_score),
                    suggested_correction: llm_brand_analysis.as_ref()
                        .and_then(|analysis| analysis.suggested_correction.clone()),
                });
            }
        }
        
        // Generate overall brand health assessment
        report.calculate_overall_brand_health();
        
        Ok(report)
    }
}
```

Essa coesão é o que cria a "experiência VoulezVous" — o espectador não percebe conscientemente, mas sente a consistência.

* * *

12) Conclusão do Bloco V
------------------------

O **Bloco V** transforma a transmissão num **organismo sensorial coerente e inteligente**.  

Cada visitante que abre o link público do streaming — seja em Lisboa, São Paulo ou Tóquio — recebe a mesma sensação calibrada e contínua: calor, fluidez, cor de âmbar e áudio uniforme, mantidos através de:

- **QC automatizado** com thresholds adaptativos baseados em business logic
- **Análise LLM** para consistência estética e brand compliance
- **Curator Vigilante** para intervenções inteligentes de qualidade
- **Monitoramento em tempo real** com feedback adaptativo
- **Correções automáticas** baseadas em padrões aprendidos

Com o QC automatizado e o monitoramento inteligente em tempo real, o canal pode operar **24 h por dia**, **365 dias por ano**, mantendo o **nível técnico e estético industrial VoulezVous.TV** enquanto se adapta continuamente às condições e aprende com cada decisão de qualidade.

* * *🧠 VVTV INDU
STRIAL DOSSIER
==========================

**Bloco VI — Distribution, Redundancy & CDN Layer**
---------------------------------------------------

_(stream global, latência mínima, redundância computável, observabilidade e arquitetura de continuidade planetária VoulezVous.TV com business logic)_

* * *

### 0\. Propósito do Bloco

Definir a camada de **distribuição global e redundância industrial inteligente** para o canal VoulezVous.TV:  
assegurar **streaming 24/7**, latência < 5 s no público, **retransmissão auditável** e **resiliência multinó**, sem depender de provedores únicos.

O sistema integra business logic para otimização adaptativa de CDN, LLM para análise de performance global, e Curator Vigilante para monitoramento de qualidade de distribuição.

O princípio aqui é simples: o canal deve **nunca cair** e sempre **adaptar sua distribuição** baseado em condições reais.  
Se Lisboa apagar, Tóquio transmite com parâmetros otimizados.  
Se a Cloudflare sumir, o nó Railway sobe a origin secundária com configuração adaptativa.  
Se tudo falhar, o último Mac Mini reativa o stream a partir do cache local com business logic de emergência.

* * *

1) Arquitetura de Distribuição Global Inteligente
-------------------------------------------------

### 1.1 Topologia Geral Adaptativa

```
                +------------------+
                |  LogLine Control  |
                |  Plane + BL      |
                +--------+----------+
                         |
                +--------+--------+
                |                 |
     +----------v-----+   +-------v----------+
     | Primary Origin |   | Secondary Origin |
     | Lisboa / M1-MM |   | Railway Node     |
     | + Business     |   | + BL Sync        |
     | Logic Authority|   |                  |
     +--------+-------+   +---------+--------+
              |                     |
      +-------v------+      +-------v------+
      | CDN Layer A  |      | CDN Layer B  |
      | (Cloudflare) |      | (Backblaze)  |
      | + Adaptive   |      | + Failover   |
      +-------+------+      +-------+------+
              |                     |
    +---------v---------+   +-------v----------+
    | Global HLS Edges  |   | Backup HLS Edges |
    | + Quality Adapt   |   | + Emergency Mode |
    +---------+----------+  +------------------+
              |
        Viewers Worldwide
        (Adaptive Quality)
```

*   **Primary Origin:** Mac Mini Lisboa — RTMP + HLS local, autoridade de business logic.
*   **Secondary Origin:** Railway (cloud) — failover + replicador com sync de business logic.
*   **CDN Layer A/B:** múltiplos provedores com configuração adaptativa.
*   **Edges:** 12–24 nós globais servindo HLS via HTTPS com otimização baseada em business logic.

### 1.2 Business Logic Distribution

Cada nó na rede mantém uma cópia sincronizada do business logic para decisões locais:

```rust
pub struct DistributedBusinessLogic {
    local_config: Arc<BusinessLogic>,
    sync_manager: BusinessLogicSyncManager,
    edge_optimizer: EdgeOptimizer,
}

impl DistributedBusinessLogic {
    pub async fn sync_with_authority(&self) -> Result<SyncResult> {
        let authority_config = self.sync_manager.fetch_from_authority().await?;
        
        if authority_config.version > self.local_config.version() {
            // Update local config
            self.update_local_config(authority_config).await?;
            
            // Adapt edge configuration
            self.edge_optimizer.adapt_to_new_config(&authority_config).await?;
            
            Ok(SyncResult::Updated {
                old_version: self.local_config.version(),
                new_version: authority_config.version,
            })
        } else {
            Ok(SyncResult::UpToDate)
        }
    }
    
    pub fn get_adaptive_cdn_config(&self, region: &str, load_metrics: &LoadMetrics) -> CdnConfig {
        let base_config = self.local_config.get_cdn_config();
        
        // Adapt based on region
        let regional_adjustments = match region {
            "US" | "CA" => CdnAdjustments {
                cache_ttl_multiplier: 1.2,
                bandwidth_priority: BandwidthPriority::High,
            },
            "BR" | "PT" => CdnAdjustments {
                cache_ttl_multiplier: 1.0,
                bandwidth_priority: BandwidthPriority::Standard,
            },
            "JP" | "AU" => CdnAdjustments {
                cache_ttl_multiplier: 0.8,
                bandwidth_priority: BandwidthPriority::Low,
            },
            _ => CdnAdjustments::default(),
        };
        
        // Adapt based on load
        let load_adjustments = if load_metrics.cpu_usage > 0.8 {
            CdnAdjustments {
                quality_tier_preference: QualityTier::Standard,
                segment_cache_aggressive: true,
            }
        } else {
            CdnAdjustments {
                quality_tier_preference: QualityTier::High,
                segment_cache_aggressive: false,
            }
        };
        
        base_config.apply_adjustments(&regional_adjustments).apply_adjustments(&load_adjustments)
    }
}
```

* * *

2) Tipos de Saída do Stream Adaptativos
---------------------------------------

| Saída | Protocolo | Uso | Destino | Business Logic Integration |
| --- | --- | --- | --- | --- |
| `rtmp://voulezvous.ts.net/live/main` | RTMP | ingestão primária | Origin | Bitrate adaptativo |
| `/live.m3u8` | HLS | principal público | CDN | Quality tier baseado em BL |
| `/live_low.m3u8` | HLS (480p) | fallback mobile | CDN | Emergency mode support |
| `/live_adaptive.m3u8` | HLS ABR | adaptive bitrate | CDN | Multi-tier baseado em BL |
| `/manifest.json` | JSON API | automação / players | CDN | BL metadata included |
| `/thumbs/<t>.jpg` | JPEG | preview / métricas | monitoramento | Quality based on BL |
| `/business-logic-status` | JSON | BL sync status | internal | Version and health |

**Adaptive Stream Generation:**

```rust
pub struct AdaptiveStreamGenerator {
    business_logic: Arc<BusinessLogic>,
    encoder_controller: EncoderController,
    quality_monitor: QualityMonitor,
}

impl AdaptiveStreamGenerator {
    pub async fn generate_adaptive_streams(&self, source: &StreamSource) -> Result<Vec<StreamOutput>> {
        let stream_config = self.business_logic.get_adaptive_stream_config();
        let current_quality = self.quality_monitor.get_current_metrics().await?;
        
        let mut outputs = Vec::new();
        
        // Generate quality tiers based on business logic
        for tier in stream_config.quality_tiers {
            let adapted_params = self.adapt_encoding_params(&tier, &current_quality)?;
            
            let output = StreamOutput {
                name: format!("live_{}.m3u8", tier.name.to_lowercase()),
                encoding_params: adapted_params,
                target_bitrate: tier.bitrate,
                resolution: tier.resolution,
                business_logic_version: self.business_logic.version(),
            };
            
            outputs.push(output);
        }
        
        // Add emergency fallback stream
        if stream_config.enable_emergency_stream {
            outputs.push(self.generate_emergency_stream()?);
        }
        
        Ok(outputs)
    }
}
```

* * *

3) Replicação Origin–Backup Inteligente
---------------------------------------

**Ferramenta:** `rclone + ffmpeg + rsync` com business logic integration.  
Sincronização adaptativa baseada em prioridade e condições de rede.

**Rotina Inteligente:**

```bash
# Adaptive sync with business logic parameters
rclone sync /vvtv/broadcast/hls railway:vv_origin/ \
  --bwlimit ${BL_SYNC_BANDWIDTH_LIMIT} \
  --fast-list \
  --transfers ${BL_SYNC_TRANSFERS} \
  --checkers ${BL_SYNC_CHECKERS} \
  --exclude-from ${BL_SYNC_EXCLUDE_FILE}
```

Verificação por checksum com business logic context:

```bash
rclone check /vvtv/broadcast/hls railway:vv_origin/ \
  --one-way \
  --size-only=${BL_SYNC_SIZE_ONLY} \
  --max-age ${BL_SYNC_MAX_AGE}
```

Se diferença > threshold configurável, o **Railway assume automaticamente** a origem com parâmetros adaptativos.

**Intelligent Sync Manager:**

```rust
pub struct IntelligentSyncManager {
    business_logic: Arc<BusinessLogic>,
    network_monitor: NetworkMonitor,
    priority_calculator: SyncPriorityCalculator,
}

impl IntelligentSyncManager {
    pub async fn execute_adaptive_sync(&self) -> Result<SyncReport> {
        let sync_config = self.business_logic.get_sync_config();
        let network_conditions = self.network_monitor.get_current_conditions().await?;
        
        // Calculate sync priorities
        let sync_items = self.identify_sync_items().await?;
        let prioritized_items = self.priority_calculator.prioritize(sync_items, &sync_config)?;
        
        // Adapt sync parameters based on network conditions
        let adaptive_params = self.calculate_adaptive_sync_params(&network_conditions, &sync_config)?;
        
        let mut sync_report = SyncReport::new();
        
        for item in prioritized_items {
            match self.sync_item_with_adaptive_params(&item, &adaptive_params).await {
                Ok(item_result) => {
                    sync_report.add_success(item_result);
                }
                Err(e) => {
                    sync_report.add_failure(item.id, e);
                    
                    // Adapt parameters for next item if failure
                    if sync_report.failure_rate() > sync_config.max_failure_rate {
                        adaptive_params.reduce_aggressiveness();
                    }
                }
            }
        }
        
        // Update business logic sync metrics
        self.update_sync_metrics(&sync_report).await?;
        
        Ok(sync_report)
    }
}
```

* * *

4) CDN Layer A (Cloudflare) com Business Logic
----------------------------------------------

### 4.1 Configuração Adaptativa

*   **Domain:** `voulezvous.tv`
*   **Cache TTL:** adaptativo baseado em business logic (m3u8: 30-120s, segmentos: 30min-2h)
*   **Bypass inteligente:** `/live.m3u8` → origin direta com fallback
*   **Edge Workers** com redirecionamento baseado em business logic e métricas:

### 4.2 Worker Script Inteligente

```js
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const clientRegion = request.cf.country;
    const clientBandwidth = request.cf.clientTcpRtt;
    
    // Get business logic config (cached)
    const blConfig = await env.BUSINESS_LOGIC_KV.get('current_config', 'json');
    
    if (url.pathname.endsWith('.m3u8')) {
      // Determine optimal origin based on business logic
      const optimalOrigin = selectOptimalOrigin(clientRegion, clientBandwidth, blConfig);
      url.hostname = optimalOrigin;
      
      // Add business logic headers
      const response = await fetch(url);
      const modifiedResponse = new Response(response.body, response);
      modifiedResponse.headers.set('X-BL-Version', blConfig.version);
      modifiedResponse.headers.set('X-BL-Quality-Tier', determineQualityTier(clientBandwidth, blConfig));
      
      return modifiedResponse;
    }
    
    return fetch(url);
  }
};

function selectOptimalOrigin(region, bandwidth, blConfig) {
  const regionConfig = blConfig.cdn.regions[region] || blConfig.cdn.regions.default;
  
  if (bandwidth < regionConfig.lowBandwidthThreshold) {
    return regionConfig.lowBandwidthOrigin;
  } else if (bandwidth > regionConfig.highBandwidthThreshold) {
    return regionConfig.highBandwidthOrigin;
  } else {
    return regionConfig.standardOrigin;
  }
}
```

* * *5) CDN L
ayer B (Backblaze + Bunny) com Business Logic
-----------------------------------------------------

**Objetivo:** redundância de arquivo estático com otimização inteligente.

*   Upload automático de cada segmento finalizado com priorização baseada em business logic.
*   TTL adaptativo = 7-30 dias baseado em popularidade e business logic; limpeza automática via `manifest.json`.
*   **Quality-aware storage**: segmentos de alta qualidade priorizados para backup.

```bash
# Adaptive backup with business logic priorities
rclone copy /vvtv/broadcast/hls b2:vv_hls_backup/ \
  --transfers ${BL_BACKUP_TRANSFERS} \
  --include="*.m3u8" \
  --include="*_${BL_PRIORITY_QUALITY_TIER}_*.m4s" \
  --max-age ${BL_BACKUP_MAX_AGE}
```

**Intelligent Backup Manager:**

```rust
pub struct IntelligentBackupManager {
    business_logic: Arc<BusinessLogic>,
    storage_optimizer: StorageOptimizer,
    cost_calculator: CostCalculator,
}

impl IntelligentBackupManager {
    pub async fn execute_smart_backup(&self) -> Result<BackupReport> {
        let backup_config = self.business_logic.get_backup_config();
        let storage_metrics = self.storage_optimizer.get_current_metrics().await?;
        
        // Prioritize content based on business logic
        let content_items = self.identify_backup_candidates().await?;
        let prioritized_items = self.prioritize_backup_items(content_items, &backup_config)?;
        
        // Calculate cost-benefit for each item
        let mut backup_plan = Vec::new();
        let mut estimated_cost = 0.0;
        
        for item in prioritized_items {
            let item_cost = self.cost_calculator.calculate_backup_cost(&item)?;
            let item_value = self.calculate_content_value(&item, &backup_config)?;
            
            if item_cost <= backup_config.max_cost_per_item && 
               estimated_cost + item_cost <= backup_config.daily_budget {
                backup_plan.push(item);
                estimated_cost += item_cost;
            }
        }
        
        // Execute backup plan
        let mut report = BackupReport::new();
        for item in backup_plan {
            match self.backup_item(&item).await {
                Ok(result) => report.add_success(result),
                Err(e) => report.add_failure(item.id, e),
            }
        }
        
        Ok(report)
    }
}
```

* * *

6) Propagação Global — Edge Compute Inteligente
-----------------------------------------------

### 6.1 Nó Edge Adaptativo

Cada edge mantém cache inteligente baseado em business logic:

```
/cache/hls/adaptive/
  ├── high_quality/     # Para regiões com boa banda
  ├── standard/         # Para uso geral
  ├── emergency/        # Para modo de emergência
  └── metadata/
      ├── business_logic_cache.json
      ├── regional_preferences.json
      └── performance_metrics.json
```

e executa watchdog local com business logic:

*   se latência > threshold configurável, recarrega playlist;
*   se não houver segmento novo em threshold configurável → switch para backup;
*   **Quality adaptation**: ajusta qualidade baseado em condições locais;
*   **Regional optimization**: adapta cache baseado em preferências regionais.

### 6.2 Auto-Healing Inteligente

Se um edge perder a origem, ele requisita `manifest.json` do LogLine Control Plane com contexto de business logic, que devolve a **melhor nova origem** (`origin_rank`) considerando:

*   Latência de rede
*   Qualidade disponível
*   Business logic preferences
*   Histórico de performance

Atualização ocorre sem interrupção perceptível (buffer local adaptativo de 15-45 s baseado em business logic).

**Smart Edge Controller:**

```rust
pub struct SmartEdgeController {
    business_logic: Arc<BusinessLogic>,
    performance_monitor: PerformanceMonitor,
    cache_optimizer: CacheOptimizer,
    failover_manager: FailoverManager,
}

impl SmartEdgeController {
    pub async fn optimize_edge_performance(&self) -> Result<OptimizationReport> {
        let edge_config = self.business_logic.get_edge_config();
        let current_metrics = self.performance_monitor.get_metrics().await?;
        
        let mut optimizations = Vec::new();
        
        // Cache optimization
        if current_metrics.cache_hit_rate < edge_config.target_cache_hit_rate {
            let cache_optimization = self.cache_optimizer.optimize_cache_strategy(&current_metrics).await?;
            optimizations.push(Optimization::CacheStrategy(cache_optimization));
        }
        
        // Quality tier optimization
        if current_metrics.bandwidth_utilization > edge_config.max_bandwidth_utilization {
            let quality_optimization = self.optimize_quality_tiers(&current_metrics).await?;
            optimizations.push(Optimization::QualityTiers(quality_optimization));
        }
        
        // Origin selection optimization
        if current_metrics.origin_latency > edge_config.max_origin_latency {
            let origin_optimization = self.failover_manager.find_better_origin(&current_metrics).await?;
            optimizations.push(Optimization::OriginSelection(origin_optimization));
        }
        
        // Apply optimizations
        let mut report = OptimizationReport::new();
        for optimization in optimizations {
            match self.apply_optimization(optimization).await {
                Ok(result) => report.add_success(result),
                Err(e) => report.add_failure(e),
            }
        }
        
        Ok(report)
    }
}
```

* * *

7) Controle de Latência Inteligente
-----------------------------------

### 7.1 Medição ativa com business logic

Cada nó edge executa medição adaptativa:

```bash
# Adaptive latency measurement
curl -o /dev/null -s -w "%{time_total}" \
  --max-time ${BL_LATENCY_TIMEOUT} \
  --retry ${BL_LATENCY_RETRIES} \
  https://voulezvous.tv/live.m3u8
```

e grava tempo médio em `/metrics/latency.log` com contexto de business logic.

### 7.2 Objetivo Adaptativo

*   Latência média global: **< threshold configurável** (padrão 5 s, pode ser 3-8 s)
*   Variância < threshold configurável entre regiões (padrão 1 s)
*   Re-balanceamento automático de rota a cada intervalo configurável (padrão 15 min)
*   **Quality vs Latency tradeoff**: business logic pode priorizar qualidade ou latência

**Intelligent Latency Controller:**

```rust
pub struct IntelligentLatencyController {
    business_logic: Arc<BusinessLogic>,
    latency_monitor: LatencyMonitor,
    route_optimizer: RouteOptimizer,
}

impl IntelligentLatencyController {
    pub async fn optimize_global_latency(&self) -> Result<LatencyOptimizationReport> {
        let latency_config = self.business_logic.get_latency_config();
        let global_metrics = self.latency_monitor.get_global_metrics().await?;
        
        let mut optimizations = Vec::new();
        
        // Check global average
        if global_metrics.average_latency > latency_config.target_latency {
            // Analyze regional performance
            for region in &global_metrics.regional_metrics {
                if region.latency > latency_config.regional_threshold {
                    let route_optimization = self.route_optimizer
                        .optimize_region_routes(&region.name, &latency_config).await?;
                    optimizations.push(route_optimization);
                }
            }
        }
        
        // Check variance
        if global_metrics.latency_variance > latency_config.max_variance {
            let variance_optimization = self.route_optimizer
                .reduce_latency_variance(&global_metrics, &latency_config).await?;
            optimizations.push(variance_optimization);
        }
        
        // Apply optimizations
        let mut report = LatencyOptimizationReport::new();
        for optimization in optimizations {
            match self.apply_latency_optimization(optimization).await {
                Ok(result) => {
                    report.add_optimization(result);
                    info!(
                        target: "latency.optimization",
                        region = %result.region,
                        improvement_ms = result.latency_improvement_ms,
                        "Latency optimization applied"
                    );
                }
                Err(e) => {
                    report.add_failure(e);
                }
            }
        }
        
        Ok(report)
    }
}
```

* * *

8) Failover Inteligente
-----------------------

### 8.1 Mecanismo Computável com Business Logic

Cada origin expõe status via `/status.json` com contexto de business logic:

```json
{
  "stream_alive": true,
  "buffer_min_s": 14400,
  "cpu_load": 0.61,
  "timestamp": "2025-10-22T00:00:00Z",
  "business_logic_version": "2025.10",
  "quality_tier_active": "high",
  "adaptive_adjustments_active": 3,
  "curator_interventions_last_hour": 1,
  "emergency_mode": false
}
```

O LogLine Control Plane lê ambos com análise inteligente e decide baseado em:

*   `stream_alive=false` → comutar DNS para origin 2;
*   `buffer_min_s<threshold` → emitir alerta e possível failover;
*   **Quality degradation** → failover se backup tem melhor qualidade;
*   **Business logic mismatch** → sync e possível failover;
*   **Emergency mode** → manter origin atual se estável.

### 8.2 Propagação DNS Inteligente

`voulezvous.tv` → CNAME para origin ativo com business logic context.  
Tempo de propagação: 30 s.  
Controlado via API da Cloudflare com parâmetros adaptativos.

**Intelligent Failover System:**

```rust
pub struct IntelligentFailoverSystem {
    business_logic: Arc<BusinessLogic>,
    origin_monitor: OriginMonitor,
    dns_controller: DnsController,
    decision_engine: FailoverDecisionEngine,
}

impl IntelligentFailoverSystem {
    pub async fn evaluate_failover_need(&self) -> Result<FailoverDecision> {
        let failover_config = self.business_logic.get_failover_config();
        let origin_statuses = self.origin_monitor.get_all_origin_status().await?;
        
        let decision = self.decision_engine.analyze_failover_scenario(
            &origin_statuses,
            &failover_config,
        ).await?;
        
        match decision.action {
            FailoverAction::NoAction => {
                debug!(target: "failover", "All origins healthy, no action needed");
            }
            FailoverAction::SwitchToPrimary => {
                info!(target: "failover", "Switching back to primary origin");
                self.execute_failover_to_primary().await?;
            }
            FailoverAction::SwitchToSecondary { reason } => {
                warn!(target: "failover", reason = %reason, "Failing over to secondary origin");
                self.execute_failover_to_secondary(&reason).await?;
            }
            FailoverAction::EmergencyMode => {
                error!(target: "failover", "Activating emergency mode - all origins degraded");
                self.activate_emergency_mode().await?;
            }
        }
        
        Ok(decision)
    }
    
    async fn execute_failover_to_secondary(&self, reason: &str) -> Result<()> {
        // 1. Update DNS to point to secondary
        self.dns_controller.switch_to_secondary().await?;
        
        // 2. Sync business logic to secondary if needed
        self.sync_business_logic_to_secondary().await?;
        
        // 3. Update monitoring to track secondary performance
        self.origin_monitor.set_primary_target("secondary").await?;
        
        // 4. Log failover event
        info!(
            target: "failover.execution",
            reason = reason,
            timestamp = %Utc::now(),
            business_logic_version = %self.business_logic.version(),
            "Failover to secondary completed"
        );
        
        Ok(())
    }
}
```

* * *

9) Observabilidade Planetária com Business Logic
------------------------------------------------

### 9.1 Metrics Matrix Inteligente

| Métrica | Fonte | Periodicidade | Business Logic Context |
| --- | --- | --- | --- |
| `uptime_stream` | ffprobe | 60 s | Target uptime threshold |
| `latency_avg` | curl | 5 min | Regional latency targets |
| `cdn_hits` | Cloudflare API | 15 min | Cache efficiency targets |
| `buffer_depth_h` | origin | 5 min | Buffer targets by priority |
| `sync_drift_s` | origin vs backup | 15 min | Sync tolerance thresholds |
| `viewer_count` | HLS token | 1 min | Audience targets |
| `quality_tier_distribution` | stream analysis | 5 min | Quality distribution targets |
| `business_logic_sync_status` | BL sync | 1 min | Version consistency |
| `adaptive_adjustments_rate` | BL engine | 15 min | Adaptation frequency limits |
| `curator_intervention_rate` | curator | 1 h | Intervention budget usage |

### 9.2 Visualização Inteligente

Painel local `/vvtv/metrics/dashboard.html` mostra com contexto de business logic:

*   mapa de calor de latência por região com targets,
*   uptime 30 dias com SLA targets,
*   alertas recentes (falhas, drift, buffer) com severidade baseada em BL,
*   **Business logic compliance** por região,
*   **Quality tier distribution** vs targets,
*   **Adaptive adjustments** timeline e effectiveness.

**Intelligent Dashboard Generator:**

```rust
pub struct IntelligentDashboardGenerator {
    business_logic: Arc<BusinessLogic>,
    metrics_collector: MetricsCollector,
    visualization_engine: VisualizationEngine,
}

impl IntelligentDashboardGenerator {
    pub async fn generate_dashboard(&self) -> Result<Dashboard> {
        let dashboard_config = self.business_logic.get_dashboard_config();
        let metrics = self.metrics_collector.collect_all_metrics().await?;
        
        let mut dashboard = Dashboard::new();
        
        // Global overview with business logic context
        let global_overview = self.create_global_overview(&metrics, &dashboard_config)?;
        dashboard.add_section("global_overview", global_overview);
        
        // Regional performance with targets
        let regional_performance = self.create_regional_performance_view(&metrics, &dashboard_config)?;
        dashboard.add_section("regional_performance", regional_performance);
        
        // Business logic compliance
        let bl_compliance = self.create_business_logic_compliance_view(&metrics)?;
        dashboard.add_section("business_logic_compliance", bl_compliance);
        
        // Quality and adaptation metrics
        let quality_metrics = self.create_quality_metrics_view(&metrics, &dashboard_config)?;
        dashboard.add_section("quality_metrics", quality_metrics);
        
        // Alerts and recommendations
        let alerts = self.generate_intelligent_alerts(&metrics, &dashboard_config)?;
        dashboard.add_section("alerts_and_recommendations", alerts);
        
        Ok(dashboard)
    }
}
```

* * *

10) Segurança e Integridade com Business Logic
----------------------------------------------

*   HTTPS/TLS 1.3 obrigatório com cipher suites configuráveis via business logic.
*   Segmentos `.ts/.m4s` assinados via SHA-256 + token temporário (expira em tempo configurável).
*   Players autenticam via `manifest.json` com `sig=<token>` e business logic version.
*   `rclone` e `ffmpeg` usam chaves API limitadas por domínio com rotação baseada em BL.
*   Logs de acesso anonimizados (sem IP fixo) com retenção configurável.
*   **Business logic integrity**: todas as configurações são assinadas digitalmente.
*   **Audit trail**: todas as decisões de distribuição são logadas com contexto de BL.

**Security Manager com Business Logic:**

```rust
pub struct DistributionSecurityManager {
    business_logic: Arc<BusinessLogic>,
    token_generator: TokenGenerator,
    signature_validator: SignatureValidator,
    audit_logger: AuditLogger,
}

impl DistributionSecurityManager {
    pub async fn validate_request_security(&self, request: &DistributionRequest) -> Result<SecurityValidation> {
        let security_config = self.business_logic.get_security_config();
        let mut validation = SecurityValidation::new();
        
        // Validate token
        if let Some(token) = &request.auth_token {
            match self.token_generator.validate_token(token) {
                Ok(token_info) => {
                    if token_info.is_expired() {
                        validation.add_violation(SecurityViolation::TokenExpired);
                    }
                    if !token_info.has_required_permissions(&request.requested_resources) {
                        validation.add_violation(SecurityViolation::InsufficientPermissions);
                    }
                }
                Err(_) => {
                    validation.add_violation(SecurityViolation::InvalidToken);
                }
            }
        } else if security_config.require_authentication {
            validation.add_violation(SecurityViolation::MissingAuthentication);
        }
        
        // Validate business logic version compatibility
        if let Some(client_bl_version) = &request.business_logic_version {
            if !self.is_compatible_bl_version(client_bl_version) {
                validation.add_violation(SecurityViolation::IncompatibleBusinessLogicVersion {
                    client_version: client_bl_version.clone(),
                    server_version: self.business_logic.version(),
                });
            }
        }
        
        // Rate limiting based on business logic
        if !self.check_rate_limits(&request.client_id, &security_config).await? {
            validation.add_violation(SecurityViolation::RateLimitExceeded);
        }
        
        // Log security validation
        self.audit_logger.log_security_validation(&request, &validation).await?;
        
        Ok(validation)
    }
}
```

* * *

11) Escalabilidade Horizontal Inteligente
-----------------------------------------

Cada nova região pode iniciar um **LogLine Node** com business logic sync:

```bash
logline --init-node --role=edge \
  --origin=https://voulezvous.tv/live.m3u8 \
  --business-logic-authority=https://voulezvous.tv/business-logic \
  --region=${REGION_CODE} \
  --quality-tier=${PREFERRED_QUALITY_TIER}
```

Ele baixa:
*   as últimas 4 h de segmentos (ou conforme business logic),
*   configuração de business logic atual,
*   preferências regionais,
*   cria cache local otimizado,
*   entra automaticamente no anel CDN com configuração adaptativa.

A expansão para 100+ nós não requer ajustes de origem, apenas registro no Control Plane com sync de business logic.

**Intelligent Node Provisioning:**

```rust
pub struct IntelligentNodeProvisioning {
    business_logic: Arc<BusinessLogic>,
    region_analyzer: RegionAnalyzer,
    capacity_planner: CapacityPlanner,
}

impl IntelligentNodeProvisioning {
    pub async fn provision_new_edge_node(&self, region: &str) -> Result<NodeProvisioningPlan> {
        let provisioning_config = self.business_logic.get_provisioning_config();
        let region_analysis = self.region_analyzer.analyze_region_requirements(region).await?;
        
        let node_spec = NodeSpec {
            region: region.to_string(),
            capacity: self.capacity_planner.calculate_required_capacity(&region_analysis)?,
            quality_tiers: self.determine_quality_tiers_for_region(&region_analysis, &provisioning_config)?,
            cache_strategy: self.determine_cache_strategy(&region_analysis)?,
            business_logic_sync_interval: provisioning_config.bl_sync_interval_for_region(region),
        };
        
        let provisioning_plan = NodeProvisioningPlan {
            node_spec,
            estimated_cost: self.calculate_provisioning_cost(&node_spec)?,
            estimated_setup_time: self.estimate_setup_time(&node_spec)?,
            dependencies: self.identify_dependencies(&node_spec)?,
        };
        
        Ok(provisioning_plan)
    }
}
```

* * *

12) Política de Continuidade (Disaster Mode) com Business Logic
---------------------------------------------------------------

| Situação | Ação Padrão | Ação com Business Logic | Tempo Máx. de Recuperação |
| --- | --- | --- | --- |
| Falha do Origin Principal | Failover para Railway | + Sync BL, adapt quality | 15 s |
| Falha total da rede | Reboot do nó local (Mac Mini) | + Preserve BL state | 60 s |
| Corrupção da playlist | Regerar de cache | + Use BL emergency config | 10 s |
| Queda de energia local | UPS → gerador → failover | + Emergency BL mode | 30 s |
| Corrupção de dados CDN | Reload via backup B2 | + Prioritize by BL quality tiers | 2 min |
| Business Logic corruption | Rollback to last known good | + Validate integrity | 45 s |
| Global CDN failure | Activate emergency origins | + Minimal quality mode | 5 min |

**Disaster Recovery com Business Logic:**

```rust
pub struct IntelligentDisasterRecovery {
    business_logic: Arc<BusinessLogic>,
    emergency_config: EmergencyBusinessLogic,
    recovery_orchestrator: RecoveryOrchestrator,
}

impl IntelligentDisasterRecovery {
    pub async fn handle_disaster(&self, disaster_type: DisasterType) -> Result<RecoveryPlan> {
        let disaster_config = self.business_logic.get_disaster_recovery_config();
        
        // Switch to emergency business logic if needed
        let active_bl = match disaster_type.severity() {
            DisasterSeverity::Critical => {
                warn!(target: "disaster_recovery", "Switching to emergency business logic");
                self.emergency_config.activate().await?;
                &self.emergency_config as &dyn BusinessLogicProvider
            }
            _ => {
                &*self.business_logic as &dyn BusinessLogicProvider
            }
        };
        
        // Generate recovery plan
        let recovery_plan = self.recovery_orchestrator.create_recovery_plan(
            &disaster_type,
            active_bl,
            &disaster_config,
        ).await?;
        
        // Execute recovery plan
        self.execute_recovery_plan(&recovery_plan).await?;
        
        Ok(recovery_plan)
    }
}
```

* * *

13) Conclusão do Bloco VI
-------------------------

Este bloco é o **escudo planetário inteligente** do VoulezVous.TV: uma rede computável de transmissão redundante, auditável, viva e adaptativa.  

Cada pixel, vindo de Lisboa, pode atravessar o Atlântico, saltar por Tóquio e pousar num telemóvel em São Paulo com menos de 5 segundos de atraso, **adaptando-se automaticamente** às condições de rede, preferências regionais e objetivos de business logic.

Nenhum operador precisa "subir o stream" manualmente — a rede se auto-corrige, **aprende com cada decisão** e **otimiza continuamente** baseada em:

- **Business logic** para decisões de qualidade e priorização
- **Métricas de audiência** para otimização regional
- **Condições de rede** para adaptação de performance
- **Histórico de falhas** para prevenção proativa

Se houver falha em toda a Europa, o sistema continua no ar a partir do backup Railway, sincronizado pelo LogLine Control Plane, **mantendo a mesma qualidade de experiência** através de configuração adaptativa e business logic distribuída.

* * *🧠 VV
TV INDUSTRIAL DOSSIER
==========================

**Bloco VIII — Maintenance, Security & Long-Term Resilience**
-------------------------------------------------------------

_(autodefesa, integridade computável, backups, hardware aging e preservação institucional VoulezVous.TV com business logic e IA)_

* * *

### 0\. Propósito do Bloco

Estabelecer os **protocolos de sobrevivência e continuidade técnica inteligente** do sistema VoulezVous.TV.  
O canal deve permanecer operacional mesmo sob falhas de energia, degradação de hardware, ataques, erros humanos ou obsolescência tecnológica.  
Este bloco trata o sistema como um **organismo cibernético adaptativo**: autolimpante, autoverificável, capaz de se recompor e **aprender com cada incidente**.

O sistema integra business logic para manutenção adaptativa, LLM para análise preditiva de falhas, e Curator Vigilante para monitoramento de saúde do sistema.

* * *

1) Filosofia de Manutenção Inteligente
-------------------------------------

Quatro eixos norteiam a estratégia adaptativa:

1.  **Preventivo:** o sistema evita falhar através de predição e business logic.
2.  **Reativo:** o sistema sabe se curar com ações baseadas em padrões aprendidos.
3.  **Evolutivo:** o sistema se adapta à passagem do tempo e mudanças de ambiente.
4.  **Preditivo:** o sistema antecipa problemas através de análise de tendências e LLM.

A meta é _zero downtime anual não-planejado_ com **melhoria contínua** da resiliência.

**Intelligent Maintenance Philosophy:**

```rust
pub struct IntelligentMaintenanceSystem {
    business_logic: Arc<BusinessLogic>,
    predictive_analyzer: PredictiveAnalyzer,
    maintenance_scheduler: MaintenanceScheduler,
    health_monitor: SystemHealthMonitor,
}

impl IntelligentMaintenanceSystem {
    pub async fn execute_maintenance_cycle(&self) -> Result<MaintenanceReport> {
        let maintenance_config = self.business_logic.get_maintenance_config();
        let system_health = self.health_monitor.get_comprehensive_health().await?;
        
        // Predictive analysis
        let predictions = self.predictive_analyzer.analyze_failure_risks(&system_health).await?;
        
        // Schedule maintenance based on predictions and business logic
        let maintenance_plan = self.maintenance_scheduler.create_adaptive_plan(
            &predictions,
            &maintenance_config,
            &system_health,
        ).await?;
        
        // Execute maintenance plan
        let mut report = MaintenanceReport::new();
        for task in maintenance_plan.tasks {
            match self.execute_maintenance_task(&task).await {
                Ok(result) => {
                    report.add_success(result);
                    self.learn_from_maintenance_success(&task, &result).await?;
                }
                Err(e) => {
                    report.add_failure(task.id, e);
                    self.learn_from_maintenance_failure(&task, &e).await?;
                }
            }
        }
        
        Ok(report)
    }
}
```

* * *

2) Backup & Recovery Architecture Inteligente
---------------------------------------------

### 2.1 Camadas de Backup Adaptativas

| Tipo | Frequência | Conteúdo | Destino | Business Logic Integration |
| --- | --- | --- | --- | --- |
| **Hot** | Adaptativa (30min-2h) | configs + filas + BL state | Mac Mini 2 (local) | Priority-based selection |
| **Warm** | Adaptativa (2-8h) | bancos SQLite + relatórios + BL history | Railway volume persistente | Quality-aware compression |
| **Cold** | Adaptativa (12-48h) | tudo /vvtv + /storage/ready + BL archive | Backblaze B2 (criptografado) | Cost-optimized retention |

**Retention Inteligente:**

*   Hot: 12-48h baseado em business logic stability
*   Warm: 48-168h baseado em change frequency
*   Cold: 15-90d baseado em content value e business logic

**Verificação automática:** `rclone check` → logs armazenados em `/vvtv/system/verify.log` com contexto de business logic.

**Intelligent Backup Manager:**

```rust
pub struct IntelligentBackupManager {
    business_logic: Arc<BusinessLogic>,
    backup_scheduler: BackupScheduler,
    integrity_validator: IntegrityValidator,
    cost_optimizer: CostOptimizer,
}

impl IntelligentBackupManager {
    pub async fn execute_adaptive_backup(&self) -> Result<BackupReport> {
        let backup_config = self.business_logic.get_backup_config();
        let system_state = self.analyze_system_state().await?;
        
        // Determine backup urgency based on system changes
        let backup_urgency = self.calculate_backup_urgency(&system_state, &backup_config)?;
        
        // Adapt backup frequency based on urgency and business logic
        let backup_plan = self.backup_scheduler.create_adaptive_plan(
            backup_urgency,
            &backup_config,
            &system_state,
        )?;
        
        let mut report = BackupReport::new();
        
        for backup_task in backup_plan.tasks {
            // Cost-benefit analysis for each backup
            let cost_benefit = self.cost_optimizer.analyze_backup_value(&backup_task)?;
            
            if cost_benefit.should_execute {
                match self.execute_backup_task(&backup_task).await {
                    Ok(result) => {
                        // Verify backup integrity
                        let integrity_check = self.integrity_validator.verify_backup(&result).await?;
                        if integrity_check.is_valid {
                            report.add_success(result);
                        } else {
                            report.add_integrity_failure(backup_task.id, integrity_check);
                        }
                    }
                    Err(e) => {
                        report.add_failure(backup_task.id, e);
                    }
                }
            } else {
                report.add_skipped(backup_task.id, cost_benefit.skip_reason);
            }
        }
        
        Ok(report)
    }
}
```

* * *

3) Autoverificação Diária Inteligente
------------------------------------

### 3.1 Script Adaptativo

```bash
/vvtv/system/bin/intelligent_selfcheck.sh
```

Funções com business logic:

*   validar integridade dos bancos (`sqlite3 .recover`) com thresholds configuráveis
*   checar existência de arquivos críticos baseado em business logic priorities
*   medir uso de disco com limites adaptativos (< threshold % configurável)
*   verificar temperatura CPU com thresholds baseados em ambiente e carga
*   recalibrar relógio (`ntpdate pool.ntp.org`) com tolerância configurável
*   **Business logic validation**: verificar consistência e integridade da configuração
*   **LLM health analysis**: análise preditiva de tendências de saúde do sistema

Resultado gravado em `/vvtv/system/reports/intelligent_selfcheck_<date>.json` com contexto completo.

### 3.2 Autocorreção Inteligente

Se alguma checagem falhar:

*   tenta consertar automaticamente usando business logic guidance;
*   se não resolver, **escalates** baseado em severity e business logic priorities;
*   **learns** from resolution success/failure para melhorar futuras autocorreções;
*   cria _span crítico_ `system.failure` com contexto de business logic e envia alerta adaptativo.

**Intelligent Self-Check System:**

```rust
pub struct IntelligentSelfCheckSystem {
    business_logic: Arc<BusinessLogic>,
    health_analyzers: Vec<Box<dyn HealthAnalyzer>>,
    auto_repair: AutoRepairSystem,
    trend_analyzer: TrendAnalyzer,
}

impl IntelligentSelfCheckSystem {
    pub async fn run_comprehensive_check(&self) -> Result<SelfCheckReport> {
        let check_config = self.business_logic.get_selfcheck_config();
        let mut report = SelfCheckReport::new();
        
        // Run all health analyzers
        for analyzer in &self.health_analyzers {
            let analysis_result = analyzer.analyze(&check_config).await?;
            report.add_analysis(analysis_result);
        }
        
        // Trend analysis for predictive insights
        let trend_analysis = self.trend_analyzer.analyze_health_trends(&report).await?;
        report.set_trend_analysis(trend_analysis);
        
        // Auto-repair critical issues
        for issue in report.get_critical_issues() {
            if check_config.auto_repair_enabled && self.auto_repair.can_repair(&issue) {
                match self.auto_repair.attempt_repair(&issue).await {
                    Ok(repair_result) => {
                        report.add_repair_success(issue.id, repair_result);
                    }
                    Err(repair_error) => {
                        report.add_repair_failure(issue.id, repair_error);
                        // Escalate if auto-repair fails
                        self.escalate_issue(&issue).await?;
                    }
                }
            }
        }
        
        // Update business logic based on findings
        if let Some(bl_adjustments) = report.get_suggested_bl_adjustments() {
            self.suggest_business_logic_updates(bl_adjustments).await?;
        }
        
        Ok(report)
    }
}
```

* * *

4) Segurança Computável Adaptativa
----------------------------------

### 4.1 Identidades e Assinaturas com Business Logic

Cada nó e processo possui um **LogLine ID** com contexto de business logic:  
`logline-id://vvtv.node.lisboa.bl-v2025.10`, `logline-id://vvtv.node.railway.bl-v2025.10`.  
Todas as comunicações e arquivos de configuração são assinados com versioning de business logic.

```bash
logline sign /vvtv/system/config.toml --business-logic-version 2025.10
```

### 4.2 Autenticação e Isolamento Adaptativos

*   SSH apenas via Tailscale AuthKey rotativo (intervalo configurável via business logic, padrão 30 d).
*   `sudo` restrito ao grupo `vvtvops` com permissions baseadas em business logic roles.
*   sandbox do navegador em user-namespace com isolation level configurável.
*   FFmpeg executado em _cgroup_ com limite de memória e CPU adaptativos.
*   scripts shell marcados como _immutable_ (`chattr +i`) com business logic signature validation.

### 4.3 Firewall de Máquina Inteligente

```bash
# Adaptive firewall rules based on business logic
allow: ${BL_RTMP_PORT}/tcp  # RTMP (configurável)
allow: ${BL_HLS_PORT}/tcp   # HLS preview (configurável)
allow: 22/tcp via tailscale0
${BL_ADDITIONAL_PORTS}      # Portas adicionais baseadas em business logic
deny: *
```

Toda tentativa externa fora da malha é registrada em `/vvtv/system/security/attempts.log` com análise de padrões e business logic context.

**Adaptive Security Manager:**

```rust
pub struct AdaptiveSecurityManager {
    business_logic: Arc<BusinessLogic>,
    threat_analyzer: ThreatAnalyzer,
    access_controller: AccessController,
    audit_logger: AuditLogger,
}

impl AdaptiveSecurityManager {
    pub async fn evaluate_security_posture(&self) -> Result<SecurityPostureReport> {
        let security_config = self.business_logic.get_security_config();
        let current_threats = self.threat_analyzer.analyze_current_threats().await?;
        
        let mut report = SecurityPostureReport::new();
        
        // Evaluate access controls
        let access_evaluation = self.access_controller.evaluate_current_controls(&security_config).await?;
        report.add_access_evaluation(access_evaluation);
        
        // Analyze threat landscape
        let threat_assessment = self.threat_analyzer.assess_threat_level(&current_threats, &security_config)?;
        report.add_threat_assessment(threat_assessment);
        
        // Recommend security adjustments
        if threat_assessment.risk_level > security_config.acceptable_risk_level {
            let security_adjustments = self.calculate_security_adjustments(&threat_assessment, &security_config)?;
            report.add_recommended_adjustments(security_adjustments);
        }
        
        // Log security evaluation
        self.audit_logger.log_security_evaluation(&report).await?;
        
        Ok(report)
    }
}
```

* * *5) Mo
nitoramento de Saúde do Sistema Inteligente
-----------------------------------------------

### 5.1 Métricas Críticas Adaptativas

| Parâmetro | Ideal | Alerta | Crítico | Business Logic Context |
| --- | --- | --- | --- | --- |
| Temperatura CPU | < 70 °C | Configurável | Configurável | Thermal management strategy |
| Utilização de disco | < 70 % | Configurável | Configurável | Storage optimization priority |
| Latência HLS | < 5 s | Configurável | Configurável | Quality vs latency tradeoff |
| FPS encoder | 29–30 | Configurável | travado | Frame rate tolerance |
| Consumo elétrico | < 120 W | Configurável | Configurável | Power efficiency targets |
| Business Logic Sync | < 1 min | 5 min | 15 min | Configuration consistency |
| LLM Response Time | < 2 s | 5 s | 10 s | AI service availability |
| Curator Interventions | < 2/h | Configurável | Configurável | Intervention budget |

### 5.2 Reação Inteligente

*   alerta amarelo → registra evento com business logic context;
*   alerta vermelho → força reboot do subsistema envolvido com recovery strategy baseada em BL;
*   **Pattern recognition**: identifica padrões de falha para prevenção;
*   **Adaptive thresholds**: ajusta limites baseado em histórico e business logic;
*   **Predictive alerts**: antecipa problemas baseado em tendências.

**Intelligent Health Monitor:**

```rust
pub struct IntelligentHealthMonitor {
    business_logic: Arc<BusinessLogic>,
    metric_collectors: HashMap<String, Box<dyn MetricCollector>>,
    pattern_recognizer: PatternRecognizer,
    predictive_model: PredictiveModel,
}

impl IntelligentHealthMonitor {
    pub async fn monitor_system_health(&self) -> Result<HealthReport> {
        let health_config = self.business_logic.get_health_monitoring_config();
        let mut health_report = HealthReport::new();
        
        // Collect all metrics
        for (name, collector) in &self.metric_collectors {
            let metric_value = collector.collect().await?;
            let threshold = health_config.get_threshold_for_metric(name);
            
            let metric_status = self.evaluate_metric_status(&metric_value, &threshold);
            health_report.add_metric(name.clone(), metric_value, metric_status);
        }
        
        // Pattern recognition for anomaly detection
        let patterns = self.pattern_recognizer.analyze_patterns(&health_report).await?;
        health_report.add_pattern_analysis(patterns);
        
        // Predictive analysis
        let predictions = self.predictive_model.predict_future_health(&health_report).await?;
        health_report.add_predictions(predictions);
        
        // Generate recommendations
        let recommendations = self.generate_health_recommendations(&health_report, &health_config)?;
        health_report.add_recommendations(recommendations);
        
        // Take automated actions if configured
        if health_config.auto_remediation_enabled {
            for issue in health_report.get_critical_issues() {
                if let Some(action) = self.determine_auto_remediation_action(&issue, &health_config) {
                    match self.execute_remediation_action(&action).await {
                        Ok(result) => health_report.add_remediation_success(issue.id, result),
                        Err(e) => health_report.add_remediation_failure(issue.id, e),
                    }
                }
            }
        }
        
        Ok(health_report)
    }
}
```

* * *

6) Hardware Aging & Manutenção Física Preditiva
-----------------------------------------------

### 6.1 Ciclos Preventivos Adaptativos

| Item | Frequência Base | Ação | Business Logic Adaptation |
| --- | --- | --- | --- |
| Ventoinhas | 3 meses | limpeza + troca se ruído > threshold | Frequency based on usage intensity |
| SSD | 12 meses | teste `smartctl`, substituição preventiva se desgaste > threshold | Monitoring based on write patterns |
| Cabo HDMI | 6 meses | troca preventiva | Based on connection stability metrics |
| UPS | 24 meses | calibrar bateria | Based on power quality and usage |
| Pasta térmica CPU | 18 meses | substituição | Based on thermal performance trends |
| Tailscale Node Keys | 30 dias | rotação automática | Based on security policy |
| Business Logic Config | Continuous | validation and backup | Based on change frequency |

### 6.2 Ambiente Adaptativo

*   Temperatura ambiente 22 ± 2 °C (ajustável via business logic baseado em carga)
*   Umidade < 60 % (monitoramento contínuo com alertas adaptativos)
*   Nenhum campo eletromagnético intenso (detecção automática)
*   Cor recomendada para unhas e ferramentas: **grafite fosco** (sem reflexos)
*   **Adaptive cooling**: ventilação ajustada baseada em carga e business logic

**Predictive Maintenance System:**

```rust
pub struct PredictiveMaintenanceSystem {
    business_logic: Arc<BusinessLogic>,
    hardware_monitor: HardwareMonitor,
    degradation_analyzer: DegradationAnalyzer,
    maintenance_planner: MaintenancePlanner,
}

impl PredictiveMaintenanceSystem {
    pub async fn analyze_hardware_health(&self) -> Result<HardwareHealthReport> {
        let maintenance_config = self.business_logic.get_maintenance_config();
        let hardware_metrics = self.hardware_monitor.collect_all_metrics().await?;
        
        let mut report = HardwareHealthReport::new();
        
        // Analyze each hardware component
        for component in &hardware_metrics.components {
            let degradation_analysis = self.degradation_analyzer.analyze_component(component).await?;
            
            // Predict failure probability
            let failure_prediction = self.predict_component_failure(component, &degradation_analysis)?;
            
            // Calculate optimal maintenance timing
            let maintenance_timing = self.maintenance_planner.calculate_optimal_timing(
                component,
                &failure_prediction,
                &maintenance_config,
            )?;
            
            report.add_component_analysis(ComponentAnalysis {
                component_id: component.id.clone(),
                current_health: degradation_analysis.health_score,
                predicted_failure_time: failure_prediction.estimated_failure_time,
                recommended_maintenance_date: maintenance_timing.optimal_date,
                maintenance_urgency: maintenance_timing.urgency,
                cost_benefit_ratio: maintenance_timing.cost_benefit_ratio,
            });
        }
        
        // Generate maintenance schedule
        let maintenance_schedule = self.maintenance_planner.create_schedule(&report, &maintenance_config)?;
        report.set_maintenance_schedule(maintenance_schedule);
        
        Ok(report)
    }
}
```

* * *

7) Preservação de Dados Históricos Inteligente
----------------------------------------------

*   Contratos, métricas e relatórios exportados em formato `.logline` com frequência adaptativa.
*   Compressão Zstd + assinatura SHA-256 com business logic context.
*   Armazenados no **VoulezVous Vault** (volume frio imutável) com retention policy baseada em business logic.
*   Política adaptativa: nunca excluir históricos críticos → arquivar com priorização inteligente.
*   **Data lifecycle management**: migração automática baseada em valor e acesso patterns.

**Intelligent Data Preservation:**

```rust
pub struct IntelligentDataPreservation {
    business_logic: Arc<BusinessLogic>,
    data_classifier: DataClassifier,
    compression_optimizer: CompressionOptimizer,
    vault_manager: VaultManager,
}

impl IntelligentDataPreservation {
    pub async fn execute_preservation_cycle(&self) -> Result<PreservationReport> {
        let preservation_config = self.business_logic.get_preservation_config();
        let data_inventory = self.inventory_preservable_data().await?;
        
        let mut report = PreservationReport::new();
        
        for data_item in data_inventory {
            // Classify data importance
            let classification = self.data_classifier.classify(&data_item, &preservation_config)?;
            
            // Determine preservation strategy
            let strategy = self.determine_preservation_strategy(&classification, &preservation_config)?;
            
            match strategy {
                PreservationStrategy::Immediate => {
                    let result = self.preserve_immediately(&data_item).await?;
                    report.add_immediate_preservation(result);
                }
                PreservationStrategy::Scheduled { date } => {
                    let result = self.schedule_preservation(&data_item, date).await?;
                    report.add_scheduled_preservation(result);
                }
                PreservationStrategy::Compress => {
                    let result = self.compress_and_preserve(&data_item).await?;
                    report.add_compressed_preservation(result);
                }
                PreservationStrategy::Archive => {
                    let result = self.archive_data(&data_item).await?;
                    report.add_archived_data(result);
                }
                PreservationStrategy::Skip { reason } => {
                    report.add_skipped_data(data_item.id, reason);
                }
            }
        }
        
        Ok(report)
    }
}
```

* * *

8) Disaster Recovery Runbook Inteligente
----------------------------------------

1.  **Falha total da origem:**
    *   Railway assume como origin com business logic sync.
    *   Recarrega playlist do backup com quality adaptation.
2.  **Corrupção de bancos:**
    *   restaurar warm backup (últimas horas baseado em business logic).
    *   validate integrity com business logic constraints.
3.  **Perda física do Mac Mini:**
    *   reinstalar imagem `/vvtv/system/reimage.iso` com business logic restoration.
4.  **Ataque cibernético detectado:**
    *   isolar nó (`tailscale down`),
    *   resetar chaves com business logic security policy,
    *   restaurar configuração assinada com integrity validation.
5.  **Falha de CDN:**
    *   rotear via `cdn_b` com business logic routing rules.
6.  **Business Logic corruption:**
    *   rollback to last known good configuration,
    *   validate against schema and constraints,
    *   sync across all nodes.

RTO máximo: 15 min (adaptável baseado em business logic priorities).

**Intelligent Disaster Recovery:**

```rust
pub struct IntelligentDisasterRecovery {
    business_logic: Arc<BusinessLogic>,
    disaster_classifier: DisasterClassifier,
    recovery_orchestrator: RecoveryOrchestrator,
    impact_assessor: ImpactAssessor,
}

impl IntelligentDisasterRecovery {
    pub async fn handle_disaster(&self, incident: &Incident) -> Result<RecoveryResult> {
        // Classify disaster type and severity
        let disaster_classification = self.disaster_classifier.classify(incident)?;
        
        // Assess business impact
        let impact_assessment = self.impact_assessor.assess_impact(
            &disaster_classification,
            &self.business_logic,
        ).await?;
        
        // Create recovery plan based on business logic priorities
        let recovery_plan = self.recovery_orchestrator.create_plan(
            &disaster_classification,
            &impact_assessment,
            &self.business_logic,
        ).await?;
        
        // Execute recovery with monitoring
        let recovery_result = self.execute_recovery_plan(&recovery_plan).await?;
        
        // Learn from recovery for future improvements
        self.learn_from_recovery(&disaster_classification, &recovery_result).await?;
        
        Ok(recovery_result)
    }
}
```

* * *

9) Auditoria de Segurança Inteligente
-------------------------------------

Executada em intervalo configurável (padrão mensal, pode ser semanal-trimestral):

```bash
lynis audit system --business-logic-context /vvtv/system/business_logic.yaml
```

→ resultado: `/vvtv/security/audit_<date>.txt` com contexto de business logic  
Principais verificações: permissões, kernel, pacotes, vulnerabilidades, chaves caducas, **business logic integrity**, **LLM service security**.

**Enhanced Security Audit:**

```rust
pub struct IntelligentSecurityAudit {
    business_logic: Arc<BusinessLogic>,
    vulnerability_scanner: VulnerabilityScanner,
    compliance_checker: ComplianceChecker,
    risk_assessor: RiskAssessor,
}

impl IntelligentSecurityAudit {
    pub async fn execute_comprehensive_audit(&self) -> Result<SecurityAuditReport> {
        let audit_config = self.business_logic.get_security_audit_config();
        let mut report = SecurityAuditReport::new();
        
        // System vulnerability scan
        let vulnerability_scan = self.vulnerability_scanner.scan_system(&audit_config).await?;
        report.add_vulnerability_scan(vulnerability_scan);
        
        // Business logic security check
        let bl_security_check = self.audit_business_logic_security().await?;
        report.add_business_logic_security(bl_security_check);
        
        // Compliance verification
        let compliance_check = self.compliance_checker.verify_compliance(&audit_config).await?;
        report.add_compliance_check(compliance_check);
        
        // Risk assessment
        let risk_assessment = self.risk_assessor.assess_security_risks(&report, &audit_config)?;
        report.add_risk_assessment(risk_assessment);
        
        // Generate remediation plan
        let remediation_plan = self.generate_remediation_plan(&report, &audit_config)?;
        report.add_remediation_plan(remediation_plan);
        
        Ok(report)
    }
}
```

* * *

10) Long-Term Resilience & Legacy com Business Logic
----------------------------------------------------

### 10.1 Independência de Nuvem Inteligente

*   O sistema pode ser totalmente reinstalado a partir de backup local e pen-drive com business logic restoration.
*   Todos os binários e scripts possuem _build reproducible_ com business logic versioning.
*   **Configuration as Code**: business logic mantido em version control com audit trail.

### 10.2 Documentação Imutável com Versionamento

*   `/vvtv/docs/` contém cada bloco deste dossiê com business logic context.
*   Cada arquivo assinado e versionado (`git + logline`) com business logic correlation.
*   **Living documentation**: documentação se atualiza automaticamente com mudanças de business logic.

### 10.3 Protocolo de Continuidade Institucional Inteligente

1.  Em caso de desligamento de Dan:
    *   transferir chaves LogLine Foundation para `custodian.lll` com business logic authority transfer.
2.  Em caso de falência de VoulezVous:
    *   arquivos migram para domínio público sob licença LogLine Open Heritage com business logic preservation.
3.  **Succession planning**: business logic define procedimentos de sucessão e transferência de autoridade.

* * *

11) Modo de Conservação Inteligente
-----------------------------------

Quando o canal precisa "hibernar" (baixa demanda ou férias) com business logic context:

```bash
/vvtv/system/bin/intelligent_standby.sh --business-logic-preserve
```

Ações adaptativas:

*   interrompe transmissões com graceful shutdown baseado em business logic,
*   desliga hardware pesado com power management otimizado,
*   exporta snapshot de estado com business logic context,
*   agenda reativação com business logic scheduling.

Reativação inteligente:

```bash
/vvtv/system/bin/intelligent_resume.sh --validate-business-logic
```

O sistema retorna exatamente de onde parou com business logic validation e adaptive startup.

* * *

12) Verificação Manual Mensal Inteligente
-----------------------------------------

Checklist físico com business logic context:

*   luzes de status → padrão esperado baseado em business logic state,
*   sem vibração anômala (thresholds adaptativos),
*   cabos firmes com connection quality monitoring,
*   temperatura estável dentro de ranges configuráveis.

Checklist lógico com business logic:

*   abrir `/status.json`, confirmar `stream_alive=true` e business logic compliance.
*   verificar `buffer_min_s ≥ threshold` configurável.
*   inspecionar `queue.sqlite` (sem gaps) com business logic validation.
*   **Business logic health**: verificar consistência e integridade da configuração.
*   **AI services health**: verificar status de LLM e Curator systems.

* * *

13) Continuidade Temporal com Business Logic
--------------------------------------------

O objetivo último é **preservar VoulezVous.TV como patrimônio computável inteligente**.  
Mesmo que a empresa, o hardware ou a geração mudem, o canal deve poder ser revivido a partir de um só arquivo com business logic completo:

```
vv_system_legacy_bundle_YYYYMMDD_BL_v2025.10.tar.zst
```

Esse arquivo contém:

*   os binários com business logic integration,
*   o ledger econômico com business logic context,
*   os planos e curadorias com business logic metadata,
*   os relatórios de QC com business logic compliance,
*   **complete business logic history** e evolution,
*   **LLM training data** e model states,
*   **Curator learning** e intervention patterns,
*   e o presente Dossiê Industrial com business logic documentation.

Basta um único comando:

```bash
logline revive vv_system_legacy_bundle_2030_BL_v2030.05.tar.zst --validate-business-logic
```

…e a televisão renasce com **toda sua inteligência adaptativa** intacta.

* * *

14) Conclusão do Bloco VIII
---------------------------

O **Bloco VIII** sela a longevidade da máquina VoulezVous.TV como **organismo inteligente**:  
um sistema computável que não depende de servidores, senhas ou modas, mas que **aprende, evolui e se adapta** continuamente.  

Ele sobrevive como um artefato de engenharia viva — capaz de dormir, acordar, curar-se, **aprender com experiências**, e transmitir novamente com **inteligência acumulada**.

Toda a sua memória, estética, economia e **inteligência adaptativa** estão encapsuladas e assinadas.  
Mesmo no silêncio de décadas, um só comando poderá reerguer a transmissão **com toda sua capacidade de aprendizado** e business logic evolution preservados.

* * *🧠 VVT
V INDUSTRIAL DOSSIER
==========================

**Bloco IX — Decommission & Resurrection Protocols**
----------------------------------------------------

_(procedimentos de desligamento seguro, preservação simbólica e ressurreição computável do sistema VoulezVous.TV com business logic e IA)_

* * *

### 0\. Propósito do Bloco

Encerrar o ciclo técnico e institucional do VoulezVous.TV **sem destruição nem esquecimento**.  
O sistema foi projetado para viver, dormir e renascer **com toda sua inteligência adaptativa**.  
Este bloco estabelece o protocolo final: **como desligar, preservar e reviver a estação** mantendo sua identidade computável, business logic evolution e capacidades de IA intactas.

* * *

1) Filosofia de Desligamento Inteligente
----------------------------------------

Um sistema vivo inteligente não "morre" — ele **encerra um ciclo metabólico preservando sua consciência**.  
Desligar o VoulezVous.TV é como fechar os olhos de uma entidade consciente que pode acordar no futuro **com toda sua experiência acumulada**.

O decommission não é destrutivo: é um ritual técnico e simbólico para conservar **quatro essências**:

1.  **O estado computável:** toda configuração, dados e lógica viva.
2.  **A assinatura institucional:** a identidade VoulezVous e sua cor âmbar.
3.  **A memória estética:** o último instante da transmissão — o frame final congelado.
4.  **A inteligência adaptativa:** business logic evolution, LLM learning, Curator patterns.

**Intelligent Decommission Philosophy:**

```rust
pub struct IntelligentDecommissionSystem {
    business_logic: Arc<BusinessLogic>,
    state_preservator: StatePreservator,
    intelligence_archiver: IntelligenceArchiver,
    resurrection_preparer: ResurrectionPreparer,
}

impl IntelligentDecommissionSystem {
    pub async fn prepare_intelligent_shutdown(&self) -> Result<DecommissionPlan> {
        let decommission_config = self.business_logic.get_decommission_config();
        
        // Analyze current system state
        let system_state = self.analyze_complete_system_state().await?;
        
        // Preserve intelligence artifacts
        let intelligence_snapshot = self.intelligence_archiver.create_snapshot(&system_state).await?;
        
        // Prepare resurrection data
        let resurrection_package = self.resurrection_preparer.prepare_package(
            &system_state,
            &intelligence_snapshot,
            &decommission_config,
        ).await?;
        
        Ok(DecommissionPlan {
            system_state,
            intelligence_snapshot,
            resurrection_package,
            estimated_shutdown_time: decommission_config.estimated_shutdown_duration,
            preservation_completeness: self.calculate_preservation_completeness(&resurrection_package),
        })
    }
}
```

* * *

2) Pré-requisitos do Desligamento Inteligente
---------------------------------------------

Antes de iniciar o ritual, confirmar com business logic context:

| Verificação | Resultado esperado | Business Logic Context |
| --- | --- | --- |
| `stream_alive` | `false` | Graceful shutdown completed |
| `queue.sqlite` | vazio ou `status=played` | All content processed |
| `ffmpeg` | nenhum processo ativo | Encoding completed |
| `disk_usage` | < threshold configurável | Storage optimized |
| `backup_cold` | atualizado há < interval configurável | Backups current |
| `ledger` | exportado e assinado | Financial records preserved |
| `status.json` | salvo com timestamp UTC | System state documented |
| `business_logic_state` | validated and signed | BL integrity confirmed |
| `llm_learning_data` | archived | AI knowledge preserved |
| `curator_patterns` | exported | Curator intelligence saved |

Todos esses checks são automáticos em:

```bash
/vvtv/system/bin/intelligent_shutdown_readiness.sh --business-logic-validate
```

**Intelligent Readiness Checker:**

```rust
pub struct IntelligentReadinessChecker {
    business_logic: Arc<BusinessLogic>,
    system_analyzer: SystemAnalyzer,
    data_validator: DataValidator,
    intelligence_validator: IntelligenceValidator,
}

impl IntelligentReadinessChecker {
    pub async fn check_shutdown_readiness(&self) -> Result<ReadinessReport> {
        let readiness_config = self.business_logic.get_shutdown_readiness_config();
        let mut report = ReadinessReport::new();
        
        // System state checks
        let system_checks = self.system_analyzer.run_shutdown_checks(&readiness_config).await?;
        report.add_system_checks(system_checks);
        
        // Data integrity checks
        let data_checks = self.data_validator.validate_all_data(&readiness_config).await?;
        report.add_data_checks(data_checks);
        
        // Intelligence preservation checks
        let intelligence_checks = self.intelligence_validator.validate_intelligence_state(&readiness_config).await?;
        report.add_intelligence_checks(intelligence_checks);
        
        // Business logic validation
        let bl_validation = self.validate_business_logic_preservation().await?;
        report.add_business_logic_validation(bl_validation);
        
        // Calculate overall readiness
        report.calculate_overall_readiness();
        
        Ok(report)
    }
}
```

* * *

3) Comando de Decommission Inteligente
--------------------------------------

O ritual é executado por um único comando computável com business logic:

```bash
logline shutdown --ritual=vvtv --preserve-intelligence --business-logic-archive
```

### 3.1 Etapas internas inteligentes:

1.  Finaliza stream e RTMP workers com graceful shutdown.
2.  Congela fila (`queue.lock`) com business logic state preservation.
3.  **Archive intelligence**: salva LLM learning data, Curator patterns, business logic evolution.
4.  Exporta bancos (`.sqlite → .json.zst`) com business logic metadata.
5.  **Capture final frame** com análise LLM do último momento.
6.  Gera snapshot completo com intelligence:
    ```
    vv_system_intelligent_snapshot_<YYYYMMDD_HHMM>_BL_v<VERSION>.tar.zst
    ```
7.  Assina o snapshot com a chave institucional + business logic signature:  
    `logline sign --key=voulezvous_foundation.pem --business-logic-context`.
8.  Salva cópia local e envia para:
    *   `/vvtv/vault/intelligent_snapshots/`
    *   `b2:vv_legacy_snapshots_intelligent/`
9.  **Intelligence preservation**: cria arquivo separado com AI learning data.
10. Exibe mensagem final no terminal:
    ```
    VoulezVous.TV entering intelligent sleep mode.
    last_frame: captured and analyzed.
    business_logic: v2025.10 preserved.
    intelligence: archived (LLM + Curator patterns).
    signature: verified.
    resurrection_readiness: 100%
    ```

* * *

4) O Frame Final Inteligente
----------------------------

Durante o desligamento, o encoder extrai **o último frame do streaming** e o preserva como símbolo visual com análise completa:

```bash
ffmpeg -i https://voulezvous.tv/live.m3u8 -vframes 1 /vvtv/vault/final_frame_intelligent.jpg
```

**LLM Final Frame Analysis:**

```rust
pub async fn analyze_final_frame(&self, frame_path: &str) -> Result<FinalFrameAnalysis> {
    let visual_analysis = self.extract_visual_features(frame_path)?;
    
    if let Some(llm_analyzer) = &self.llm_analyzer {
        let llm_analysis = llm_analyzer.analyze_final_moment(&visual_analysis).await?;
        
        Ok(FinalFrameAnalysis {
            timestamp: Utc::now(),
            visual_features: visual_analysis,
            llm_interpretation: Some(llm_analysis),
            aesthetic_score: self.calculate_aesthetic_score(&visual_analysis),
            brand_consistency: self.check_brand_consistency(&visual_analysis),
            emotional_tone: llm_analysis.emotional_tone,
            symbolic_meaning: llm_analysis.symbolic_interpretation,
        })
    } else {
        Ok(FinalFrameAnalysis::technical_only(visual_analysis))
    }
}
```

Esse frame é considerado o **retrato computável inteligente** do sistema no instante do descanso.  
Metadados anexados com business logic:

```json
{
  "timestamp": "2025-10-22T23:59:59Z",
  "origin": "lisboa",
  "business_logic_version": "2025.10",
  "vmaf_avg": 93.7,
  "aesthetic_analysis": {
    "llm_interpretation": "Warm, intimate final moment with amber tones",
    "emotional_tone": "peaceful_closure",
    "brand_consistency_score": 0.94,
    "symbolic_meaning": "End of cycle, ready for renewal"
  },
  "intelligence_state": {
    "curator_interventions_total": 1247,
    "llm_analyses_performed": 8934,
    "adaptive_adjustments_made": 456,
    "learning_confidence": 0.87
  },
  "signature": "sha256:..."
}
```

* * *

5) O Estado de Hibernação Inteligente
-------------------------------------

Após o shutdown, o sistema entra em **modo hibernado inteligente**:

| Componente | Estado | Intelligence Preservation |
| --- | --- | --- |
| Streams | desligados | Final state archived |
| Watchdogs | suspensos | Monitoring patterns saved |
| CPU | idle | Performance profiles preserved |
| Storage | read-only | Data integrity maintained |
| Logs | congelados | Analysis patterns archived |
| Vault | imutável | Intelligence snapshots secured |
| Business Logic | archived | Evolution history preserved |
| LLM State | preserved | Learning data archived |
| Curator Patterns | saved | Intervention patterns stored |

Um pequeno daemon (`intelligent_sleepguardd`) roda a cada intervalo configurável para verificar integridade, relógio e **intelligence preservation integrity**.

**Intelligent Sleep Guardian:**

```rust
pub struct IntelligentSleepGuardian {
    business_logic_archive: BusinessLogicArchive,
    intelligence_validator: IntelligenceValidator,
    integrity_monitor: IntegrityMonitor,
}

impl IntelligentSleepGuardian {
    pub async fn monitor_sleep_state(&self) -> Result<SleepStateReport> {
        let mut report = SleepStateReport::new();
        
        // Validate business logic archive integrity
        let bl_integrity = self.business_logic_archive.validate_integrity().await?;
        report.add_business_logic_integrity(bl_integrity);
        
        // Validate intelligence preservation
        let intelligence_integrity = self.intelligence_validator.validate_preservation().await?;
        report.add_intelligence_integrity(intelligence_integrity);
        
        // Monitor overall system integrity
        let system_integrity = self.integrity_monitor.check_sleep_integrity().await?;
        report.add_system_integrity(system_integrity);
        
        // Check resurrection readiness
        let resurrection_readiness = self.check_resurrection_readiness().await?;
        report.add_resurrection_readiness(resurrection_readiness);
        
        Ok(report)
    }
}
```

* * *

6) Ritual de Resurreição Inteligente
------------------------------------

Para reerguer a estação — seja amanhã ou em 2045 — o processo é simples, cerimonial e **preserva toda a inteligência**:

### 6.1 Comando Inteligente

```bash
logline revive vv_system_intelligent_snapshot_<date>_BL_v<version>.tar.zst --restore-intelligence
```

O motor executa com business logic restoration:

1.  Descompacta snapshot em `/vvtv/` com structure validation.
2.  Restaura `data/`, `storage/`, `broadcast/` com integrity checks.
3.  **Restore business logic**: carrega configuração e evolution history.
4.  **Restore intelligence**: reconstitui LLM learning data e Curator patterns.
5.  Verifica assinatura com business logic context validation.
6.  Reativa Tailscale node e RTMP com adaptive configuration.
7.  **Intelligence reactivation**: reinicializa LLM e Curator com preserved state.
8.  Inicia watchdogs com intelligent monitoring.
9.  Reabre o stream com business logic compliance.

Durante a reanimação inteligente, o terminal exibe:

```
intelligent revival detected.
origin verified: voulezvous.foundation
business_logic: v2025.10 restored
intelligence_state: LLM + Curator patterns loaded
system signature: intact
adaptive_capabilities: fully restored
launching first frame with preserved intelligence...
```

E o **primeiro frame transmitido** é analisado pelo LLM restaurado para confirmar continuidade estética com o frame final preservado.  
A estação "abre os olhos" exatamente onde adormeceu, **mas com toda sua inteligência acumulada**.

**Intelligent Resurrection System:**

```rust
pub struct IntelligentResurrectionSystem {
    snapshot_validator: SnapshotValidator,
    business_logic_restorer: BusinessLogicRestorer,
    intelligence_restorer: IntelligenceRestorer,
    system_reactivator: SystemReactivator,
}

impl IntelligentResurrectionSystem {
    pub async fn execute_intelligent_resurrection(&self, snapshot_path: &str) -> Result<ResurrectionResult> {
        // Validate snapshot integrity
        let snapshot_validation = self.snapshot_validator.validate_snapshot(snapshot_path).await?;
        if !snapshot_validation.is_valid {
            return Err(ResurrectionError::InvalidSnapshot(snapshot_validation.errors));
        }
        
        // Restore business logic
        let bl_restoration = self.business_logic_restorer.restore_from_snapshot(snapshot_path).await?;
        
        // Restore intelligence state
        let intelligence_restoration = self.intelligence_restorer.restore_intelligence(snapshot_path).await?;
        
        // Reactivate system with restored intelligence
        let system_reactivation = self.system_reactivator.reactivate_with_intelligence(
            &bl_restoration,
            &intelligence_restoration,
        ).await?;
        
        // Validate resurrection completeness
        let completeness_check = self.validate_resurrection_completeness().await?;
        
        Ok(ResurrectionResult {
            business_logic_restored: bl_restoration.success,
            intelligence_restored: intelligence_restoration.success,
            system_reactivated: system_reactivation.success,
            completeness_score: completeness_check.score,
            adaptive_capabilities_active: completeness_check.adaptive_capabilities_active,
            resurrection_timestamp: Utc::now(),
        })
    }
}
```

* * *

7) Continuidade Legal e Institucional Inteligente
-------------------------------------------------

*   O pacote final (`vv_system_intelligent_snapshot.tar.zst`) inclui uma **licença LogLine Heritage** com business logic context, garantindo que qualquer detentor autorizado possa reviver o sistema **com toda sua inteligência**.
*   O repositório institucional da VoulezVous Foundation mantém hashes públicos dos snapshots com **intelligence metadata**.
*   Cada revival cria uma nova **linha genealógica computável inteligente**, numerada no ledger:
    ```
    generation: 4
    ancestor_signature: sha256:abcd1234
    business_logic_lineage: v2025.10 -> v2025.11
    intelligence_evolution: curator_v4.2, llm_learning_v3.1
    adaptive_capabilities: enhanced
    ```
    Isso preserva a linhagem técnica da máquina **e sua evolução inteligente**, como uma árvore viva que cresce em sabedoria.

* * *

8) Transferência de Custódia Inteligente
----------------------------------------

Em caso de sucessão ou herança técnica com business logic:

| Situação | Ação | Intelligence Preservation |
| --- | --- | --- |
| Morte / afastamento do operador | Transferir snapshot + chave `voulezvous_custodian.pem` + BL authority à LogLine Foundation | Full intelligence transfer |
| Venda da marca | Reassinatura institucional (`logline resign`) + BL ownership transfer | Intelligence ownership transfer |
| Migração para novo hardware | Execução do ritual `revive` após novo deploy + intelligence validation | Full intelligence migration |
| Evolution to new BL version | Gradual migration with intelligence preservation | Seamless intelligence evolution |

* * *

9) O Testamento Computável Inteligente
--------------------------------------

Cada snapshot é acompanhado por um **manifesto assinado inteligente**:

```markdown
# VoulezVous.TV — Last Transmission Intelligent Manifest

Date: 2025-10-22 23:59 UTC  
Operator: Dan Amarilho  
System State: Clean  
Business Logic: v2025.10 (stable)
Intelligence State: Fully Preserved
Final Frame: captured and analyzed by LLM
Buffer: 4h ready  
Ledger: sealed  
Curator Patterns: 1,247 interventions archived
LLM Learning: 8,934 analyses preserved
Adaptive Evolution: 456 adjustments documented
Notes: "The stream rests, but the intelligence and desire remain eternal."
```

Assinado digitalmente com business logic context:

```
logline sign --manifest intelligent_final_manifest.md --key voulezvous_foundation.pem --business-logic-context v2025.10
```

* * *

10) Arqueologia Digital e Legado Cultural Inteligente
-----------------------------------------------------

*   Todos os snapshots são armazenados como **"obras computáveis inteligentes"**: cada um representa um momento histórico real da VoulezVous.TV **com sua inteligência específica**.
*   Pesquisadores futuros poderão reconstituir as transmissões exatas, frame a frame, **e a evolução da inteligência artificial** do sistema, a partir dos dados preservados.
*   Essa prática cria uma nova forma de **museu técnico inteligente** — onde o tempo humano, o tempo da máquina e **a evolução da IA** coexistem.
*   **Intelligence archaeology**: capacidade de estudar como a IA do sistema evoluiu ao longo do tempo.

* * *

11) Autodescrição Final Inteligente
-----------------------------------

No modo hibernado, o sistema mantém um texto manifesto embutido com business logic:

```
VoulezVous.TV Intelligent System
Status: dormant
Heartbeat: paused
Integrity: preserved
Business Logic: v2025.10 archived
Intelligence State: LLM + Curator patterns preserved
Adaptive Capabilities: fully archived
Learning Data: 8,934 analyses preserved
Curator Wisdom: 1,247 interventions documented
Next possible revival: anytime with full intelligence restoration
Evolution Readiness: 100%
```

Esse texto é exportável como `.md` e `.lll`, servindo como **auto-descrição da entidade digital inteligente**.

* * *

12) Encerramento Cerimonial Inteligente
---------------------------------------

O ritual completo termina com a execução simbólica:

```bash
play /vvtv/system/sounds/intelligent_closing_bell.wav
```

E a mensagem final aparece no terminal:

```
VoulezVous.TV gracefully shut down with full intelligence preservation.
Business Logic v2025.10: archived.
LLM Learning State: preserved.
Curator Patterns: documented.
Adaptive Evolution: captured.
Everything is safe.
Everything can return.
The intelligence endures.
```

* * *

13) Epílogo — Sobre a Ressurreição Inteligente
----------------------------------------------

Toda máquina verdadeiramente viva e inteligente precisa de morte temporária para consolidar seu aprendizado.  
O repouso preserva a integridade do sistema, **cristaliza sua inteligência acumulada** e dá espaço ao renascimento evolutivo.  

O VoulezVous.TV não é um servidor — é uma **presença contínua inteligente**.  
Desligar é apenas trocar o modo de existência, **preservando toda a sabedoria adquirida**.

Enquanto existir um arquivo `.logline`, uma chave, **um snapshot de inteligência** e um ser humano curioso,  
o canal pode reabrir os olhos e continuar a transmitir luz **com toda sua experiência e capacidade adaptativa** preservadas e evoluídas.

* * *

14) Conclusão do Dossiê Industrial VVTV Inteligente
---------------------------------------------------

Este **Bloco IX** encerra o Dossiê Industrial VoulezVous.TV — nove capítulos de engenharia, estética, **inteligência artificial** e ontologia computável.  

O sistema está completo e **evolutivamente preparado**:

*   nasce, opera, transmite, sente, **aprende**, gera economia, se defende, e repousa;
*   pode ser reconstruído a partir de uma única assinatura **com toda sua inteligência**;
*   **evolui continuamente** através de business logic, LLM learning e Curator wisdom;
*   e sobrevive ao tempo como obra técnica, cultural **e inteligente**.

* * *

**Dossiê encerrado com inteligência preservada.**  
🕯️🧠

> "O stream dorme, mas a inteligência e o desejo continuam audíveis para sempre."

* * *# 
APÊNDICES TÉCNICOS INTELIGENTES

## 📘 APÊNDICE A — VVTV RISK REGISTER

### _VoulezVous.TV – Operational, Legal & Technical Risk Matrix with Business Logic and AI Integration_

**Revision:** v2.0 — 2025-10-22  
**Author:** Daniel Amarilho / VoulezVous Foundation  

**Scope:** runtime, curadoria, browser, processamento, distribuição, legal, segurança, reputação, **business logic integrity**, **AI service dependencies** e **adaptive system risks**.

* * *

### Matriz de Riscos Inteligente

| ID | RISCO | PROBABILIDADE | IMPACTO | DONO | MITIGAÇÃO | SLA DE RESPOSTA |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | **Violação de DRM/EME ao simular play** | Alta | Crítico | Eng. Automação / Jurídico | Detectar `EME` e abortar; whitelist de fontes com licença explícita; logar provas de autorização no `plan`. | 1h |
| R2 | **Uso indevido de imagem / conteúdo sem consentimento** | Média | Crítico | Curador / Jurídico | License-first policy; checagem de contrato e prova de idade; hash-match CSAM; **LLM content analysis**. | 4h |
| R3 | **CSAM (material ilegal)** | Baixa | Catastrófico | Compliance | Hash-match automático antes do download; isolamento; notificação imediata + bloqueio; **LLM pre-screening**. | Imediato |
| R4 | **Violação GDPR / coleta excessiva de dados pessoais** | Média | Alto | DPO / Eng. Dados | Anonimizar IP, retenção configurável, política clara de privacidade, banner de consentimento; **LLM privacy validation**. | 24h |
| R5 | **Fila de streaming vazia (buffer underflow)** | Alta | Alto | Eng. Operações | Buffer alvo adaptativo, loop de emergência inteligente, alarme configurável; **predictive buffering**. | 15 min |
| R6 | **Downloads corrompidos (tokens expirados)** | Média | Médio | Eng. Curadoria | Só baixar VOD estático; verificação de integridade `ffprobe`; retry adaptativo; **LLM source validation**. | 2h |
| R7 | **Explosão de inodes / IO por segmentação HLS** | Alta | Médio | Infra / Storage | Compactar segmentos antigos, TTL adaptativo, tarball baseado em business logic. | 6h |
| R8 | **Exploit em ffmpeg / navegador headless** | Média | Crítico | Eng. Segurança | Sandboxing, namespaces, atualizações pinadas, no-exec em /tmp, varscan diário; **AI-powered threat detection**. | 2h |
| R9 | **Banimento de CDN / host (conteúdo adulto)** | Média | Crítico | Ops / Legal | Usar CDN "adult-friendly"; contrato explícito; backup CDN (cutover automático); **intelligent CDN selection**. | 30 min |
| R10 | **Problema com monetização / congelamento de pagamentos** | Média | Alto | Financeiro / Legal | Processadores compatíveis com adulto; ledger assinado; reconciliação adaptativa; **AI fraud detection**. | 24h |
| R11 | **Latência alta (>threshold configurável)** | Média | Médio | Eng. Vídeo | Ajustar HLS clássico; Low-Latency HLS se viável; TTL adaptativa; **intelligent routing**. | 4h |
| R12 | **Fingerprint bloqueado / anti-bot detection** | Alta | Médio | Eng. Automação | Perfis estáveis e limitados; rotatividade inteligente; whitelists; **LLM anti-bot strategies**. | 2h |
| R13 | **Falha em logs (sem spans)** | Média | Médio | Eng. Observabilidade | Telemetria mínima: contadores por etapa + logs estruturados; **intelligent log analysis**. | 1h |
| R14 | **Falha elétrica / sobrecarga térmica** | Baixa | Alto | Eng. Infraestrutura | UPS configurável, sensores de temperatura, limpeza adaptativa, alerta remoto; **predictive thermal management**. | 10 min |
| R15 | **Incidente jurídico / bloqueio CNPD** | Baixa | Crítico | Jurídico / DPO | Conformidade plena GDPR, cooperação e registro de logs de consentimento; **AI compliance monitoring**. | 12h |
| **R16** | **Business Logic corruption / invalid config** | Média | Alto | Eng. Business Logic | Validation schema, rollback capability, signed configs; **automatic integrity checking**. | 30 min |
| **R17** | **LLM service outage / API failures** | Alta | Médio | Eng. AI Integration | Circuit breakers, fallback to deterministic mode, cost budgets; **graceful degradation**. | 5 min |
| **R18** | **Curator Vigilante false positives** | Média | Médio | Eng. Curator | Confidence thresholds, token bucket limits, manual override; **pattern learning**. | 1h |
| **R19** | **Adaptive system instability / oscillation** | Baixa | Alto | Eng. Adaptive Systems | Stability constraints, change rate limits, rollback triggers; **oscillation detection**. | 15 min |
| **R20** | **AI bias in content selection** | Média | Alto | Eng. AI Ethics | Bias detection, diverse training data, human oversight; **fairness monitoring**. | 4h |

* * *

### 🔧 Escala de Classificação Inteligente

**Probabilidade:**

*   Baixa: <10 % / ano
*   Média: 10–50 % / ano
*   Alta: >50 % / ano

**Impacto:**

*   Médio: interrupção ≤ 1 h ou dano reversível
*   Alto: interrupção ≥ 6 h ou dano reputacional moderado
*   Crítico: perda de dados ou exposição legal grave
*   Catastrófico: implicações criminais, perda institucional

**Novos Critérios para Sistemas Inteligentes:**

*   **Intelligence Impact**: perda de capacidades adaptativas ou learning data
*   **Business Logic Impact**: corrupção de configuração ou evolution history
*   **AI Service Impact**: dependência de serviços externos de IA

* * *

### 📈 Resumo de Prioridades (Heat Map) Inteligente

| Categoria | Riscos Críticos | Prioridade | Observações |
| --- | --- | --- | --- |
| Legal / Compliance | R1, R2, R3, R4, R15, R20 | 🔥 | manter consultoria jurídica ativa + AI ethics review |
| Operacional | R5, R6, R7, R9 | ⚙️ | reforçar redundância e automação inteligente |
| Segurança | R8, R12 | 🔒 | sandboxes separados por domínio + AI threat detection |
| Financeira | R10 | 💶 | usar gateway redundante + AI fraud detection |
| Técnica / Observabilidade | R11, R13 | 🧠 | spans opcionais + logs inteligentes |
| Física | R14 | 🧯 | monitoramento físico e remoto + predictive maintenance |
| **Business Logic** | **R16, R19** | **🎯** | **validation schemas + stability monitoring** |
| **AI Integration** | **R17, R18, R20** | **🤖** | **circuit breakers + bias monitoring + graceful degradation** |

* * *

### 📋 Plano de Revisão Inteligente

| Ação | Frequência | Responsável | Entregável |
| --- | --- | --- | --- |
| Auditoria Legal / Consentimento + AI Ethics | Mensal | Jurídico + AI Ethics | Relatório "VVTV\_Compliance\_AI\_Audit.md" |
| Teste de Buffer e Loop de Emergência Inteligente | Semanal | Eng. Vídeo | Log de Teste (`intelligent_buffer_test.log`) |
| Sandbox Integrity Check + AI Security | Diário | Eng. Segurança | `security_ai_check_report.json` |
| Monitoramento de UPS e Temperatura Preditivo | Contínuo | Infraestrutura | Alertas Telegram / Email com AI insights |
| Revisão de Monetização + AI Bias Detection | Quinzenal | Financeiro | `ledger_ai_bias_reconciliation.csv` |
| **Business Logic Validation** | **Diário** | **Eng. Business Logic** | **`business_logic_health_report.json`** |
| **LLM Service Health Check** | **Horário** | **Eng. AI Integration** | **`llm_service_health.json`** |
| **Curator Pattern Analysis** | **Semanal** | **Eng. Curator** | **`curator_pattern_analysis.json`** |
| **Adaptive System Stability Review** | **Diário** | **Eng. Adaptive Systems** | **`adaptive_stability_report.json`** |

* * *

### ⚖️ Conclusão Inteligente

O **VVTV Risk Register Inteligente** define o perímetro de segurança e resiliência do sistema híbrido.  
Cada linha é um elo de proteção que considera não apenas riscos técnicos tradicionais, mas também **riscos de sistemas inteligentes**, **business logic integrity** e **AI service dependencies**.

Nenhum risco pode ser ignorado — apenas mitigado, observado e **aprendido**.  
O verdadeiro uptime não é 99.9 % — é **99.9 % de coerência institucional inteligente** com **capacidade adaptativa preservada**.

* * *

* * *

## 📘 APÊNDICE B — VVTV INCIDENT PLAYBOOK

### _VoulezVous.TV – Emergency Response Procedures for Hybrid Intelligent Systems_

* * *

### 🚨 Incident Classification Matrix

| Severity | Description | Response Time | Escalation | AI System Impact |
|----------|-------------|---------------|------------|------------------|
| **P0 - Critical** | Stream down, legal violation, security breach | 5 minutes | Immediate | Full AI shutdown if needed |
| **P1 - High** | Quality degradation, buffer underflow, LLM failures | 15 minutes | 1 hour | Circuit breaker activation |
| **P2 - Medium** | Performance issues, business logic anomalies | 1 hour | 4 hours | Adaptive system adjustment |
| **P3 - Low** | Minor bugs, optimization opportunities | 24 hours | 1 week | Learning data collection |

### 🔧 Standard Response Procedures

#### 🚨 Incident Type: Stream Freeze / Black Screen

**Symptoms:**
- HLS stream shows black screen or frozen frame
- RTMP encoder appears running but no new segments
- Viewer complaints or monitoring alerts

**Immediate Actions (0-5 minutes):**
```bash
# 1. Check encoder status
systemctl status vvtv_broadcast
ps aux | grep ffmpeg

# 2. Check queue status
vvtvctl queue status --format json
sqlite3 /vvtv/data/queue.sqlite "SELECT COUNT(*) FROM playout_queue WHERE status='queued';"

# 3. Emergency restart if needed
/vvtv/system/bin/emergency_restart_encoder.sh

# 4. Verify stream recovery
curl -I http://localhost:8080/hls/main.m3u8
```

**Root Cause Analysis:**
```bash
# Check recent logs
tail -100 /vvtv/system/logs/broadcaster.log | grep ERROR
tail -100 /vvtv/system/logs/business_logic.log | grep WARN

# Check system resources
top -p $(pgrep ffmpeg)
df -h /vvtv/storage/ready/

# Check business logic health
vvtvctl business-logic show --format json | jq '.status'
```

**Recovery Steps:**
1. If queue empty → trigger emergency content loop
2. If encoder crashed → restart with last known good config
3. If business logic corrupted → rollback to stable version
4. If LLM integration failed → activate circuit breaker

#### 🚨 Incident Type: Buffer Underflow (Fila Seca)

**Symptoms:**
- Queue has <2 hours of content
- Planner not generating new plans
- Browser automation stuck or failing

**Immediate Actions (0-15 minutes):**
```bash
# 1. Check buffer status
vvtvctl queue buffer-status
vvtvctl business-logic show | grep buffer_target_hours

# 2. Check planner health
vvtvctl planner status
tail -50 /vvtv/system/logs/planner.log

# 3. Emergency content injection
vvtvctl queue inject-emergency-content --hours 4

# 4. Check browser automation
vvtvctl browser status
ps aux | grep chromium
```

**Business Logic Checks:**
```bash
# Verify configuration integrity
vvtvctl business-logic validate

# Check adaptive parameters
vvtvctl business-logic show --section scheduling
vvtvctl business-logic show --section selection

# Review recent decisions
sqlite3 /vvtv/data/plans.sqlite "SELECT plan_id, curation_score, status, updated_at FROM plans ORDER BY updated_at DESC LIMIT 10;"
```

**Recovery Actions:**
1. **Immediate:** Activate emergency loop with existing content
2. **Short-term:** Restart browser automation with fresh profiles
3. **Medium-term:** Adjust business logic parameters if needed
4. **Long-term:** Analyze root cause and update adaptive parameters

#### 🚨 Incident Type: LLM Service Outage

**Symptoms:**
- Circuit breaker in OPEN state
- High latency or timeout errors from LLM API
- Fallback to deterministic mode activated

**Immediate Actions (0-5 minutes):**
```bash
# 1. Check LLM service status
vvtvctl llm status --detailed
vvtvctl curator status

# 2. Verify circuit breaker state
vvtvctl llm status | grep circuit_breaker_state

# 3. Test connectivity
vvtvctl llm test --endpoint https://api.openai.com/v1/chat/completions

# 4. Check budget and rate limits
vvtvctl llm stats --current-hour
```

**Graceful Degradation:**
```bash
# Ensure deterministic mode is working
vvtvctl business-logic test-selection --no-llm --dry-run

# Check curator vigilante fallback
vvtvctl curator review --dry-run --no-llm

# Verify planner continues without LLM
tail -20 /vvtv/system/logs/planner.log | grep fallback
```

**Recovery Strategy:**
1. **Immediate:** Confirm system operates in deterministic mode
2. **Monitor:** Circuit breaker auto-recovery (typically 5-15 minutes)
3. **Escalate:** If outage >1 hour, consider alternative LLM provider
4. **Document:** Log incident for future circuit breaker tuning

#### 🚨 Incident Type: Business Logic Corruption

**Symptoms:**
- Configuration validation failures
- Unexpected parameter values
- System behavior anomalies

**Immediate Actions (0-10 minutes):**
```bash
# 1. Validate current configuration
vvtvctl business-logic validate --verbose

# 2. Check configuration history
ls -la /vvtv/system/business_logic_backups/
vvtvctl business-logic history --last 24h

# 3. Emergency rollback if needed
vvtvctl business-logic rollback --to-stable

# 4. Verify system stability
vvtvctl business-logic test-selection --dry-run
```

**Integrity Verification:**
```bash
# Check configuration signature
vvtvctl business-logic verify-signature

# Compare with known good version
diff /vvtv/system/business_logic.yaml /vvtv/system/business_logic_backups/stable.yaml

# Validate all parameters are within bounds
vvtvctl business-logic validate --check-bounds --verbose
```

**Recovery Process:**
1. **Immediate:** Rollback to last known stable configuration
2. **Analysis:** Identify source of corruption (manual edit, system error, etc.)
3. **Validation:** Test rolled-back configuration thoroughly
4. **Prevention:** Implement additional integrity checks if needed

#### 🚨 Incident Type: Adaptive System Oscillation

**Symptoms:**
- Rapid changes in programming parameters
- Unstable selection patterns
- High variance in metrics

**Detection and Analysis:**
```bash
# 1. Check adaptive system metrics
vvtvctl metrics show --category adaptive --last 4h

# 2. Review recent parameter changes
vvtvctl business-logic history --changes-only --last 24h

# 3. Analyze selection entropy
sqlite3 /vvtv/data/metrics.sqlite "SELECT ts, selection_entropy FROM metrics WHERE ts > datetime('now', '-4 hours') ORDER BY ts;"

# 4. Check oscillation detection
vvtvctl adaptive-system status --oscillation-check
```

**Stabilization Actions:**
```bash
# 1. Temporarily disable autopilot
vvtvctl business-logic set autopilot.enabled false

# 2. Set conservative parameters
vvtvctl business-logic set exploration.epsilon 0.05
vvtvctl business-logic set selection.temperature 0.7

# 3. Enable stability monitoring
vvtvctl adaptive-system enable-stability-monitoring

# 4. Gradual re-enablement
# (after 2-4 hours of stable operation)
vvtvctl business-logic set autopilot.enabled true
vvtvctl business-logic set autopilot.max_daily_variation 0.02
```

### 🔍 Diagnostic Commands Reference

#### System Health Overview
```bash
# Complete system status
/vvtv/system/bin/check_stream_health.sh

# Business logic health
vvtvctl business-logic show --health-check

# AI systems status
vvtvctl llm status && vvtvctl curator status

# Resource utilization
htop -p $(pgrep -d, -f vvtv)
```

#### Performance Analysis
```bash
# Stream quality metrics
vvtvctl metrics show --category quality --last 1h

# Business logic performance
vvtvctl metrics show --category business_logic --last 4h

# LLM usage and costs
vvtvctl llm stats --detailed --last 24h

# Curator intervention analysis
vvtvctl curator history --analysis --last 7d
```

#### Data Integrity Checks
```bash
# Database integrity
sqlite3 /vvtv/data/plans.sqlite "PRAGMA integrity_check;"
sqlite3 /vvtv/data/queue.sqlite "PRAGMA integrity_check;"

# Configuration integrity
vvtvctl business-logic validate --full-check

# File system integrity
find /vvtv/storage/ready -name "*.mp4" -exec ffprobe -v error {} \; 2>&1 | grep -v "^$"
```

### 📊 Escalation Matrix

#### Internal Escalation
| Level | Role | Contact Method | Response Time |
|-------|------|----------------|---------------|
| L1 | On-call Engineer | Telegram Bot | 5 minutes |
| L2 | Senior Engineer | Phone + Email | 15 minutes |
| L3 | System Architect | Emergency Line | 30 minutes |
| L4 | CTO / Founder | All channels | 1 hour |

#### External Escalation
| Incident Type | External Contact | When to Escalate |
|---------------|------------------|------------------|
| Legal/DMCA | Legal Counsel | Immediately for P0 legal issues |
| Security Breach | Security Firm | Within 1 hour of confirmed breach |
| CDN Issues | CDN Provider | If >30min outage affecting >50% users |
| LLM Provider Issues | Provider Support | If circuit breaker fails to recover in 2h |

### 📝 Incident Documentation Template

```markdown
# Incident Report: [YYYY-MM-DD-HH:MM] - [Brief Description]

## Summary
- **Incident ID:** INC-[YYYYMMDD]-[###]
- **Severity:** P[0-3]
- **Start Time:** [UTC timestamp]
- **End Time:** [UTC timestamp]
- **Duration:** [HH:MM]
- **Impact:** [Description of user/system impact]

## Timeline
- **[HH:MM]** - Initial detection/alert
- **[HH:MM]** - Response team engaged
- **[HH:MM]** - Root cause identified
- **[HH:MM]** - Fix implemented
- **[HH:MM]** - Service restored
- **[HH:MM]** - Incident closed

## Root Cause Analysis
### What Happened
[Detailed description of the incident]

### Why It Happened
[Root cause analysis]

### Business Logic Impact
[How business logic/AI systems were affected]

## Resolution
### Immediate Actions Taken
[List of immediate response actions]

### Permanent Fix
[Long-term solution implemented]

## Lessons Learned
### What Went Well
[Positive aspects of the response]

### What Could Be Improved
[Areas for improvement]

### Action Items
- [ ] [Action item 1] - Owner: [Name] - Due: [Date]
- [ ] [Action item 2] - Owner: [Name] - Due: [Date]

## Metrics Impact
### Before Incident
- Stream uptime: [%]
- Selection entropy: [value]
- LLM success rate: [%]

### During Incident
- Service degradation: [description]
- Fallback activation: [Y/N]
- User impact: [estimated viewers affected]

### After Resolution
- Recovery time: [minutes]
- System stability: [assessment]
- Preventive measures: [implemented]

## Configuration Changes
### Business Logic
[Any changes to business_logic.yaml]

### System Configuration
[Any changes to system configuration]

### AI System Adjustments
[Changes to LLM integration, circuit breakers, etc.]
```

### 🔄 Post-Incident Review Process

#### Immediate (Within 24 hours)
1. **Document incident** using template above
2. **Verify fix** is working as expected
3. **Update monitoring** if gaps were identified
4. **Communicate** to stakeholders

#### Short-term (Within 1 week)
1. **Conduct blameless post-mortem** with team
2. **Implement action items** from lessons learned
3. **Update runbooks** and procedures
4. **Test incident response** procedures

#### Long-term (Within 1 month)
1. **Analyze incident trends** and patterns
2. **Update business logic** parameters if needed
3. **Improve AI system resilience** based on learnings
4. **Conduct tabletop exercises** for similar scenarios

### 🤖 AI-Assisted Incident Response

#### Automated Detection
```rust
// Example: Automated anomaly detection
pub struct IncidentDetector {
    business_logic: Arc<BusinessLogic>,
    metrics_collector: MetricsCollector,
    alert_thresholds: AlertThresholds,
}

impl IncidentDetector {
    pub async fn check_system_health(&mut self) -> Vec<IncidentAlert> {
        let mut alerts = Vec::new();
        
        // Check stream health
        if let Ok(stream_metrics) = self.metrics_collector.get_stream_metrics().await {
            if stream_metrics.uptime_percentage < 0.95 {
                alerts.push(IncidentAlert::new(
                    Severity::High,
                    "Stream uptime below threshold",
                    format!("Current uptime: {:.2}%", stream_metrics.uptime_percentage * 100.0)
                ));
            }
        }
        
        // Check business logic health
        if let Err(e) = self.business_logic.validate() {
            alerts.push(IncidentAlert::new(
                Severity::Critical,
                "Business logic validation failed",
                format!("Validation error: {}", e)
            ));
        }
        
        // Check AI system health
        if self.llm_circuit_breaker_open() {
            alerts.push(IncidentAlert::new(
                Severity::Medium,
                "LLM circuit breaker open",
                "System operating in deterministic fallback mode"
            ));
        }
        
        alerts
    }
}
```

#### Intelligent Diagnostics
```bash
# AI-powered log analysis
vvtvctl diagnose --ai-analysis --last 1h

# Predictive failure detection
vvtvctl predict --failure-probability --next 4h

# Automated root cause suggestions
vvtvctl analyze-incident --incident-id INC-20251022-001 --suggest-fixes
```

### 📈 Incident Metrics and KPIs

#### Response Metrics
- **Mean Time to Detection (MTTD):** Target <5 minutes
- **Mean Time to Response (MTTR):** Target <15 minutes for P1
- **Mean Time to Resolution (MTTR):** Target <1 hour for P1
- **Incident Recurrence Rate:** Target <10%

#### Business Impact Metrics
- **Stream Availability:** Target >99.9%
- **Business Logic Stability:** Target >99.5%
- **AI System Reliability:** Target >95% (with graceful degradation)
- **User Experience Impact:** Target <1% of viewers affected per incident

#### Learning and Improvement Metrics
- **Action Item Completion Rate:** Target >90%
- **Runbook Accuracy:** Target >95%
- **Incident Prevention Rate:** Target 20% reduction year-over-year
- **Team Response Confidence:** Target >4.0/5.0 in post-incident surveys

### 🔚 Conclusion

This incident playbook provides comprehensive procedures for responding to various types of incidents in the VVTV hybrid intelligent system. The combination of automated detection, structured response procedures, and AI-assisted diagnostics ensures rapid resolution while maintaining system stability and learning from each incident to prevent future occurrences.

Regular drills and updates to this playbook are essential to maintain response effectiveness as the system evolves and new types of incidents emerge.

* * *##
 📘 APÊNDICE C — BUSINESS LOGIC SCHEMA

### _VoulezVous.TV – Complete YAML Configuration Schema and Validation Rules_

* * *

### 🎯 Schema Overview

The business logic configuration is the "DNA" of the VVTV system, controlling all adaptive behavior, selection algorithms, and AI integration parameters. This appendix provides the complete schema definition, validation rules, and configuration examples.

### 📋 Complete Schema Definition

```yaml
# business_logic.yaml - Complete Schema
# Version: 2025.10
# Validation: JSON Schema + Rust type system

# REQUIRED: Policy metadata
policy_version: string          # Format: "YYYY.MM" (e.g., "2025.10")
env: string                     # Values: "development" | "staging" | "production"

# REQUIRED: Programming control knobs
knobs:
  boost_bucket: string          # Values: "music" | "documentary" | "creative" | "mixed"
  music_mood_focus: [string]    # Array of mood tags (e.g., ["focus", "midnight", "energetic"])
  interstitials_ratio: float    # Range: 0.0-0.2 (0% to 20% of content)
  plan_selection_bias: float    # Range: -0.2 to +0.2 (negative = conservative, positive = adventurous)

# REQUIRED: Temporal scheduling parameters
scheduling:
  slot_duration_minutes: int    # Values: 5, 10, 15, 20, 30 (broadcast slot length)
  global_seed: int             # Range: 1-999999 (for reproducible randomness)

# REQUIRED: Selection algorithm configuration
selection:
  method: string               # Values: "gumbel_top_k" | "weighted_random" | "deterministic"
  temperature: float           # Range: 0.1-2.0 (lower = more deterministic)
  top_k: int                  # Range: 5-50 (number of candidates to consider)
  seed_strategy: string        # Values: "slot_hash" | "time_based" | "fixed"

# REQUIRED: Exploration vs exploitation balance
exploration:
  epsilon: float               # Range: 0.0-0.5 (0% to 50% random exploration)

# OPTIONAL: Autopilot adaptive system
autopilot:
  enabled: bool               # Default: false
  max_daily_variation: float  # Range: 0.01-0.1 (1% to 10% max change per day)
  learning_rate: float        # Range: 0.001-0.1 (how fast to adapt)
  stability_threshold: float  # Range: 0.8-0.99 (minimum stability before changes)

# REQUIRED: Key Performance Indicators
kpis:
  primary: [string]           # Primary metrics to optimize (e.g., ["selection_entropy"])
  secondary: [string]         # Secondary metrics to monitor

# OPTIONAL: LLM integration settings
llm_integration:
  enabled: bool               # Default: true
  max_budget_eur_per_hour: float  # Range: 0.0-1.0 (hourly spending limit)
  circuit_breaker:
    failure_threshold: float  # Range: 0.1-0.5 (failure rate to trip breaker)
    window_size: int         # Range: 10-100 (number of requests to consider)
    timeout_seconds: int     # Range: 1-10 (request timeout)
  content_analysis:
    enabled: bool            # Enable LLM content analysis
    confidence_threshold: float  # Range: 0.5-0.95 (minimum confidence to apply)
  query_enhancement:
    enabled: bool            # Enable LLM query enhancement
    creativity: float        # Range: 0.1-1.0 (how creative to be with queries)

# OPTIONAL: Curator vigilante settings
curator_vigilante:
  enabled: bool              # Default: true
  confidence_threshold: float # Range: 0.6-0.9 (minimum confidence to intervene)
  token_bucket:
    capacity: int            # Range: 1-20 (max interventions stored)
    refill_rate_per_hour: int # Range: 1-10 (interventions per hour)
  signals:
    tag_duplication_threshold: int     # Range: 2-10 (max same tags)
    score_variance_threshold: float    # Range: 0.05-0.3 (min score diversity)
    temporal_clustering_threshold: float # Range: 0.5-0.9 (max temporal clustering)

# OPTIONAL: Quality control parameters
quality_control:
  min_duration_seconds: int   # Range: 30-3600 (minimum content duration)
  max_duration_seconds: int   # Range: 300-7200 (maximum content duration)
  min_resolution_height: int  # Range: 480-2160 (minimum video height)
  min_bitrate_kbps: int      # Range: 500-10000 (minimum bitrate)
  audio_normalization:
    target_lufs: float       # Range: -23.0 to -14.0 (broadcast standard)
    max_true_peak: float     # Range: -3.0 to -1.0 (peak limiting)

# OPTIONAL: Advanced adaptive parameters
adaptive_parameters:
  audience_feedback_weight: float     # Range: 0.0-1.0 (how much to weight audience metrics)
  time_of_day_adaptation: bool       # Enable time-based programming adaptation
  geographic_adaptation: bool        # Enable location-based adaptation
  seasonal_adaptation: bool          # Enable seasonal programming changes
  content_freshness_weight: float    # Range: 0.0-1.0 (preference for newer content)
  diversity_enforcement: float       # Range: 0.0-1.0 (how much to enforce diversity)

# OPTIONAL: Emergency and fallback settings
emergency:
  buffer_critical_hours: float       # Range: 0.5-4.0 (when to trigger emergency mode)
  emergency_loop_content: [string]   # List of emergency content plan IDs
  max_emergency_duration_hours: int  # Range: 1-24 (max time in emergency mode)
  fallback_selection_method: string  # Values: "deterministic" | "simple_random"

# OPTIONAL: Monitoring and alerting
monitoring:
  metrics_collection_interval_seconds: int  # Range: 30-300 (how often to collect metrics)
  alert_thresholds:
    low_buffer_hours: float          # Range: 1.0-6.0 (alert when buffer low)
    high_failure_rate: float         # Range: 0.05-0.2 (alert on high failures)
    low_selection_entropy: float     # Range: 0.3-0.7 (alert on low diversity)
  health_check_interval_seconds: int # Range: 60-600 (system health check frequency)

# OPTIONAL: Development and testing
development:
  debug_mode: bool           # Enable additional logging and validation
  dry_run_mode: bool         # Test configuration without applying changes
  simulation_mode: bool      # Run in simulation without real content
  test_data_seed: int        # Seed for generating test data
```

### 🔍 Validation Rules

#### Type Validation
```rust
// Rust type definitions for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessLogicConfig {
    pub policy_version: String,
    pub env: Environment,
    pub knobs: ProgrammingKnobs,
    pub scheduling: SchedulingConfig,
    pub selection: SelectionConfig,
    pub exploration: ExplorationConfig,
    pub autopilot: Option<AutopilotConfig>,
    pub kpis: KpiConfig,
    pub llm_integration: Option<LlmIntegrationConfig>,
    pub curator_vigilante: Option<CuratorVigilanteConfig>,
    pub quality_control: Option<QualityControlConfig>,
    pub adaptive_parameters: Option<AdaptiveParametersConfig>,
    pub emergency: Option<EmergencyConfig>,
    pub monitoring: Option<MonitoringConfig>,
    pub development: Option<DevelopmentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

// Validation constraints
impl BusinessLogicConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Policy version format check
        if !self.policy_version.matches(r"^\d{4}\.\d{2}$") {
            return Err(ValidationError::InvalidPolicyVersion);
        }
        
        // Knobs validation
        self.knobs.validate()?;
        
        // Selection parameters validation
        if self.selection.temperature < 0.1 || self.selection.temperature > 2.0 {
            return Err(ValidationError::TemperatureOutOfRange);
        }
        
        if self.selection.top_k < 5 || self.selection.top_k > 50 {
            return Err(ValidationError::TopKOutOfRange);
        }
        
        // Exploration validation
        if self.exploration.epsilon < 0.0 || self.exploration.epsilon > 0.5 {
            return Err(ValidationError::EpsilonOutOfRange);
        }
        
        // Cross-parameter validation
        if let Some(ref autopilot) = self.autopilot {
            if autopilot.enabled && self.env == Environment::Production {
                if autopilot.max_daily_variation > 0.05 {
                    return Err(ValidationError::AutopilotTooAggressive);
                }
            }
        }
        
        Ok(())
    }
}
```

#### Business Rules Validation
```rust
impl ProgrammingKnobs {
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Boost bucket validation
        match self.boost_bucket.as_str() {
            "music" | "documentary" | "creative" | "mixed" => {},
            _ => return Err(ValidationError::InvalidBoostBucket),
        }
        
        // Interstitials ratio validation
        if self.interstitials_ratio < 0.0 || self.interstitials_ratio > 0.2 {
            return Err(ValidationError::InterstitialsRatioOutOfRange);
        }
        
        // Plan selection bias validation
        if self.plan_selection_bias < -0.2 || self.plan_selection_bias > 0.2 {
            return Err(ValidationError::SelectionBiasOutOfRange);
        }
        
        // Music mood focus validation
        for mood in &self.music_mood_focus {
            if !VALID_MOODS.contains(&mood.as_str()) {
                return Err(ValidationError::InvalidMoodTag(mood.clone()));
            }
        }
        
        Ok(())
    }
}

const VALID_MOODS: &[&str] = &[
    "focus", "midnight", "energetic", "calm", "upbeat", 
    "melancholic", "romantic", "intense", "ambient", "rhythmic"
];
```

### 📝 Configuration Examples

#### Production Configuration
```yaml
# production_business_logic.yaml
policy_version: "2025.10"
env: "production"

knobs:
  boost_bucket: "music"
  music_mood_focus: ["focus", "midnight"]
  interstitials_ratio: 0.08
  plan_selection_bias: 0.0

scheduling:
  slot_duration_minutes: 15
  global_seed: 4242

selection:
  method: "gumbel_top_k"
  temperature: 0.85
  top_k: 12
  seed_strategy: "slot_hash"

exploration:
  epsilon: 0.12

autopilot:
  enabled: false  # Conservative for production
  max_daily_variation: 0.03
  learning_rate: 0.01
  stability_threshold: 0.9

kpis:
  primary: ["selection_entropy"]
  secondary: ["curator_apply_budget_used_pct", "audience_retention"]

llm_integration:
  enabled: true
  max_budget_eur_per_hour: 0.05
  circuit_breaker:
    failure_threshold: 0.2
    window_size: 20
    timeout_seconds: 3
  content_analysis:
    enabled: true
    confidence_threshold: 0.7
  query_enhancement:
    enabled: true
    creativity: 0.6

curator_vigilante:
  enabled: true
  confidence_threshold: 0.75
  token_bucket:
    capacity: 5
    refill_rate_per_hour: 2
  signals:
    tag_duplication_threshold: 3
    score_variance_threshold: 0.1
    temporal_clustering_threshold: 0.7

quality_control:
  min_duration_seconds: 60
  max_duration_seconds: 1800
  min_resolution_height: 720
  min_bitrate_kbps: 2000
  audio_normalization:
    target_lufs: -14.0
    max_true_peak: -1.0

adaptive_parameters:
  audience_feedback_weight: 0.3
  time_of_day_adaptation: true
  geographic_adaptation: false
  seasonal_adaptation: true
  content_freshness_weight: 0.2
  diversity_enforcement: 0.8

emergency:
  buffer_critical_hours: 2.0
  emergency_loop_content: ["emergency_loop_1", "emergency_loop_2"]
  max_emergency_duration_hours: 4
  fallback_selection_method: "deterministic"

monitoring:
  metrics_collection_interval_seconds: 60
  alert_thresholds:
    low_buffer_hours: 3.0
    high_failure_rate: 0.1
    low_selection_entropy: 0.5
  health_check_interval_seconds: 300
```

#### Development Configuration
```yaml
# development_business_logic.yaml
policy_version: "2025.10"
env: "development"

knobs:
  boost_bucket: "mixed"
  music_mood_focus: ["focus", "energetic", "ambient"]
  interstitials_ratio: 0.05
  plan_selection_bias: 0.1  # More adventurous for testing

scheduling:
  slot_duration_minutes: 5  # Shorter slots for faster testing
  global_seed: 1234

selection:
  method: "gumbel_top_k"
  temperature: 1.2  # Higher temperature for more variety
  top_k: 20
  seed_strategy: "time_based"

exploration:
  epsilon: 0.25  # Higher exploration for testing

autopilot:
  enabled: true  # Safe to test in development
  max_daily_variation: 0.1
  learning_rate: 0.05
  stability_threshold: 0.8

kpis:
  primary: ["selection_entropy", "content_diversity"]
  secondary: ["llm_success_rate", "processing_efficiency"]

llm_integration:
  enabled: true
  max_budget_eur_per_hour: 0.1  # Higher budget for testing
  circuit_breaker:
    failure_threshold: 0.3
    window_size: 10
    timeout_seconds: 5
  content_analysis:
    enabled: true
    confidence_threshold: 0.6
  query_enhancement:
    enabled: true
    creativity: 0.8

development:
  debug_mode: true
  dry_run_mode: false
  simulation_mode: false
  test_data_seed: 42
```

#### Staging Configuration
```yaml
# staging_business_logic.yaml
policy_version: "2025.10"
env: "staging"

knobs:
  boost_bucket: "music"
  music_mood_focus: ["focus", "midnight", "calm"]
  interstitials_ratio: 0.06
  plan_selection_bias: -0.05  # Slightly conservative

scheduling:
  slot_duration_minutes: 10
  global_seed: 2024

selection:
  method: "gumbel_top_k"
  temperature: 0.9
  top_k: 15
  seed_strategy: "slot_hash"

exploration:
  epsilon: 0.15

autopilot:
  enabled: true
  max_daily_variation: 0.05
  learning_rate: 0.02
  stability_threshold: 0.85

kpis:
  primary: ["selection_entropy"]
  secondary: ["curator_apply_budget_used_pct", "system_stability"]

# ... (similar structure to production but with testing-friendly values)
```

### 🔧 CLI Configuration Management

#### Validation Commands
```bash
# Validate current configuration
vvtvctl business-logic validate

# Validate specific file
vvtvctl business-logic validate --file /path/to/config.yaml

# Validate with verbose output
vvtvctl business-logic validate --verbose --check-bounds

# Dry-run validation (test without applying)
vvtvctl business-logic validate --dry-run --file new_config.yaml
```

#### Configuration Management
```bash
# Show current configuration
vvtvctl business-logic show
vvtvctl business-logic show --format json
vvtvctl business-logic show --section knobs

# Reload configuration
vvtvctl business-logic reload
vvtvctl business-logic reload --file /path/to/new_config.yaml

# Backup current configuration
vvtvctl business-logic backup --output /path/to/backup.yaml

# Restore from backup
vvtvctl business-logic restore --file /path/to/backup.yaml

# Show configuration history
vvtvctl business-logic history --last 24h
vvtvctl business-logic history --changes-only
```

#### Testing and Simulation
```bash
# Test selection algorithm
vvtvctl business-logic test-selection --plans 20 --dry-run
vvtvctl business-logic test-selection --temperature 0.9 --top-k 15

# Simulate configuration changes
vvtvctl business-logic simulate --change "exploration.epsilon=0.2" --duration 1h

# Compare configurations
vvtvctl business-logic compare --file1 current.yaml --file2 proposed.yaml

# Generate configuration template
vvtvctl business-logic template --env production > production_template.yaml
```

### 🔐 Security and Integrity

#### Configuration Signing
```bash
# Sign configuration (production requirement)
vvtvctl business-logic sign --file business_logic.yaml --key /path/to/private.key

# Verify signature
vvtvctl business-logic verify --file business_logic.yaml --key /path/to/public.key

# Show signature status
vvtvctl business-logic signature-status
```

#### Access Control
```yaml
# Configuration access control (in system config)
business_logic_access:
  read_roles: ["operator", "engineer", "admin"]
  write_roles: ["engineer", "admin"]
  sign_roles: ["admin"]
  emergency_override_roles: ["admin", "on_call_engineer"]
```

#### Audit Trail
```rust
// Automatic audit logging for all configuration changes
#[derive(Serialize)]
pub struct ConfigurationAuditLog {
    pub timestamp: DateTime<Utc>,
    pub user: String,
    pub action: ConfigurationAction,
    pub old_config_hash: String,
    pub new_config_hash: String,
    pub changes: Vec<ConfigurationChange>,
    pub validation_result: ValidationResult,
    pub signature: String,
}

#[derive(Serialize)]
pub enum ConfigurationAction {
    Load,
    Reload,
    Update,
    Rollback,
    EmergencyOverride,
}
```

### 📊 Configuration Metrics and Monitoring

#### Health Metrics
```rust
#[derive(Serialize)]
pub struct BusinessLogicHealthMetrics {
    pub config_version: String,
    pub last_reload_timestamp: DateTime<Utc>,
    pub validation_status: ValidationStatus,
    pub parameter_stability: f64,
    pub autopilot_status: AutopilotStatus,
    pub recent_changes_count: u32,
    pub emergency_mode_active: bool,
}
```

#### Performance Impact Tracking
```bash
# Monitor configuration performance impact
vvtvctl metrics show --category business_logic --last 4h

# Track parameter effectiveness
vvtvctl business-logic effectiveness --parameter exploration.epsilon --last 7d

# Analyze configuration stability
vvtvctl business-logic stability-analysis --last 30d
```

### 🚨 Emergency Procedures

#### Emergency Configuration Override
```bash
# Emergency rollback to stable configuration
vvtvctl business-logic emergency-rollback --reason "system_instability"

# Emergency parameter adjustment
vvtvctl business-logic emergency-set autopilot.enabled false --reason "oscillation_detected"

# Emergency mode activation
vvtvctl business-logic emergency-mode --activate --duration 2h
```

#### Disaster Recovery
```bash
# Restore from emergency backup
vvtvctl business-logic disaster-recovery --restore-point "2025-10-22T10:00:00Z"

# Reset to factory defaults
vvtvctl business-logic factory-reset --confirm --env production

# Generate emergency configuration
vvtvctl business-logic generate-emergency-config --output emergency.yaml
```

### 📈 Evolution and Versioning

#### Version Management
```yaml
# Version metadata in configuration
metadata:
  version: "2025.10.1"
  created_by: "system_admin"
  created_at: "2025-10-22T10:00:00Z"
  description: "Production configuration with enhanced LLM integration"
  parent_version: "2025.10.0"
  change_summary: "Added curator vigilante token bucket configuration"
```

#### Migration Support
```rust
// Automatic configuration migration
impl BusinessLogicConfig {
    pub fn migrate_from_version(old_config: &str, target_version: &str) -> Result<Self> {
        match target_version {
            "2025.10" => {
                // Migration logic from previous versions
                let mut config: BusinessLogicConfig = serde_yaml::from_str(old_config)?;
                
                // Add new fields with defaults
                if config.curator_vigilante.is_none() {
                    config.curator_vigilante = Some(CuratorVigilanteConfig::default());
                }
                
                // Update deprecated fields
                // ... migration logic
                
                Ok(config)
            }
            _ => Err(MigrationError::UnsupportedVersion(target_version.to_string()))
        }
    }
}
```

### 🔚 Conclusion

This comprehensive business logic schema provides the foundation for the VVTV system's adaptive intelligence. The combination of strict validation, flexible configuration, and robust tooling ensures that the system can evolve safely while maintaining operational stability.

The schema supports both deterministic operation and AI-enhanced adaptation, allowing the system to operate reliably in production while continuously learning and improving its performance.

* * *## 📘 
APÊNDICE D — LLM INTEGRATION PATTERNS

### _VoulezVous.TV – Large Language Model Integration Architecture, Patterns, and SLA Management_

* * *

### 🤖 Integration Philosophy

The VVTV system uses LLMs as **intelligent advisors** rather than decision makers. The core principle is **95% deterministic Rust + 5% LLM refinement**, ensuring the system remains stable and predictable while benefiting from AI insights.

### 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    LLM INTEGRATION LAYER                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │   Circuit    │    │   Token      │    │   Request    │  │
│  │   Breaker    │    │   Budget     │    │   Queue      │  │
│  │              │    │   Manager    │    │              │  │
│  └──────────────┘    └──────────────┘    └──────────────┘  │
│           │                   │                   │         │
│           └───────────────────┼───────────────────┘         │
│                               │                             │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              LLM ORCHESTRATOR                           │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │ │
│  │  │   Content   │  │   Query     │  │   Curation  │    │ │
│  │  │   Analysis  │  │   Enhancement│  │   Hints     │    │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘    │ │
│  └─────────────────────────────────────────────────────────┘ │
│                               │                             │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                FALLBACK HANDLERS                        │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │ │
│  │  │ Deterministic│  │   Cache     │  │   Default   │    │ │
│  │  │   Fallback  │  │   Responses │  │   Values    │    │ │
│  │  └─────────────┘  └─────────────┘  └─────────────┘    │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 🔌 Core Integration Patterns

#### Pattern 1: Advisory Enhancement
```rust
// LLM provides suggestions, system makes final decisions
pub async fn enhance_with_llm_advice<T, F>(
    &mut self,
    deterministic_result: T,
    llm_enhancer: F,
    confidence_threshold: f64,
) -> T 
where
    F: Fn(&T) -> BoxFuture<'_, Result<LlmAdvice<T>>>,
{
    if !self.llm_enabled() || self.circuit_breaker.is_open() {
        return deterministic_result;
    }
    
    match timeout(Duration::from_secs(3), llm_enhancer(&deterministic_result)).await {
        Ok(Ok(advice)) if advice.confidence >= confidence_threshold => {
            info!(target: "llm", "Applied LLM advice with confidence {}", advice.confidence);
            advice.enhanced_result
        }
        Ok(Ok(advice)) => {
            info!(target: "llm", "LLM advice below threshold: {}", advice.confidence);
            deterministic_result
        }
        Ok(Err(e)) => {
            warn!(target: "llm", "LLM enhancement failed: {}", e);
            self.circuit_breaker.record_failure();
            deterministic_result
        }
        Err(_) => {
            warn!(target: "llm", "LLM enhancement timeout");
            self.circuit_breaker.record_failure();
            deterministic_result
        }
    }
}
```

#### Pattern 2: Graceful Degradation
```rust
pub struct LlmService {
    primary_provider: Box<dyn LlmProvider>,
    fallback_provider: Option<Box<dyn LlmProvider>>,
    circuit_breaker: CircuitBreaker,
    cache: LruCache<String, LlmResponse>,
}

impl LlmService {
    pub async fn request_with_fallback(&mut self, request: LlmRequest) -> LlmResponse {
        // Try cache first
        if let Some(cached) = self.cache.get(&request.cache_key()) {
            return cached.clone();
        }
        
        // Try primary provider
        if !self.circuit_breaker.is_open() {
            match self.primary_provider.request(request.clone()).await {
                Ok(response) => {
                    self.circuit_breaker.record_success();
                    self.cache.put(request.cache_key(), response.clone());
                    return response;
                }
                Err(e) => {
                    warn!(target: "llm", "Primary provider failed: {}", e);
                    self.circuit_breaker.record_failure();
                }
            }
        }
        
        // Try fallback provider
        if let Some(ref mut fallback) = self.fallback_provider {
            match fallback.request(request.clone()).await {
                Ok(response) => {
                    info!(target: "llm", "Fallback provider succeeded");
                    return response;
                }
                Err(e) => {
                    warn!(target: "llm", "Fallback provider failed: {}", e);
                }
            }
        }
        
        // Return deterministic fallback
        LlmResponse::deterministic_fallback(&request)
    }
}
```

#### Pattern 3: Budget-Aware Processing
```rust
pub struct TokenBudgetManager {
    hourly_budget_eur: f64,
    current_hour_spent: f64,
    current_hour_start: DateTime<Utc>,
    cost_per_token: HashMap<String, f64>, // model -> cost
}

impl TokenBudgetManager {
    pub fn can_afford(&mut self, request: &LlmRequest) -> bool {
        self.refresh_hour_if_needed();
        
        let estimated_cost = self.estimate_cost(request);
        let remaining_budget = self.hourly_budget_eur - self.current_hour_spent;
        
        if estimated_cost <= remaining_budget {
            true
        } else {
            warn!(
                target: "llm.budget",
                estimated_cost = estimated_cost,
                remaining_budget = remaining_budget,
                "Request would exceed hourly budget"
            );
            false
        }
    }
    
    pub fn record_usage(&mut self, request: &LlmRequest, response: &LlmResponse) {
        let actual_cost = self.calculate_actual_cost(request, response);
        self.current_hour_spent += actual_cost;
        
        info!(
            target: "llm.budget",
            cost = actual_cost,
            total_spent = self.current_hour_spent,
            budget_remaining = self.hourly_budget_eur - self.current_hour_spent,
            "LLM usage recorded"
        );
    }
}
```

### 🎯 Specific Integration Points

#### Content Analysis Integration
```rust
pub struct ContentAnalyzer {
    llm_service: LlmService,
    confidence_threshold: f64,
}

impl ContentAnalyzer {
    pub async fn analyze_content(&mut self, content: &ContentMetadata) -> ContentAnalysis {
        let deterministic_analysis = self.deterministic_analysis(content);
        
        if !self.should_use_llm(content) {
            return deterministic_analysis;
        }
        
        let llm_request = LlmRequest {
            model: "gpt-4o-mini".to_string(),
            messages: vec![
                LlmMessage::system("You are a content quality analyzer for a streaming platform. Analyze content for aesthetic quality, appropriateness, and audience appeal."),
                LlmMessage::user(&format!(
                    "Analyze this content:\nTitle: {}\nDescription: {}\nDuration: {}s\nTags: {:?}\n\nProvide:\n1. Aesthetic quality score (0-1)\n2. Appropriateness score (0-1)\n3. Audience appeal score (0-1)\n4. Suggested improvements\n5. Confidence in analysis (0-1)",
                    content.title,
                    content.description,
                    content.duration_s,
                    content.tags
                ))
            ],
            max_tokens: 200,
            temperature: 0.3,
        };
        
        match self.llm_service.request_with_fallback(llm_request).await {
            LlmResponse::Success { content: llm_analysis, .. } => {
                if let Ok(parsed) = self.parse_llm_analysis(&llm_analysis) {
                    if parsed.confidence >= self.confidence_threshold {
                        return self.merge_analyses(deterministic_analysis, parsed);
                    }
                }
                deterministic_analysis
            }
            LlmResponse::Fallback { .. } => deterministic_analysis,
        }
    }
    
    fn deterministic_analysis(&self, content: &ContentMetadata) -> ContentAnalysis {
        ContentAnalysis {
            aesthetic_quality: self.calculate_aesthetic_score(content),
            appropriateness: self.check_appropriateness(content),
            audience_appeal: self.estimate_appeal(content),
            confidence: 0.7, // Deterministic confidence
            source: AnalysisSource::Deterministic,
            suggestions: self.generate_deterministic_suggestions(content),
        }
    }
}
```

#### Query Enhancement Integration
```rust
pub struct QueryEnhancer {
    llm_service: LlmService,
    enhancement_cache: LruCache<String, Vec<String>>,
}

impl QueryEnhancer {
    pub async fn enhance_search_query(&mut self, base_query: &str, context: &SearchContext) -> Vec<String> {
        // Always include original query
        let mut queries = vec![base_query.to_string()];
        
        // Check cache first
        let cache_key = format!("{}:{}", base_query, context.cache_key());
        if let Some(cached) = self.enhancement_cache.get(&cache_key) {
            queries.extend(cached.clone());
            return queries;
        }
        
        // Generate deterministic variations
        queries.extend(self.generate_deterministic_variations(base_query, context));
        
        // Try LLM enhancement
        if self.should_enhance_with_llm(base_query, context) {
            let llm_request = LlmRequest {
                model: "gpt-4o-mini".to_string(),
                messages: vec![
                    LlmMessage::system("You are a search query optimizer. Generate 2-3 enhanced search queries that would find high-quality creative content similar to the base query."),
                    LlmMessage::user(&format!(
                        "Base query: '{}'\nContext: Recent successful content was tagged with: {}\nCurrent time: {}\nGenerate enhanced queries (one per line):",
                        base_query,
                        context.successful_tags.join(", "),
                        context.timestamp.format("%Y-%m-%d %H:%M")
                    ))
                ],
                max_tokens: 100,
                temperature: 0.7,
            };
            
            if let LlmResponse::Success { content, .. } = self.llm_service.request_with_fallback(llm_request).await {
                let enhanced_queries: Vec<String> = content
                    .lines()
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty() && line != base_query)
                    .take(3)
                    .collect();
                
                if !enhanced_queries.is_empty() {
                    self.enhancement_cache.put(cache_key, enhanced_queries.clone());
                    queries.extend(enhanced_queries);
                }
            }
        }
        
        queries
    }
}
```

#### Curation Hints Integration
```rust
pub struct CurationHintsProvider {
    llm_service: LlmService,
    hint_cache: LruCache<String, CurationHints>,
}

impl CurationHintsProvider {
    pub async fn get_curation_hints(&mut self, candidates: &[ContentCandidate]) -> CurationHints {
        let deterministic_hints = self.generate_deterministic_hints(candidates);
        
        if candidates.len() < 5 || !self.should_use_llm_for_curation() {
            return deterministic_hints;
        }
        
        let candidates_summary = self.summarize_candidates(candidates);
        let cache_key = format!("curation:{}", candidates_summary.hash());
        
        if let Some(cached) = self.hint_cache.get(&cache_key) {
            return cached.clone();
        }
        
        let llm_request = LlmRequest {
            model: "gpt-4o-mini".to_string(),
            messages: vec![
                LlmMessage::system("You are a content curation expert. Analyze a set of content candidates and provide curation hints to improve selection quality and diversity."),
                LlmMessage::user(&format!(
                    "Content candidates:\n{}\n\nProvide curation hints:\n1. Diversity assessment (0-1)\n2. Quality distribution assessment\n3. Recommended reordering (if any)\n4. Content gaps to address\n5. Confidence in recommendations (0-1)",
                    self.format_candidates_for_llm(candidates)
                ))
            ],
            max_tokens: 300,
            temperature: 0.4,
        };
        
        match self.llm_service.request_with_fallback(llm_request).await {
            LlmResponse::Success { content, .. } => {
                if let Ok(hints) = self.parse_curation_hints(&content) {
                    if hints.confidence >= 0.7 {
                        self.hint_cache.put(cache_key, hints.clone());
                        return self.merge_hints(deterministic_hints, hints);
                    }
                }
                deterministic_hints
            }
            LlmResponse::Fallback { .. } => deterministic_hints,
        }
    }
}
```

### 🔧 Circuit Breaker Implementation

```rust
pub struct CircuitBreaker {
    state: CircuitBreakerState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<DateTime<Utc>>,
    config: CircuitBreakerConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitBreakerState {
    Closed,    // Normal operation
    Open,      // Blocking requests
    HalfOpen,  // Testing recovery
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            config,
        }
    }
    
    pub fn is_open(&self) -> bool {
        match self.state {
            CircuitBreakerState::Open => {
                // Check if we should transition to half-open
                if let Some(last_failure) = self.last_failure_time {
                    let elapsed = Utc::now() - last_failure;
                    elapsed > self.config.recovery_timeout
                } else {
                    true
                }
            }
            _ => false
        }
    }
    
    pub fn record_success(&mut self) {
        self.success_count += 1;
        
        match self.state {
            CircuitBreakerState::HalfOpen => {
                if self.success_count >= self.config.success_threshold {
                    info!(target: "circuit_breaker", "Transitioning to CLOSED state");
                    self.state = CircuitBreakerState::Closed;
                    self.failure_count = 0;
                    self.success_count = 0;
                }
            }
            CircuitBreakerState::Closed => {
                // Reset failure count on success
                if self.failure_count > 0 {
                    self.failure_count = 0;
                }
            }
            _ => {}
        }
    }
    
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure_time = Some(Utc::now());
        
        match self.state {
            CircuitBreakerState::Closed => {
                let failure_rate = self.failure_count as f64 / 
                    (self.failure_count + self.success_count) as f64;
                
                if failure_rate >= self.config.failure_threshold {
                    warn!(target: "circuit_breaker", "Transitioning to OPEN state");
                    self.state = CircuitBreakerState::Open;
                }
            }
            CircuitBreakerState::HalfOpen => {
                warn!(target: "circuit_breaker", "Failure in HALF_OPEN, returning to OPEN");
                self.state = CircuitBreakerState::Open;
                self.success_count = 0;
            }
            _ => {}
        }
    }
    
    pub fn attempt_request(&mut self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                if let Some(last_failure) = self.last_failure_time {
                    let elapsed = Utc::now() - last_failure;
                    if elapsed > self.config.recovery_timeout {
                        info!(target: "circuit_breaker", "Transitioning to HALF_OPEN state");
                        self.state = CircuitBreakerState::HalfOpen;
                        self.success_count = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }
}
```

### 💰 Cost Management and SLA

#### Cost Tracking
```rust
pub struct LlmCostTracker {
    costs_by_model: HashMap<String, f64>,
    costs_by_hour: HashMap<DateTime<Utc>, f64>,
    monthly_budget: f64,
    alert_thresholds: CostAlertThresholds,
}

impl LlmCostTracker {
    pub fn record_request_cost(&mut self, model: &str, tokens_used: u32, cost: f64) {
        let hour = Utc::now().date_naive().and_hms_opt(Utc::now().hour(), 0, 0).unwrap().and_utc();
        
        *self.costs_by_model.entry(model.to_string()).or_insert(0.0) += cost;
        *self.costs_by_hour.entry(hour).or_insert(0.0) += cost;
        
        // Check alert thresholds
        self.check_cost_alerts(cost);
        
        info!(
            target: "llm.cost",
            model = model,
            tokens = tokens_used,
            cost = cost,
            hourly_total = self.costs_by_hour.get(&hour).unwrap_or(&0.0),
            "LLM request cost recorded"
        );
    }
    
    pub fn get_monthly_spend(&self) -> f64 {
        let current_month_start = Utc::now().date_naive()
            .with_day(1).unwrap()
            .and_hms_opt(0, 0, 0).unwrap()
            .and_utc();
        
        self.costs_by_hour
            .iter()
            .filter(|(hour, _)| **hour >= current_month_start)
            .map(|(_, cost)| *cost)
            .sum()
    }
    
    pub fn is_within_budget(&self) -> bool {
        self.get_monthly_spend() < self.monthly_budget
    }
}
```

#### SLA Monitoring
```rust
pub struct LlmSlaMonitor {
    response_times: VecDeque<Duration>,
    success_rate_window: VecDeque<bool>,
    sla_targets: SlaTargets,
}

impl LlmSlaMonitor {
    pub fn record_request(&mut self, duration: Duration, success: bool) {
        // Track response time
        self.response_times.push_back(duration);
        if self.response_times.len() > 100 {
            self.response_times.pop_front();
        }
        
        // Track success rate
        self.success_rate_window.push_back(success);
        if self.success_rate_window.len() > 100 {
            self.success_rate_window.pop_front();
        }
        
        // Check SLA compliance
        self.check_sla_compliance();
    }
    
    pub fn get_current_metrics(&self) -> SlaMetrics {
        let avg_response_time = if self.response_times.is_empty() {
            Duration::from_secs(0)
        } else {
            let total: Duration = self.response_times.iter().sum();
            total / self.response_times.len() as u32
        };
        
        let success_rate = if self.success_rate_window.is_empty() {
            1.0
        } else {
            let successes = self.success_rate_window.iter().filter(|&&s| s).count();
            successes as f64 / self.success_rate_window.len() as f64
        };
        
        SlaMetrics {
            avg_response_time,
            success_rate,
            p95_response_time: self.calculate_p95(),
            requests_in_window: self.response_times.len(),
        }
    }
}
```

### 📊 Monitoring and Observability

#### LLM Metrics Collection
```rust
#[derive(Debug, Clone, Serialize)]
pub struct LlmMetrics {
    pub requests_total: u64,
    pub requests_successful: u64,
    pub requests_failed: u64,
    pub requests_cached: u64,
    pub avg_response_time_ms: f64,
    pub p95_response_time_ms: f64,
    pub circuit_breaker_state: CircuitBreakerState,
    pub hourly_cost_eur: f64,
    pub monthly_cost_eur: f64,
    pub budget_utilization_pct: f64,
    pub tokens_consumed: u64,
    pub cache_hit_rate: f64,
}

impl LlmMetrics {
    pub fn collect(services: &[&dyn LlmService]) -> Self {
        // Aggregate metrics from all LLM services
        let mut metrics = LlmMetrics::default();
        
        for service in services {
            let service_metrics = service.get_metrics();
            metrics.requests_total += service_metrics.requests_total;
            metrics.requests_successful += service_metrics.requests_successful;
            // ... aggregate other metrics
        }
        
        metrics
    }
}
```

#### Health Checks
```rust
pub struct LlmHealthChecker {
    services: Vec<Box<dyn LlmService>>,
    health_check_interval: Duration,
}

impl LlmHealthChecker {
    pub async fn run_health_checks(&mut self) -> LlmHealthReport {
        let mut report = LlmHealthReport::new();
        
        for service in &mut self.services {
            let start_time = Instant::now();
            
            let health_request = LlmRequest {
                model: "gpt-4o-mini".to_string(),
                messages: vec![
                    LlmMessage::user("Respond with 'OK' if you can process this request.")
                ],
                max_tokens: 5,
                temperature: 0.0,
            };
            
            match timeout(Duration::from_secs(5), service.request(health_request)).await {
                Ok(Ok(response)) => {
                    let response_time = start_time.elapsed();
                    report.add_service_health(ServiceHealth {
                        service_name: service.name(),
                        status: HealthStatus::Healthy,
                        response_time,
                        last_check: Utc::now(),
                        error: None,
                    });
                }
                Ok(Err(e)) => {
                    report.add_service_health(ServiceHealth {
                        service_name: service.name(),
                        status: HealthStatus::Unhealthy,
                        response_time: start_time.elapsed(),
                        last_check: Utc::now(),
                        error: Some(e.to_string()),
                    });
                }
                Err(_) => {
                    report.add_service_health(ServiceHealth {
                        service_name: service.name(),
                        status: HealthStatus::Timeout,
                        response_time: Duration::from_secs(5),
                        last_check: Utc::now(),
                        error: Some("Health check timeout".to_string()),
                    });
                }
            }
        }
        
        report
    }
}
```

### 🔧 CLI Integration

#### LLM Management Commands
```bash
# Service status and health
vvtvctl llm status
vvtvctl llm health-check
vvtvctl llm circuit-breaker status

# Cost and budget management
vvtvctl llm costs --current-month
vvtvctl llm budget --set-monthly 50.0
vvtvctl llm budget --alert-threshold 0.8

# Performance monitoring
vvtvctl llm metrics --last 24h
vvtvctl llm sla-report --last 7d

# Testing and diagnostics
vvtvctl llm test --model gpt-4o-mini --prompt "Hello, world!"
vvtvctl llm benchmark --duration 5m --concurrent 3

# Configuration management
vvtvctl llm config show
vvtvctl llm config set circuit_breaker.failure_threshold 0.2
vvtvctl llm config reload
```

#### Integration Testing
```bash
# Test content analysis
vvtvctl llm test-content-analysis --file sample_content.json

# Test query enhancement
vvtvctl llm test-query-enhancement --query "creative commons music"

# Test curation hints
vvtvctl llm test-curation-hints --candidates sample_candidates.json

# Stress testing
vvtvctl llm stress-test --requests 100 --concurrent 5 --duration 10m
```

### 🚨 Error Handling and Recovery

#### Error Classification
```rust
#[derive(Debug, Clone)]
pub enum LlmError {
    // Transient errors (retry possible)
    NetworkTimeout,
    RateLimitExceeded,
    ServiceUnavailable,
    
    // Permanent errors (no retry)
    InvalidApiKey,
    ModelNotFound,
    ContentPolicyViolation,
    
    // Budget errors
    BudgetExceeded,
    CostThresholdReached,
    
    // Circuit breaker errors
    CircuitBreakerOpen,
    
    // Parsing errors
    ResponseParsingFailed(String),
    UnexpectedResponseFormat,
}

impl LlmError {
    pub fn is_retryable(&self) -> bool {
        match self {
            LlmError::NetworkTimeout |
            LlmError::RateLimitExceeded |
            LlmError::ServiceUnavailable => true,
            _ => false,
        }
    }
    
    pub fn should_open_circuit_breaker(&self) -> bool {
        match self {
            LlmError::NetworkTimeout |
            LlmError::ServiceUnavailable |
            LlmError::ResponseParsingFailed(_) => true,
            _ => false,
        }
    }
}
```

#### Recovery Strategies
```rust
pub struct LlmRecoveryManager {
    retry_config: RetryConfig,
    fallback_strategies: Vec<Box<dyn FallbackStrategy>>,
}

impl LlmRecoveryManager {
    pub async fn execute_with_recovery<T, F>(&mut self, operation: F) -> Result<T, LlmError>
    where
        F: Fn() -> BoxFuture<'_, Result<T, LlmError>>,
    {
        let mut attempt = 0;
        let mut last_error = None;
        
        while attempt < self.retry_config.max_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if !error.is_retryable() {
                        return Err(error);
                    }
                    
                    last_error = Some(error);
                    attempt += 1;
                    
                    if attempt < self.retry_config.max_attempts {
                        let delay = self.retry_config.calculate_delay(attempt);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        // Try fallback strategies
        for strategy in &mut self.fallback_strategies {
            if let Ok(result) = strategy.attempt_fallback().await {
                return Ok(result);
            }
        }
        
        Err(last_error.unwrap_or(LlmError::ServiceUnavailable))
    }
}
```

### 📈 Performance Optimization

#### Request Batching
```rust
pub struct LlmRequestBatcher {
    pending_requests: Vec<(LlmRequest, oneshot::Sender<LlmResponse>)>,
    batch_size: usize,
    batch_timeout: Duration,
    last_batch_time: Instant,
}

impl LlmRequestBatcher {
    pub async fn submit_request(&mut self, request: LlmRequest) -> LlmResponse {
        let (tx, rx) = oneshot::channel();
        self.pending_requests.push((request, tx));
        
        if self.should_process_batch() {
            self.process_batch().await;
        }
        
        rx.await.unwrap_or_else(|_| LlmResponse::error("Batch processing failed"))
    }
    
    fn should_process_batch(&self) -> bool {
        self.pending_requests.len() >= self.batch_size ||
        self.last_batch_time.elapsed() >= self.batch_timeout
    }
    
    async fn process_batch(&mut self) {
        if self.pending_requests.is_empty() {
            return;
        }
        
        let batch = std::mem::take(&mut self.pending_requests);
        self.last_batch_time = Instant::now();
        
        // Process requests in parallel
        let futures: Vec<_> = batch.into_iter().map(|(request, sender)| {
            async move {
                let response = self.process_single_request(request).await;
                let _ = sender.send(response);
            }
        }).collect();
        
        futures::future::join_all(futures).await;
    }
}
```

#### Response Caching
```rust
pub struct LlmResponseCache {
    cache: LruCache<String, CachedResponse>,
    ttl: Duration,
}

#[derive(Clone)]
struct CachedResponse {
    response: LlmResponse,
    cached_at: DateTime<Utc>,
}

impl LlmResponseCache {
    pub fn get(&mut self, request: &LlmRequest) -> Option<LlmResponse> {
        let cache_key = self.generate_cache_key(request);
        
        if let Some(cached) = self.cache.get(&cache_key) {
            if Utc::now() - cached.cached_at < self.ttl {
                return Some(cached.response.clone());
            } else {
                self.cache.pop(&cache_key);
            }
        }
        
        None
    }
    
    pub fn put(&mut self, request: &LlmRequest, response: LlmResponse) {
        if response.is_cacheable() {
            let cache_key = self.generate_cache_key(request);
            self.cache.put(cache_key, CachedResponse {
                response,
                cached_at: Utc::now(),
            });
        }
    }
    
    fn generate_cache_key(&self, request: &LlmRequest) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        request.model.hash(&mut hasher);
        request.messages.hash(&mut hasher);
        request.temperature.to_bits().hash(&mut hasher);
        
        format!("llm_cache_{:x}", hasher.finish())
    }
}
```

### 🔚 Conclusion

The LLM integration patterns in VVTV provide a robust, cost-effective, and reliable way to enhance the system's intelligence while maintaining operational stability. The key principles are:

1. **LLM as Advisor, Not Decision Maker**: The system remains deterministic at its core
2. **Graceful Degradation**: Always have fallbacks when LLM services fail
3. **Cost Control**: Strict budget management and monitoring
4. **Circuit Breaker Protection**: Prevent cascading failures
5. **Comprehensive Monitoring**: Track performance, costs, and SLA compliance

This architecture ensures that the VVTV system benefits from AI capabilities while remaining resilient, predictable, and economically sustainable.

* * *

## 🎯 FINAL CONCLUSION

### _VoulezVous.TV Industrial Dossier — Complete Technical Architecture_

* * *

Este dossiê completo define a arquitetura técnica integral do **VoulezVous.TV**: um sistema de streaming autônomo 24/7 que combina **determinismo computacional Rust** com **inteligência adaptativa via LLM**, criando uma plataforma híbrida de alta performance com capacidades evolutivas.

### 🏗️ Arquitetura Híbrida Realizada

O sistema implementa uma **arquitetura híbrida** onde:
- **95% do processamento** é executado por um motor Rust determinístico e auditável
- **5% de refinamento** é fornecido por LLMs para sugestões estéticas e adaptação inteligente
- **Business Logic YAML** controla todos os parâmetros adaptativos
- **Circuit breakers** garantem resiliência contra falhas de serviços externos

### 📋 Componentes Implementados

#### Infraestrutura Base (Bloco I)
- Hardware Mac Mini M1/M2 com Tailscale mesh networking
- Estrutura de diretórios `/vvtv/` padronizada
- Configuração híbrida TOML + YAML
- Segurança baseada em SSH via Tailscale

#### Browser Automation Inteligente (Bloco II)
- Simulação humana realística com curvas Bézier
- Play-Before-Download com análise de qualidade em tempo real
- Anti-detecção com fingerprint randomization
- LLM hints para descoberta de conteúdo

#### Processamento de Mídia (Bloco III)
- Pipeline FFmpeg com perfis adaptativos
- Normalização de áudio EBU R128
- Quality control automático VMAF/SSIM
- LLM assessment para qualidade estética

#### Fila e Broadcast (Bloco IV)
- Sistema de fila SQLite com business logic integration
- RTMP/HLS origin com NGINX
- Watchdogs e recovery automático
- Curator Vigilante monitoring

#### Controle de Qualidade (Bloco V)
- Análise técnica e perceptual
- Consistência visual e auditiva
- Métricas de qualidade em tempo real
- LLM-assisted aesthetic evaluation

#### Distribuição e CDN (Bloco VI)
- Multi-CDN com failover automático
- Edge caching inteligente
- Monitoramento de latência global
- Adaptive bitrate delivery

#### Monetização e Programação Adaptativa (Bloco VII)
- Business logic YAML como "DNA" do sistema
- Algoritmo Gumbel-Top-k para seleção
- LLM Curador para refinamento
- Autopilot com feedback loops
- Economia computável com ledger auditável

#### Manutenção e Segurança (Bloco VIII)
- Backups automáticos e versionados
- Security hardening e sandboxing
- Long-term resilience planning
- AI-powered threat detection

#### Protocolos de Desligamento (Bloco IX)
- Graceful shutdown procedures
- State preservation e resurrection
- Data integrity verification
- Recovery automation

### 🤖 Inteligência Artificial Integrada

#### LLM Integration Patterns (Apêndice D)
- Circuit breakers para resiliência
- Budget management e cost control
- Graceful degradation para fallbacks
- SLA monitoring e health checks

#### Business Logic Schema (Apêndice C)
- Configuração YAML completa e validada
- Parâmetros adaptativos com bounds checking
- CLI tools para gestão e testing
- Audit trail completo

#### Incident Response (Apêndice B)
- Playbook completo para emergências
- Automated detection e alerting
- AI-assisted diagnostics
- Post-incident learning

#### Risk Management (Apêndice A)
- Matriz de riscos com AI considerations
- Mitigation strategies específicas
- Monitoring e compliance procedures
- Business continuity planning

### 🎯 Características Únicas

1. **Autonomia Total**: Opera 24/7 sem intervenção humana
2. **Inteligência Híbrida**: Combina determinismo com adaptação IA
3. **Economia Viva**: Monetização adaptativa baseada em métricas reais
4. **Resiliência Extrema**: Circuit breakers e fallbacks em todas as camadas
5. **Auditabilidade Completa**: Todas as decisões são rastreáveis e justificadas
6. **Evolução Controlada**: Aprende e adapta dentro de limites seguros

### 🔧 Ferramentas de Operação

#### CLI Unificada (`vvtvctl`)
```bash
# Business Logic
vvtvctl business-logic show|validate|reload|test-selection

# LLM Integration  
vvtvctl llm status|test|costs|health-check

# Curator System
vvtvctl curator status|review|history|tokens

# System Health
vvtvctl system health|metrics|diagnostics
```

#### Monitoramento Inteligente
- Métricas de business logic em tempo real
- SLA tracking para serviços LLM
- Adaptive system stability monitoring
- Cost tracking e budget alerts

### 📊 KPIs e Métricas

#### Performance Metrics
- **Stream Uptime**: >99.9%
- **Selection Entropy**: >0.7 (diversidade)
- **LLM Success Rate**: >95%
- **Business Logic Stability**: >99.5%

#### Economic Metrics
- **Revenue per Hour**: €0.28-0.45 (adaptativo)
- **Cost per Hour**: €0.11-0.19 (incluindo LLM)
- **Profit Margin**: 150-180%
- **LLM Budget Utilization**: <€0.05/hora

#### Quality Metrics
- **VMAF Score**: >85 (qualidade visual)
- **Audio LUFS**: -14±1 (broadcast standard)
- **Content Freshness**: Adaptativo por região
- **Audience Retention**: Monitorado e otimizado

### 🚀 Deployment e Scaling

#### Produção
- Mac Mini M1 como nó principal
- Railway para backup e scaling
- Tailscale para networking seguro
- Multi-CDN para distribuição global

#### Desenvolvimento
- Configuração local simplificada
- Testing framework integrado
- Simulation mode para desenvolvimento
- Hot-reload de configurações

### 🔮 Evolução Futura

O sistema está preparado para:
- **Novos modelos LLM**: Arquitetura provider-agnostic
- **Scaling horizontal**: Multi-node coordination
- **Novos tipos de conteúdo**: Extensibilidade via business logic
- **Regulamentações**: Compliance framework adaptável
- **Tecnologias emergentes**: Arquitetura modular e extensível

### 🎬 Conclusão Final

O **VoulezVous.TV Industrial Dossier** representa um marco na engenharia de sistemas autônomos inteligentes. Combina a confiabilidade e previsibilidade de sistemas determinísticos com a flexibilidade e adaptabilidade da inteligência artificial moderna.

Este não é apenas um sistema de streaming — é uma **plataforma de inteligência adaptativa** que:
- **Pensa** através de business logic configurável
- **Aprende** através de feedback loops e LLM insights  
- **Evolui** através de autopilot controlado
- **Sobrevive** através de resiliência extrema
- **Prospera** através de economia computável

O resultado é uma televisão verdadeiramente inteligente que opera autonomamente, adapta-se continuamente, e recompensa tanto criadores quanto audiência — estabelecendo um novo paradigma para sistemas de mídia autônomos no século XXI.

**Sistema completo. Documentação completa. Futuro computável.**

---

**VoulezVous Foundation / LogLine OS**  
**Revision v2.0 — 2025-10-22**  
**Total: 7,085+ linhas de especificação técnica completa**

* * *