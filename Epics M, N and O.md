
🕯️ Epic M — Decommission & Resurrection Protocols (Blueprint Executável)
=========================================================================

> Objetivo: desligar a VVTV **com dignidade computável**, hibernar **sem risco**, e **ressuscitar** com integridade total — incluindo trilha forense, assinatura, e “frame final” como prova.

**Escopo do Epic**

*   **M1 — Ritual de Desligamento Computável**
*   **M2 — Hibernação & Guardião**
*   **M3 — Ressurreição/Pick-up Total**
*   Segurança, evidências, testes e runbooks operacionais

* * *

📦 Entregáveis (artefatos gerados)
----------------------------------

*   `final_frame.jpg` + `final_frame.json` (metadados técnicos + assinatura)
*   `last_manifest.json` (hashes, tamanhos, versões, seeds)
*   Snapshots compactados:
    *   `snapshot_data.tar.zst` (storage, HLS, mezzanines, somente leitura)
    *   `snapshot_db.tar.zst` (SQLite \*.sqlite + WAL + schema)
    *   `snapshot_system.tar.zst` (configs, scripts, plists, versões)
*   `resurrection_playbook.md` (passo a passo verificado)
*   `signature.sig` (Ed25519 do conjunto)
*   `resurrection_report.json` (relato pós-revival)

* * *

🧭 Estado e diretórios padrão
-----------------------------

```
/vvtv/
├─ broadcast/          # HLS atual, nginx-rtmp
├─ storage/            # mezzanine e prontos
├─ data/               # *.sqlite (plans, queue, metrics, economy…)
├─ system/             # bin/, logs/, configs toml
├─ vault/              # keys/, manifests/, snapshots/
└─ monitor/            # dashboard, captures
```

* * *

✅ M1 — Ritual de Desligamento Computável
----------------------------------------

### Objetivo

Congelar o sistema **em estado íntegro**, com **prova criptográfica** e **manifesto verificável**.

### Entregas

*   Script: `/vvtv/system/bin/m1_shutdown_ritual.sh`
*   Manifesto: `/vvtv/vault/manifests/last_manifest.json`
*   Snapshot: `/vvtv/vault/snapshots/<YYYY-mm-dd_HHMM>/*`

### Fluxo (passo a passo)

1.  **Pré-check (hard stop se falhar)**
    *   Stream ativo mas **sem itens críticos rodando**:
    *   `queue_length >= 1h` (buffer garantido) **OU** `paused=true`
    *   FFmpeg estável > 60s (sem quedas)
*   Bancos íntegros:
    *   `PRAGMA integrity_check` em todos os \*.sqlite (fail → abort)
*   Disco, temp, CPU:
    *   `disk_free > 15%`, `temp_cpu < 80°C`
        2.  **Congelamento do agendador**
    *   `planner: pause`, `realizer: pause`, `discovery: pause`
*   Zerar download/transcode pendentes (aguardar até `in_progress=0`)
*   Setar `queue.freeze=true` (sem reordenações automáticas)
    3.  **Frame final + Telemetria**
    *   Capturar `final_frame.jpg` do stream público (curl+ffmpeg)
*   Extrair:
    *   `vmaf_live`, `stream_bitrate`, `segment_age`, `viewer_count?`
*   Criar `final_frame.json`:
    *   `ts_utc`, `slot_id`, `plan_id_executado`, `ffmpeg_args`, KPIs
        4.  **Manifesto & Assinatura**
    *   Calcular SHA-256 recursivo:
    *   `/broadcast`, `/storage/ready`, `/system/configs`, `/data/*.sqlite`
*   Montar `last_manifest.json`:
    *   versões (Rust, FFmpeg, nginx), seeds de seleção, business\_logic hash
*   Assinar (Ed25519) com chave do **Fundador** (`/vvtv/vault/keys/foundation_ed25519`)
    *   `signature.sig` anexado ao manifesto
        5.  **Snapshots**
    *   `snapshot_db.tar.zst` (inclui \*.sqlite + WAL + schema.sql)
*   `snapshot_system.tar.zst` (TOML, plists, bin, versões, commit ids)
*   `snapshot_data.tar.zst` (somente **mezzanines/HLS**; excluir cache/tmp)
*   Arquivar em `/vvtv/vault/snapshots/<stamp>/`
    6.  **Encerramento “limpo”**
    *   `broadcaster: stop ingest` (gravar último segmento)
*   `nginx-rtmp: stop` (graceful)
*   **Opcional**: `closing_bell.mp4` (5s) como baixa simbólica
*   Emitir `Last Transmission Manifest` em markdown
    *   “Último slot, plano, duração executada, métricas finais…”

### Script (núcleo simplificado)

```bash
#!/usr/bin/env bash
set -euo pipefail
STAMP="$(date -u +'%Y-%m-%d_%H%MZ')"
BASE="/vvtv"; VAULT="$BASE/vault"; SNAP="$VAULT/snapshots/$STAMP"
mkdir -p "$SNAP"

echo "[M1] Pré-checks…"
"$BASE/system/bin/selfcheck.sh" --strict

echo "[M1] Congelando agendadores…"
vvtvctl planner pause
vvtvctl realizer pause
vvtvctl discovery pause
vvtvctl queue freeze --on

echo "[M1] Aguardando pipelines concluírem…"
vvtvctl status --wait-empty --timeout 600

echo "[M1] Capturando frame final…"
ffmpeg -y -i "http://127.0.0.1/hls/main.m3u8" -frames:v 1 "$SNAP/final_frame.jpg"
"$BASE/system/bin/capture_telemetry.sh" > "$SNAP/final_frame.json"

echo "[M1] Calculando manifesto…"
"$BASE/system/bin/make_last_manifest.sh" > "$SNAP/last_manifest.json"
logline sign --key "$VAULT/keys/foundation_ed25519" --in "$SNAP/last_manifest.json" --out "$SNAP/signature.sig"

echo "[M1] Gerando snapshots…"
tar -I 'zstd -19' -cf "$SNAP/snapshot_db.tar.zst" -C "$BASE/data" .
tar -I 'zstd -19' -cf "$SNAP/snapshot_system.tar.zst" -C "$BASE/system" .
tar -I 'zstd -19' -cf "$SNAP/snapshot_data.tar.zst" -C "$BASE" storage broadcast

echo "[M1] Parando broadcast…"
vvtvctl broadcaster stop
sudo systemctl stop nginx-rtmp || true

echo "[M1] Last Transmission pronta em $SNAP"
```

