# Edge Nodes & Latência Planetária

## Ferramentas

- **CLI:** `scripts/system/init_edge_node.sh`
- **Config:** `[distribution.edge]`
- **Código:** `EdgeOrchestrator` (`vvtv-core/src/distribution/edge.rs`)
- **Métricas:** tabela `edge_latency`

## Provisionamento

```bash
scripts/system/init_edge_node.sh --origin https://voulezvous.tv/live.m3u8 --region eu-west
```

O script gera log em `/vvtv/system/logs/edge_nodes.log` e invoca `logline --init-node --role=edge` quando disponível.

## Monitoramento

`DistributionManager::execute_cycle()` chama `EdgeOrchestrator::probe_latency`, registrando latências por alvo em `metrics.sqlite`.

- JSON em `/vvtv/system/logs/edge_latency.jsonl`
- Heatmap agregado em `/vvtv/system/metrics/latency_heatmap.json`

`EdgeOrchestrator::evaluate_buffer()` permite saber se o nó precisa recarregar cache quando o buffer for < 15 s ou o último segmento estiver obsoleto.
