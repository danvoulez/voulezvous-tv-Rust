# Operação de CDN Primária e Secundária

## Cloudflare (Primária)

- Configuração em `[distribution.cdn_primary]`.
- Worker script: `configs/cdn/cloudflare_worker.js` (reescreve `.m3u8` para origin Tailscale).
- Ferramenta CLI: `scripts/system/switch_cdn.sh <hostname>`.
- Métricas armazenadas na tabela `cdn_metrics` (`MetricsStore::record_cdn_metrics`).

### Checklist

1. Definir variáveis de ambiente `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ZONE_ID`, `CLOUDFLARE_RECORD_ID` no ambiente local.
2. Atualizar `vvtv/system/workers/cloudflare_rewrite.js` (deploy via `PrimaryCdnManager::deploy_worker`).
3. Executar `switch_cdn.sh` para failover manual:
   ```bash
   CLOUDFLARE_API_TOKEN=... scripts/system/switch_cdn.sh railway.voulezvous.ts.net --reason teste
   ```
4. Validar log em `/vvtv/system/logs/cdn_failover.log` e métricas em `metrics.sqlite`.

## Backblaze/Bunny (Secundária)

- Configuração em `[distribution.cdn_backup]`.
- `BackupCdnManager` usa `rclone copy` com `--immutable` e limpa segmentos via manifest.
- Logs JSON em `/vvtv/system/logs/cdn_backup.log`.
- Métricas armazenadas em `backup_syncs` (timestamp, arquivos enviados, segmentos removidos).

### Fluxo diário

1. `origin_replication.sh` garante `/vvtv/broadcast/hls` atualizado.
2. `BackupCdnManager::sync_backup()` envia segmentos para `b2:vv_hls_backup`.
3. Manifest (`/vvtv/broadcast/hls/manifest.json`) determina quais segmentos podem ser apagados.
4. `scripts/system/switch_cdn.sh` permite promover CDN secundária em segundos.

## Monitoramento

- `cdn_metrics` + `backup_syncs` alimentam dashboards.
- `DistributionManager::execute_cycle()` já grava métricas agregadas por ciclo de replicação.