### Critérios de aceitação

*   Snapshot verificável (hashes conferem) + assinatura válida
*   `last_manifest.json` completo (versões, seeds, hashes, tamanhos)
*   Nenhuma escrita pendente nos bancos (WAL vazia)
*   “Frame final” reproduzível

* * *

💤 M2 — Hibernação & Guardião
-----------------------------

### Objetivo

Manter o **estado estático e íntegro** até ordem de retorno, com **vigilância mínima**.

### Entregas

*   Daemon: `sleepguardd` (Rust/tokio, ciclo 15 min)
*   `resume.sh` e `lockdown.sh` (shell)

### Políticas

*   **Filesystem**: tornar **somente-leitura** tudo que for snapshotado
*   **Serviços**: _down_, watchdogs parados, timers desativados
*   **Chaves**: rotacionar Tailscale/chaves que viriam a expirar em 60d
*   **Monitoramento**: `sleepguardd` verifica:
    *   integridade (hashes do snapshot vs atual)
    *   modificações indevidas (touch/mtime)
    *   tentativa de gravação → alerta

### Fluxo

1.  **Lockdown**
    *   Remontar dirs críticos como ro (`chflags uchg` no macOS, `chattr +i` no Linux onde possível)
*   Desligar timers/launchd/systemd dos módulos
*   Ativar `sleepguardd` (somente leitura, logs locais securizados)
    2.  **Guardião**
    *   A cada 15 min:
    *   `sha256sum` de amostra (amostragem 1:N arquivos por rodada)
    *   Verifica permissões e flags de imutabilidade
    *   Checa espaço (disco não pode encher)
    *   Emite `sleepguard_report.jsonl` (append-only)
        3.  **Retorno (resume)**
    *   Inverter flags **com checklist**
*   Recarregar serviços em ordem segura:
    1.  `nginx-rtmp` (sem ingest)
    2.  bancos SQLITE (modo RW)
    3.  planner/realizer (descongelar)
    4.  broadcaster (voltar ingest)

### Scripts essenciais

```bash
# lockdown.sh
sudo chattr +i /vvtv/vault/snapshots/* || true
sudo launchctl unload -w …/planner.plist || true
sudo launchctl unload -w …/watchdog.plist || true
echo '{"state":"hibernating","ts":"'$(date -u +%FT%TZ)'"}' >> /vvtv/vault/sleepguard_state.jsonl

# resume.sh
sudo chattr -i /vvtv/vault/snapshots/* || true
sudo launchctl load -w …/nginx-rtmp.plist
sudo launchctl load -w …/db-maint.plist
vvtvctl planner resume
vvtvctl realizer resume
vvtvctl discovery resume
vvtvctl queue freeze --off
vvtvctl broadcaster start
```

### Critérios de aceitação

*   `sleepguardd` rodando, relatórios sem divergências por 24h
*   Flags de imutabilidade ativos em snapshots/manifestos
*   `resume.sh` repõe os serviços sem erro e sem perda de estado

* * *

🔁 M3 — Protocolo de Ressurreição Completo
------------------------------------------

### Objetivo

Subir **em máquina nova** (ou nó backup) **partindo do snapshot**, validando integridade e provando continuidade (frame final = frame inicial do revival).

### Entregas

*   `logline revive` (wrapper/CLI)
*   `resurrection_playbook.md` e `resurrection_report.json`

### Fluxo em nó de revival

1.  **Provisionar base**
    *   Instalar dependências mínimas (FFmpeg, nginx-rtmp, Rust runtime)
*   Criar árvore `/vvtv/…`
*   Copiar pacote `snapshot_*.tar.zst` + `last_manifest.json` + `signature.sig`
    2.  **Verificação**
    *   `logline verify`:
    *   valida assinatura (Ed25519)
    *   confere hashes no `last_manifest.json`
*   **Fail** → aborta revival (gera relatório e para)
    3.  **Restauração**
    *   Descompactar `snapshot_system` → `/vvtv/system`
*   Descompactar `snapshot_db` → `/vvtv/data`
*   Descompactar `snapshot_data` → `/vvtv/{storage,broadcast}`
*   Rodar migrações se necessário (`schema.sql`)
    4.  **Sanidade**
    *   `PRAGMA integrity_check` em todos os bancos
*   Conferir versões do binário vs manifesto (alertar se divergirem)
*   Subir `nginx-rtmp` **sem ingest** e testar `/status`
    5.  **Prova de continuidade**
    *   Iniciar `broadcaster` com o primeiro item que estava “a executar” no desligamento
*   Capturar `first_frame_after_revive.jpg` — **deve ser igual** (ou tolerância perceptual) ao `final_frame.jpg`
    *   Caso seja HLS live, valida‐se por fingerprint + metadados de playout
        6.  **Reativação**
    *   `planner/realizer` retomam com `queue.freeze=false`
*   `discovery` continua **congelado** por 24h (recomposição do buffer)
*   Watchdogs reabilitados gradualmente (encoders → nginx → fila)
    7.  **Relato**
    *   Salvar `resurrection_report.json` (tempos, divergências, correções)
*   Atualizar `vault/history/` com o Generation+1 + `ancestor_signature`

### CLI (pseudo)

```bash
logline revive \
  --snapshots /mnt/snap/2025-10-21_2130Z \
  --verify /mnt/snap/last_manifest.json:/mnt/snap/signature.sig \
  --target /vvtv \
  --strict \
  --prove-continuity
```

### Critérios de aceitação

*   Assinatura **válida** e hashes **iguais**
*   Bancos íntegros e serviços sobem **sem warnings críticos**
*   Prova de continuidade **aprovada** (frame/manifesto)
*   Relatório final gerado e arquivado

* * *

🔐 Segurança & Forense (todas as fases)
---------------------------------------

