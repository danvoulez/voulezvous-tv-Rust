# Replicação Origin → Railway e Failover Automático

## Visão geral

- **Ferramenta:** `scripts/system/origin_replication.sh`
- **Configuração:** `[distribution.replication]` em `configs/vvtv.toml`
- **Persistência:** tabelas `replication_syncs` e `replication_events` em `metrics.sqlite`
- **Failover:** `scripts/system/promote_failover.sh`

## Execução manual

```bash
# Sincroniza e checa drift
scripts/system/origin_replication.sh

# Apenas checagem (sem sync)
scripts/system/origin_replication.sh --check-only
```

Saídas relevantes:

- Log JSON em `/vvtv/system/logs/origin_replication.log`
- Drift acumulado também exposto via `vvtv-core::MetricsStore::record_replication_report`

## Fluxo de failover

1. `origin_replication.sh` calcula `drift_percent`.
2. Quando `drift_percent > distribution.replication.check_threshold_percent`, executa `scripts/system/promote_failover.sh`.
3. O script registra auditoria em `/vvtv/system/logs/failover.log` e tenta iniciar `systemctl start vvtv-failover.target`.
4. Métricas ficam disponíveis para dashboard via `replication_events`.

## Criação de cron job

```
*/15 * * * * /vvtv/system/bin/origin_replication.sh >> /vvtv/system/logs/origin_replication.cron.log 2>&1
```

Adapte o caminho de acordo com o ambiente físico. O script é idempotente e aceita `BWLIMIT_Mbps` para limitar banda.
