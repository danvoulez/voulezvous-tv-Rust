# Epic H — Distribuição, CDN e Redundância Global

Este diretório documenta a implementação do Epic H: replicação origin→Railway, automação de failover, integrações CDN primária e secundária, gerenciamento de nós edge e segurança de distribuição.

## Componentes

| PR | Descrição | Artefatos-chave |
| --- | --- | --- |
| H1 | Replicação automática para Railway com auditoria de drift | `scripts/system/origin_replication.sh`, `configs/vvtv.toml` (`distribution.replication`), tabela `replication_*` em `metrics.sqlite` |
| H2 | Integração Cloudflare (CDN primária) com failover via API e métricas | `scripts/system/switch_cdn.sh`, `configs/cdn/cloudflare_worker.js`, `distribution.cdn_primary` |
| H3 | CDN secundária (Backblaze/Bunny) com limpeza por manifest | `distribution.cdn_backup`, `BackupCdnManager`, log `/vvtv/system/logs/cdn_backup.log` |
| H4 | Provisionamento de edge nodes e sondas de latência | `scripts/system/init_edge_node.sh`, `EdgeOrchestrator`, heatmap `edge_latency.jsonl` |
| H5 | Segurança de distribuição (tokens HLS, TLS, firewall) | `DistributionSecurity`, `distribution.security`, log `/vvtv/system/logs/distribution_access.log` |

Cada seção abaixo detalha os playbooks operacionais.

- [Replicação e failover](replicacao_failover.md)
- [CDN primária e secundária](cdn_operacao.md)
- [Edge nodes e telemetria planetária](edge_nodes.md)
- [Segurança de distribuição](seguranca.md)