*   **Assinatura Ed25519** do manifesto e dos relatórios (cadeia de custódia)
*   `chattr +i`/`uchg` e permissões 600/700 nas chaves/relatórios
*   Logs **append-only**: `sleepguard_report.jsonl`, `m1_artifacts.jsonl`, `resurrection_report.jsonl`
*   **Air-gap opcional**: exportar snapshots para mídia offline (B2/NAS/Cold Storage)
*   Detector de DRM/CSAM permanece **ativo** e impede revival com conteúdo problemático

* * *

🧪 Testes e Ensaios
-------------------

*   **Dry-run mensal** do M1 em sandbox:
    *   Gere um snapshot pequeno (amostra de dados) e valide assinatura
*   **Revival trimestral** em nó reserva:
    *   Medir RTO (objetivo: < 30 min) e RPO (objetivo: 0, íntegro)
*   **Chaos drills**:
    *   Corrupção simulada de 1 arquivo → revival deve **falhar** com relatório claro
    *   Divergência de versão de FFmpeg → **warning** e auto‐ajuste do preset

* * *

📋 Runbooks
-----------

### Operador — Desligar hoje (M1)

1.  `vvtvctl status` → conferir filas vazias ou freeze
2.  `m1_shutdown_ritual.sh`
3.  Conferir `…/snapshots/<stamp>/*` + `signature.sig`
4.  `lockdown.sh` (se for hibernar)

### Operador — Retornar amanhã (M2)

1.  `resume.sh`
2.  `vvtvctl broadcaster start` → smoke test
3.  `planner/realizer/discovery` → retomar em sequência
4.  Monitorar buffer > 1h

### Operador — Catástrofe, novo nó (M3)

1.  Restaurar snapshots + `logline revive --strict`
2.  Validar prova de continuidade
3.  Publicar `resurrection_report.json`
4.  Atualizar `vault/history` com **Generation+1**

* * *

🔧 Tarefas por PR
-----------------

**PR M1 — Ritual de Desligamento**

*   `m1_shutdown_ritual.sh`, `make_last_manifest.sh`, `capture_telemetry.sh`
*   `logline sign/verify` integrado
*   Testes: bancos íntegros, hash recursive, assinatura OK

**PR M2 — Hibernação & Guardião**

*   `sleepguardd` (Rust), `lockdown.sh`, `resume.sh`
*   Flags imutáveis, sampling de verificação, relatórios jsonl
*   Teste: 24h sem divergência e sem alarme falso

**PR M3 — Ressurreição**

*   `logline revive` (wrapper), `resurrection_playbook.md`
*   Prova de continuidade (frame match / fingerprint)
*   Teste: revival em nó alternativo (RTO < 30 min)

* * *

🧩 Integração com o resto do sistema
------------------------------------

*   O **Epic M** conversa com:
    *   **Epic A/B** (setup base, configs, CLI)
    *   **Epic F** (queue/broadcast/watchdogs)
    *   **Epic G** (QC para prova de continuidade)
*   As chaves/assinaturas e manifestos ficam centralizados em `/vvtv/vault/…`

* * *

✅ Resumo
--------

*   **Desligar** com snapshot **assinado**, manifesto **verificável** e **frame final** arquivado.
*   **Hibernar** com **imutabilidade** e guardião ativo **sem escrever** nos dados.
*   **Ressuscitar** qualquer nó a partir de snapshots, **provando continuidade** e **sem perder estado**.

Se quiser, eu já te entrego os **scripts M1/M2/M3** prontos (com caminhos do teu ambiente) e um “mini-snapshot” de teste para ensaio de revival em 15 minutos.

## Prompt:
Prefiro que faca o epico N desse jeito que fez o M

## Response:
Pensou por 9s

perfeito — segue o **Epic N** no mesmo formato do Epic M: documento único, “executável”, pronto para entrar no repo como `docs/epics/Epic-N_Public_API_MultiTenant.md`.

* * *

🌐 Epic N — Public API & Multi-Tenant LLM Pool (Blueprint Executável)
=====================================================================

> Objetivo: expor o **LLM Pool** na rede de forma **segura, multi-tenant, com chaves**, **rate limit**, **quotas**, **observabilidade**, **SLOs** e **PRs automáticos** para governança. Pronto para uso interno e parceiros.

**Âmbito do Epic**

*   **N1 — Gateway & Auth (HMAC/JWT/mTLS)**
*   **N2 — Tenants, Quotas & Rate-Limit**
*   **N3 — Auditoria, Métricas & Faturamento (usage)**
*   **N4 — Rotação de Segredos & Onboarding Automatizado**
*   **N5 — Edge/DMZ Deployment & WAF**
*   **N6 — SLOs, Testes de Carga & Chaos Scenarios**

* * *

📦 Entregáveis (artefatos e serviços)
-------------------------------------

*   **Gateway**: NGINX/Envoy com rotas `/v1/infer`, `/v1/health`, `/v1/usage`
*   **Auth**: HMAC **e** JWT opcional (com `kid`/JWKS), suporte mTLS
*   **Tenant DB**: `tenants.sqlite` (chaves, quotas, planos, escopos)
*   **Rate-limit**: token bucket por tenant + burst, por rota
*   **Auditoria**: `audit/api_requests.jsonl` (append-only, com `request_id`)
*   **Métricas**: Prometheus + Grafana dashboards “API Health” e “Tenant Usage”
*   **Faturamento**: `usage_rollup_daily.json` (tokens, requests, bytes, latência)
*   **Runbooks**: onboarding, rotação de chaves, incidente de abuso
*   **Infra**: manifests (launchd/systemd), `nginx.conf`, `envoy.yaml`
*   **Testes**: carga (k6), latência, rate-limit, auth, caos (falha de provider)

* * *

🗂️ Estrutura de diretórios
---------------------------

```
/vvtv/
├─ api/
│  ├─ gateway/               # nginx/ ou envoy/
│  │  ├─ nginx.conf
│  │  └─ envoy.yaml
│  ├─ configs/
│  │  ├─ api.toml            # portas, thresholds, SLOs
│  │  └─ jwks.json           # opcional (se JWT externo)
│  ├─ bin/
│  │  ├─ start_gateway.sh
│  │  ├─ stop_gateway.sh
│  │  ├─ rotate_keys.sh
│  │  └─ smoke.sh
│  └─ schemas/               # JSON Schemas de requests/responses
├─ pool/                      # serviço LLM Pool já existente
├─ data/
│  └─ tenants.sqlite         # tenants, keys, quotas, planos
├─ logs/
│  ├─ api_access.log
│  ├─ api_error.log
│  └─ audit/api_requests.jsonl
└─ dashboards/
   ├─ api_health.json        # Grafana
   └─ tenant_usage.json
```

* * *

✅ N1 — Gateway & Auth
---------------------

### Rotas públicas

*   `GET /v1/health/ready` → 200 se **≥1 provider saudável** e pool pronto
*   `POST /v1/infer` → inferência (task/strategy + payload padronizado)
*   `GET /v1/usage?from=…&to=…` → métricas do tenant autenticado (resumo)
*   `POST /v1/keys/rotate` → (opcional) auto-rotação com política e webhooks

### HMAC (simples e robusto)

**Cabeçalhos requeridos:**

*   `X-Api-Key: <public_key>`
*   `X-Timestamp: <epoch_ms>`
*   `X-Nonce: <uuid4>`
*   `X-Signature: base64( HMAC_SHA256( secret, method|path|body|ts|nonce ) )`

**Validações:**

*   Chave existe e ativa no `tenants.sqlite`
*   `|now - ts| <= 5 min`
*   Nonce sem replay (cache em memória/redis 10 min)
*   Assinatura confere

**Resposta em falha:** `401/403` com `error: "AUTH_FAILED"` ou `409 NONCE_REPLAY`.

### JWT (opcional)

*   `Authorization: Bearer <jwt>`
*   Validação de `kid` em `jwks.json` ou endpoint externo
*   Claims mínimas: `tenant_id`, `scopes` (ex.: `["infer"]`), `exp <= 15m`
*   Pode coexistir com HMAC (modo “qualquer um que passar”)

### mTLS (opcional, partners premium)

*   Verificação de certificado do cliente no gateway (CA interna)
*   Mapeia `CN` → `tenant_id` (tabela de mapeamento)

### Exemplo NGINX (núcleo das rotas)

```nginx
server {
  listen 7071;
  server_name api.vvtv.local;

  # health (sem auth)
  location = /v1/health/ready { proxy_pass http://127.0.0.1:7070/health/ready; }

  # infer (com rate-limit por tenant/ip)
  location = /v1/infer {
    proxy_pass http://127.0.0.1:7070/infer;
    proxy_read_timeout 120;
    proxy_connect_timeout 2;
    limit_req zone=perip burst=20 nodelay;
    # headers repassados (auth verificada no app/pool-api)
    proxy_set_header X-Api-Key   $http_x_api_key;
    proxy_set_header X-Timestamp $http_x_timestamp;
    proxy_set_header X-Nonce     $http_x_nonce;
    proxy_set_header X-Signature $http_x_signature;
  }

  # usage (autenticado, leitura)
  location = /v1/usage {
    proxy_pass http://127.0.0.1:7070/usage;
    proxy_read_timeout 10;
    proxy_connect_timeout 2;
  }
}
```

> **Nota:** a validação **criptográfica** (HMAC/JWT), identificação de tenant e rate-limit **por tenant** são feitas no **pool-api** (camada app atrás do gateway), não apenas por IP.

* * *

📊 N2 — Tenants, Quotas & Rate-Limit
------------------------------------

### Esquema `tenants.sqlite`

```
tenants(
  id TEXT PK,
  name TEXT,
  status TEXT CHECK(status IN ('active','suspended')),
  plan TEXT,                          -- free, pro, enterprise
  api_public_key TEXT UNIQUE,
  api_secret_hash TEXT,               -- scrypt/argon2id
  rpm_limit INTEGER,                  -- requests/min
  rps_burst INTEGER,                  -- burst
  tpm_limit INTEGER,                  -- tokens/min (opcional)
  scopes TEXT,                        -- json array
  created_at, updated_at
)

nonces( tenant_id TEXT, nonce TEXT, ts INTEGER, PRIMARY KEY(tenant_id, nonce) )
usage_events( ts INTEGER, tenant_id TEXT, route TEXT, tokens_in INTEGER, tokens_out INTEGER, duration_ms INTEGER, status INTEGER, request_id TEXT )
```

### Rate-limit (token bucket)

*   **Por tenant**: `rpm_limit` (minuto), `rps_burst` (explosão curta)
*   **Por rota**: pesos (ex.: `infer=1`, `usage=0.2`)
*   **Resposta**: `429` + `Retry-After: 1`

### Quotas mensais

*   Tabela de **rollup diário** → soma mensal
*   Travas “soft” (warning 80/90%) e “hard stop” (100%, retorna 402/429)

* * *

🧾 N3 — Auditoria, Métricas & Faturamento
-----------------------------------------

### Auditoria (append-only)

Arquivo: `/vvtv/logs/audit/api_requests.jsonl`

Campos:

```json
{
  "ts":"2025-10-22T12:34:56Z",
  "request_id":"uuid",
  "tenant_id":"t_abc",
  "route":"/v1/infer",
  "task":"expand_queries",
  "strategy":"FASTEST",
  "status":200,
  "duration_ms":412,
  "tokens_in":102,
  "tokens_out":210,
  "provider":"phi3-mini",
  "from_cache":false,
  "hedge":false,
  "ip":"10.0.0.50"
}
```

### Métricas Prometheus

*   `api_http_duration_ms{route,tenant_id}` (histogram)
*   `api_requests_total{route,code}` (counter)
*   `api_rate_limited_total{tenant_id}` (counter)
*   `api_quota_exceeded_total{tenant_id}` (counter)
*   `llmpool_infer_duration_ms{provider,task}` (reuso das métricas do pool)
*   `llmpool_timeouts_total{provider,task}`

**SLOs propostos (p95, janela 7d):**

*   `/v1/infer`: **≤ 1200 ms** (após warm)
*   Erro 5xx total **< 0.5%**
*   Disponibilidade **≥ 99.5%**

### Faturamento (usage)

*   Job de **rollup diário**:
    *   `usage_rollup_daily.json` por tenant (tokens, requests, p95, bytes)
*   Export para CSV/Parquet (integração contábil)
*   Webhook opcional `POST /billing/usage`

* * *

🔐 N4 — Rotação de Segredos & Onboarding
----------------------------------------

### Onboarding de tenant (CLI)

```bash
vvtvctl tenants create --name "Acme" --plan pro --rpm 120 --burst 30 --scopes '["infer"]'
# retorna: tenant_id, api_public_key, api_secret (mostrar 1x)
```

*   Secret armazenado como **hash** (argon2id) — nunca em claro
*   Envio seguro ao cliente (canal out-of-band)

### Rotação (chaves HMAC)

*   `POST /v1/keys/rotate` (se habilitado e scope permitir)
*   Política: chave nova **ativa em T0**, chave antiga **expira em T0+7d**
*   Gate: rate-limit específico para rotação (1/dia)

### Chaves JWT (se usado)

*   `jwks.json` com `kid` e rotação trimestral
*   Tolerância simultânea de **2 chaves** por 7 dias

* * *

🛡️ N5 — Edge/DMZ & WAF
-----------------------

*   Gateway em **máquina separada** (ou container) com:
    *   WAF básico (OWASP CRS se usar NGINX/ModSecurity)
    *   **mTLS** opcional p/ parceiros enterprise
*   Pool fica **em rede interna**; somente API do gateway chega nele
*   **mTLS interno** opcional entre gateway ↔ pool

**Headers de segurança:**

*   `X-Request-Id` gerado no gateway
*   Limitar tamanhos: `client_max_body_size 1m` (JSON curto)
*   `Content-Security-Policy` irrelevante (API), mas defina `X-Content-Type-Options: nosniff`

* * *

🧪 N6 — SLOs, Carga & Chaos
---------------------------

### Testes de carga (k6, exemplos)

*   50 rps / 2 min / payload pequeno (`expand_queries`)
*   10 rps / 2 min / payload grande (`judge/rerank` 10–12 itens)
*   Ver:
    *   p50/p95, erro 5xx, timeouts
    *   hit de cache no pool

### Chaos (injetar falhas)

*   Simular **falha de provider** (breaker abre) → hedge assume
*   Induzir **latência** de rede no Ollama → p95 ainda ≤ SLO?
*   Falhar **Auth** (assinatura inválida) → 403 rápido (≤5ms de CPU)

### Sinais de “go/no-go”

*   p95 ≤ 1.2s em `/v1/infer` com 10 rps sustentados
*   5xx < 0.5% por 30 min
*   rate-limit dispara < 2% com clientes “bem-educados”

* * *

🔌 Contratos de API
-------------------

### `POST /v1/infer` (body)

```json
{
  "task": "expand_queries | judge | rerank_candidates | enrich_metadata | recovery_plan",
  "strategy": "FASTEST | VOTING | WEIGHTED | JUDGE", 
  "payload": { "...dados específicos da tarefa..." },
  "hints": { "deadline_ms": 1200, "max_tokens": 256 }
}
```

**Resposta padrão**

```json
{
  "request_id": "uuid",
  "tenant_id": "t_abc",
  "winner_model": "phi3:mini",
  "strategy_used": "FASTEST",
  "duration_ms": 412,
  "from_cache": false,
  "content": { "...schema da tarefa..." }
}
```

**Erros**

*   `401/403 AUTH_FAILED`
*   `409 NONCE_REPLAY`
*   `429 RATE_LIMITED` (Retry-After)
*   `422 SCHEMA_INVALID`
*   `502 PROVIDER_FAILED`
*   `504 DEADLINE_EXCEEDED`

* * *

🧰 Exemplos práticos
--------------------

### `curl` HMAC (cliente)

```bash
BODY='{"task":"expand_queries","strategy":"FASTEST","payload":{"seed":"abc"}}'
TS=$(date +%s%3N); NONCE=$(uuidgen); KEY="vvtv-pub-001"; SECRET="…"

SIG=$(printf "POST|/v1/infer|%s|%s|%s" "$BODY" "$TS" "$NONCE" \
  | openssl dgst -sha256 -hmac "$SECRET" -binary | base64)

curl -s http://api.vvtv.local:7071/v1/infer \
  -H "Content-Type: application/json" \
  -H "X-Api-Key: $KEY" \
  -H "X-Timestamp: $TS" \
  -H "X-Nonce: $NONCE" \
  -H "X-Signature: $SIG" \
  -d "$BODY" | jq .
```

### Resposta esperada

```json
{
  "request_id": "f527…",
  "tenant_id": "t_abc",
  "winner_model": "phi3:mini",
  "strategy_used": "FASTEST",
  "duration_ms": 318,
  "from_cache": true,
  "content": {
    "must_include": ["…"],
    "optional": ["…"],
    "negatives": [],
    "duration_range_sec": [540,720],
    "site_hints": ["…"]
  }
}
```

* * *

🔭 Observabilidade & Alertas
----------------------------

### Dashboards

*   **API Health**: `api_http_duration_ms`, `api_requests_total`, `api_rate_limited_total`, `5xx_rate`
*   **Tenant Usage**: `requests`, `tokens_in/out`, `latência p95`, `erro por tenant`

### Alertas (Prometheus)

*   `5xx_rate > 0.005` (5m) → **warning**
*   `p95_infer > 1500ms` (10m) → **warning**
*   `quota_exceeded_total{tenant} > 0` → **info** + webhook
*   `breaker_open_total{provider} > 0` (5m) → **warning** (ver provider)

* * *

📘 Runbooks
-----------

### Onboarding

1.  `vvtvctl tenants create …`
2.  Enviar `api_public_key` + instruções HMAC
3.  Teste `curl` do cliente (sandbox)
4.  Colocar em **pro** (se aplicável), setar quotas

### Rotação de chave

1.  Rodar `rotate_keys.sh` (ou `POST /v1/keys/rotate`)
2.  Informar cliente com **T0/T0+7d**
3.  Monitorar falhas 403

### Incidente de abuso (DoS lógico)

1.  `suspend tenant` (status=`suspended`)
2.  Aumentar `hedge_after_ms` temporário
3.  Ajustar `burst` no gateway
4.  Post-mortem no `audit/` + recomendação

* * *

🧪 Testes e Aceitação
---------------------

### Unit/Integration

*   HMAC/JWT válidos e inválidos
*   Nonce replay
*   Rate-limit por tenant (rps/rpm)
*   Quota mensal (hard/soft)
*   Esquemas por tarefa (422 se inválido)
*   Sanitização estrita de JSON (saída do pool)

### Carga

*   `k6` com 10–50 rps (curto) e 5–10 rps (longo)
*   Ver p95, 5xx, hedge, breaker

### Chaos

*   Matar provider (Ollama) → 502 limitados, breaker abre, hedge assume
*   Atraso artificial → deadline respeitado (504 controlado)

**Critérios de aceitação**

*   p95 ≤ 1.2s estável
*   5xx < 0.5%
*   Rate-limit/quota atuando corretamente
*   Logs/auditoria consistentes e sem PII sensível
*   Dashboards completos + alertas disparam conforme esperado

* * *

🔧 Tarefas por PR
-----------------

**PR N1 — Gateway & Auth**

*   `nginx.conf`/`envoy.yaml`, `start_gateway.sh`, `smoke.sh`
*   Validador HMAC/JWT no **pool-api**
*   Testes de unidade de assinatura

**PR N2 — Tenants & Limits**

*   `tenants.sqlite` schema + migrações
*   Middleware `tenant_resolver` + nonces
*   Token bucket por tenant (rps) + quotas mensais (rpm/tokens)

**PR N3 — Auditoria & Usage**

*   `audit/api_requests.jsonl` (append-only)
*   Export diário `usage_rollup_daily.json` + webhook
*   Dashboards Grafana (API Health, Tenant Usage)

**PR N4 — Rotation & Onboarding**

*   `vvtvctl tenants create/rotate/suspend`
*   Políticas T0/T0+7d
*   Documentação de cliente (HMAC sample)

**PR N5 — Edge/DMZ**

*   WAF básico + limites de corpo
*   mTLS opcional (parceiros)
*   Hardening de SO/Firewall

**PR N6 — SLOs & Chaos**

*   Testes k6 (scripts em `api/tests/k6/`)
*   Fault-injection (provider down, latency)
*   Alertas Prometheus

* * *

✅ Resumo
--------

*   **API pública segura** com HMAC/JWT/mTLS
*   **Multi-tenant** com rate-limit/quotas e auditoria
*   **Métricas & SLOs** prontos para produção
*   **Governança** com rotação de chaves e PRs automáticos (usage/report)
*   **Resiliência**: hedge, breaker, deadlines, caos testado

> **Mantra:** _expor com elegância, limitar com justiça, medir com obsessão._

Se quiser, te entrego os **templates reais** (`nginx.conf`, `envoy.yaml`, `api.toml`, `jwks.json`, `k6 scripts`) já preenchidos para o teu Mac mini e rede local — é só colar e `./bin/start_gateway.sh`.

## Prompt:
Agora o O

## Response:
perfeito — segue o **Epic O** no mesmo formato dos Epics M e N: documento único, executável, para entrar no repo como `docs/epics/Epic-O_Operations_OnCall_SRE.md`.

* * *

🛠️ Epic O — Operations, On-Call & SRE Governance (Blueprint Executável)
========================================================================

> Objetivo: padronizar **operações, plantão e confiabilidade** do VVTV (streaming + business logic + LLM Pool), com **SLOs claros**, **alertas úteis**, **runbooks clicáveis**, **rotina de mudanças segura** e **resposta a incidentes** com pós-mortem.

**Âmbito**

*   **O1 — SLOs, SLIs & Orçamentos de erro**
*   **O2 — Observabilidade unificada (logs, métricas, traços)**
*   **O3 — Alertas que importam e não acordam à toa**
*   **O4 — On-call: escalas, ferramentas, runbooks operacionais**
*   **O5 — Gestão de mudanças (change mgmt) & feature flags**
*   **O6 — Capacidade, disponibilidade, DR & exercícios de caos**

* * *

📦 Entregáveis
--------------

*   **Catálogo de SLOs** por serviço (stream, planner, pool, API pública)
*   **Dashboards Grafana**: “Ops Overview”, “Streaming Health”, “LLM Pool”, “API Health”
*   **Alertas Prometheus** com rótulos por severidade/rota
*   **Runbooks** shell/markdown em `/vvtv/ops/runbooks/…`
*   **Playbooks de incidente** com _checklists_ e _timers_
*   **Ferramentas**: `vvtvctl ops` (status, drains, toggles), _feature flags_ TOML
*   **Calendário de On-Call** (iCal) + Escalonamento (pessoa → coordenação)
*   **Mudanças**: `change_requests/CR-*.md` + _preflight_ automatizado
*   **Pós-mortem**: `postmortems/PM-*.md` com lições e ações rastreáveis

* * *

🗂️ Estrutura de diretórios
---------------------------

```
/vvtv/
├─ ops/
│  ├─ runbooks/
│  │  ├─ rb_streaming.md              # NGINX-RTMP/FFmpeg
│  │  ├─ rb_pool.md                   # LLM Pool
│  │  ├─ rb_api.md                    # Public API
│  │  ├─ rb_planner.md                # Planner/Queue/Curator
│  │  ├─ rb_db.md                     # SQLite (WAL, backup, vacuum)
│  │  ├─ rb_security.md               # chaves, rotação, bloqueios
│  │  └─ rb_discovery.md              # Search/Download/Browser sim
│  ├─ playbooks/
│  │  ├─ pb_incident_major.md
│  │  ├─ pb_cs_intrusion.md
│  │  └─ pb_dr_failover.md
│  ├─ slo/
│  │  ├─ slo_streaming.yaml
│  │  ├─ slo_pool.yaml
│  │  ├─ slo_api.yaml
│  │  └─ slo_planner.yaml
│  ├─ change_requests/
│  │  └─ CR-YYYYMMDD-<slug>.md
│  ├─ postmortems/
│  │  └─ PM-YYYYMMDD-<slug>.md
│  ├─ oncall/
│  │  ├─ rota.ics
│  │  └─ escalation.yaml
│  └─ flags/
│     └─ features.toml                # feature flags
├─ dashboards/
│  ├─ ops_overview.json
│  ├─ streaming_health.json
│  ├─ llm_pool.json
│  └─ api_health.json
└─ alerts/
   └─ prometheus_rules.yaml
```

* * *

✅ O1 — SLOs, SLIs & Orçamento de erro
-------------------------------------

### Tabela de SLOs (p95, janela 7 dias)

| Serviço | SLI | Alvo SLO | Orçamento de erro |
| --- | --- | --- | --- |
| **Streaming** | `segment_age_seconds` | ≤ **6s** (HLS LL) | 0.5% janelas |
|  | `encoder_drops_rate` | < **0.1%** | 0.2% |
| **LLM Pool** | `/infer p95` | ≤ **1200ms** warm | 1% |
|  | `5xx rate` | < **0.5%** | 0.5% |
| **API Pública** | `/v1/infer` p95 | ≤ **1200ms** | 1% |
|  | `5xx rate` | < **0.5%** | 0.5% |
| **Planner** | `selection_cycle_duration_ms p95` | ≤ **500ms** | 1% |
|  | `queue_starvation` (buffer < 30min) | **0** eventos/7d | zero tolerance |

**Definições rápidas**

*   `segment_age_seconds`: idade do último segmento ao vivo.
*   `queue_starvation`: buffer < 30 min por ≥ 2 min.

Arquivos YAML por serviço (ex.: `ops/slo/slo_streaming.yaml`) definem:

*   **Métrica**, **alvo**, **janela**, **owner**, **painel grafana**, **alertas vinculados**.

* * *

📈 O2 — Observabilidade Unificada
---------------------------------

### Logs estruturados (JSONL)

*   `logs/streaming.jsonl` (ingest, ffmpeg, segmenter)
*   `logs/pool.jsonl` (task, strategy, provider, duration, cache/hedge)
*   `logs/api_access.log` (como N mas com `request_id`)
*   `logs/curator_vigilante.jsonl`, `logs/autopilot.jsonl`

**Campo padrão**: `ts`, `service`, `severity`, `msg`, `trace_id`, `span_id`

### Métricas Prometheus (exemplos)

*   `stream_segment_age_seconds`
*   `stream_encoder_drops_total`
*   `llmpool_infer_duration_ms` (histogram)
*   `llmpool_timeouts_total`
*   `api_http_duration_ms{route}`
*   `api_requests_total{code}`
*   `planner_cycle_duration_ms`
*   `queue_buffer_duration_minutes`

### Traços (opcional, OpenTelemetry)

*   `trace_id` passado da **API → Pool → Provider**
*   Amostragem 1–5% para baixa sobrecarga

### Dashboards Grafana

*   **Ops Overview**: visão executiva (SLOs, erros, backlog, buffer)
*   **Streaming Health**: segment\_age, fps, drops, ingest bitrate
*   **LLM Pool**: p95 por task/provider, cache hit, breaker, hedge rate
*   **API Health**: 5xx, p95 por rota, rate-limit/quota

* * *

🔔 O3 — Alertas úteis (sem pager fatigue)
-----------------------------------------

`alerts/prometheus_rules.yaml` (resumo):

*   **CRÍTICO**
    *   `stream_segment_age_seconds > 10` por 2 min
    *   `queue_starvation` detectado
    *   `llmpool_timeouts_total` aumenta > 10/min por 5 min
    *   `api_5xx_rate > 1%` por 5 min
*   **MAJOR**
    *   `p95_infer > 1500ms` por 10 min
    *   `encoder_drops_rate > 0.2%` por 10 min
    *   `buffer_duration_minutes < 45` por 10 min
*   **MINOR**
    *   `curator_apply_rate > 20%` por 1h (config suspeita)
    *   `autopilot_rollbacks_total > 0` (investigar em horário comercial)

**Etiqueta de alerta**: `service`, `severity`, `runbook_url`  
Cada alerta tem `annotations.runbook_url` apontando para um _anchor_ no runbook.

* * *

🧑‍🚒 O4 — On-Call, Escalonamento & Runbooks
--------------------------------------------

### Rota On-Call

*   Arquivo: `ops/oncall/rota.ics` (turnos semanais)
*   Escalonamento: `ops/oncall/escalation.yaml`
    *   Nível 1: Operador (24/7)
    *   Nível 2: SRE Líder (08:00–22:00)
    *   Nível 3: Eng Prod (em horário comercial)
    *   _Timeouts_: 5m → 10m → 20m

### Ferramentas On-Call

*   `vvtvctl ops status` (saúde consolidada)
*   `vvtvctl ops drain` (colocar serviço em manutenção)
*   `vvtvctl flags set <flag>=on|off` (feature flags)
*   `vvtvctl snapshot quick` (snapshot leve de diagnóstico)

### Runbooks (exemplos práticos)

#### `rb_streaming.md` (trecho)

*   Sintoma: **segment\_age > 10s**
*   Ações (em ordem):
    1.  `vvtvctl streaming status`
    2.  Ver ingest bitrate (`ffprobe`), CPU, IO
    3.  **Se encoder travado**: `vvtvctl streaming restart-encoder` (sem derrubar NGINX)
    4.  **Se buffer < 30min**: injetar **emergency loop**
    5.  Capturar `ops bundles` (`vvtvctl ops bundle --scope streaming`)

**Timer**: 10 min para estabilizar **antes** de escalar nível 2.

#### `rb_pool.md`

*   Sintoma: **p95\_infer > 1500ms**
*   Ações:
    1.  `vvtvctl pool health`
    2.  Ver **breaker**/hedge rate
    3.  Desabilitar provider com pior latência: `vvtvctl pool provider disable <id>`
    4.  Reduzir `deadline_ms` de tarefa pesada via **flag** temporária
    5.  Se com API pública: ativar **burst shedding** (limitar `/v1/infer` a rps 60 por tenant)

#### `rb_api.md`

*   Sintoma: **5xx\_rate > 1%**
*   Ações:
    1.  Checar autenticação (replay/nonce)
    2.  Aplicar **ban temporário** ao tenant abusivo
    3.  Ver conns no gateway, _backpressure_ para `/infer`

> Cada runbook tem **comandos copy-paste** e **checklist** (box de ticar).

* * *

🧮 O5 — Mudanças, _Feature Flags_ & Janela de Manutenção
--------------------------------------------------------

### Change Requests (CR)

Modelo: `ops/change_requests/CR-YYYYMMDD-<slug>.md` com:

*   **Descrição**, **riscos**, **roll-back**, **checklist de pré-voo**
*   **Janela** (ex.: Domingo 02:00–03:00 UTC)
*   **Owner** e **Aprovação**

**Preflight automático** (CI ou script):

*   Teste de carga leve (k6 smoke)
*   `vvtvctl validate all`
*   Dry-run de configurações (YAML/TOML)

### Feature Flags

Arquivo: `ops/flags/features.toml`

```toml
[curator]
apply_enabled = true
min_confidence_apply = 0.62

[autopilot]
enabled = false
canary_pct = 0.2
```

CLI:

```bash
vvtvctl flags get curator.apply_enabled
vvtvctl flags set curator.apply_enabled=false
```

### Janelas

*   **Manutenção padrão**: Domingos 02:00–03:00 UTC
*   **Congelamento**: véspera de eventos (ex.: promoções) → **sem mudanças**

* * *

📦 O6 — Capacidade, DR & Caos
-----------------------------

### Planejamento de Capacidade

*   Painel “Capacity Forecast”:
    *   CPU média e p95 por pipeline (encoder, pool, API)
    *   RAM livre, _swap_ zero (meta)
    *   Disco: alerta em 80%, _action_ em 90%
*   Política:
    *   Adicionar instância de encoder se `encoder_drops_rate` toca 0.1% por 3 dias
    *   Aumentar keep-alive do modelo (Ollama) em janelas de pico

### DR/Backups

*   Banco SQLite: **WAL + snapshots diários** (integra com Epic M)
*   Configs & flags: backup versionado (git) + tar.zst diário
*   Teste DR trimestral (revival em nó reserva) — usar playbook do **Epic M**

### Chaos Engineering

*   Cenários mensais:
    *   **Kill provider** (ver hedge/breaker e p95)
    *   **IO throttling** em storage (stream deve seguir com emergency loop)
    *   **Latência artificial** no gateway (p95 API < 1.5s?)

* * *

🧰 Exemplos de comandos operacionais
------------------------------------

```bash
# Saúde consolidada
vvtvctl ops status

# Drenar pool (manutenção sem derrubar API)
vvtvctl ops drain --service pool --reason "upgrade provider weights"

# Desligar provider com latência anômala
vvtvctl pool provider disable ollama-gemma-9b

# Habilitar emergência streaming
vvtvctl streaming emergency --inject 5 --reason "buffer<30m"

# Congelar curador (modo advice-only)
vvtvctl flags set curator.apply_enabled=false
```

* * *

🧪 Testes & Aceitação
---------------------

### Unit/Integration

*   Export de métricas (todos os serviços respondem)
*   Alertas disparam em _fixtures_ de teste (simulações)
*   Runbooks referenciados por todos os alertas (link OK)
*   Flags aplicam efeitos sem restart

### Carga

*   _Soak test_ de 2h (5–10 rps contínuos na API pública)
*   p95 dentro do SLO, 5xx < 0.5%

### Caos

*   Provider down → breaker abre, hedge assume, SLO mantido
*   Encoder reiniciado → segment\_age não ultrapassa 10s por >2min

**Critérios de aceitação**

*   Dashboards com **todas** as métricas chave
*   Alertas sem falsos positivos por 7 dias
*   On-call com playbooks testados em _game day_ trimestral
*   CR de mudança com _preflight_ automatizado

* * *

🧭 Playbook de Incidente (resumo)
---------------------------------

1.  **Reconhecer** alerta (pager)
2.  **Nomear IMOC** (Incident Manager On Call)
3.  **Abrir canal** `#incident-YYYYMMDD-HHMM`
4.  **Status page** interna (severidade + ETA)
5.  **Mitigar** com runbook (10 min alvo para MAJOR)
6.  **Registrar** linha do tempo (timestamps)
7.  **Encerrar** com comunicação e **ação corretiva**
8.  **Pós-mortem** em 48h: causas, ações, owners, prazos

* * *

🔧 Tarefas por PR
-----------------

**PR O1 — SLOs & Painéis**

*   Definir SLO YAML por serviço
*   Criar dashboards Grafana (Ops Overview, etc.)
*   Expor métricas que faltavam

**PR O2 — Alertas & Runbooks**

*   Escrever `alerts/prometheus_rules.yaml`
*   Criar runbooks em `ops/runbooks/*` com _anchors_ (links de alerta)
*   Validar semântica + _dry-run_ de regras

**PR O3 — On-Call & Escalonamento**

*   `oncall/rota.ics` e `escalation.yaml`
*   Template de _status update_ e _war room_

**PR O4 — vvtvctl ops & Flags**

*   Comandos `ops status|drain|bundle`
*   _Feature flags_ TOML + CLI `flags`

**PR O5 — Mudanças & Preflight**

*   Template CR e _preflight_ (validate + k6 smoke)
*   Make targets `make preflight`, `make deploy-change CR=…`

**PR O6 — DR & Caos**

*   Scripts de caos (latência, kill provider/encoder)
*   Procedimentos de revival (integra Epic M)
*   Game-day trimestral documentado

* * *

✅ Resumo
--------

*   **SLOs claros** + **alertas acionáveis** + **runbooks executáveis**
*   **On-call leve** (menos barulho, mais contexto, mais automação)
*   **Mudanças seguras** com _feature flags_ e _preflight_
*   **Resiliência real** com DR testado e exercícios de caos
*   **Operação auditável**: o que fizemos, quando e por quê — sempre documentado

> _“Confiabilidade não é ausência de falhas; é capacidade de agir rápido com boa informação.”_