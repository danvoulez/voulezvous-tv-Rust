# VVTV Incident Playbook — Scripts de Resposta Rápida

Os scripts abaixo vivem em `/vvtv/system/bin/` (espelhados em `scripts/system/`) e registram atividades no arquivo estruturado `incident_log.md`.
Use-os em conjunto com o [Apêndice B do Dossiê Industrial](../VVTV%20INDUSTRIAL%20DOSSIER.md) e com o registro de incidentes no vault.

## Monitoramento e Diagnóstico

| Script | Uso | Objetivo |
| --- | --- | --- |
| `check_queue.sh` | `check_queue.sh --recent 10 --json` | Sumário da fila de playout e cálculo de buffer (com códigos de saída 0/1/2 para saudável/alerta/crítico). |
| `browser_diagnose.sh` | `browser_diagnose.sh --profile profile-1 --json` | Valida estado dos perfis Chromium (processos, diretórios, proxies, logs de curadoria). |
| `integrity_check.sh` | `integrity_check.sh --report /vvtv/system/reports/integrity.json` | Executa `PRAGMA integrity_check`, checa serviços essenciais e gera relatório JSON assinado no log de integridade. |

## Ação Imediata

| Script | Uso | Objetivo |
| --- | --- | --- |
| `inject_emergency_loop.sh` | `inject_emergency_loop.sh --count 5` | Injeta conteúdo seguro do arquivo para restaurar buffer (suporta `--dry-run` para validação). |
| `restart_encoder.sh` | `restart_encoder.sh` | Reinicia o encoder FFmpeg com parada graciosa e fallback manual; aceita `--dry-run`. |
| `switch_cdn.sh` | `switch_cdn.sh cdn.backup.example --reason manutencao` | Aciona failover de CDN via Cloudflare e registra o evento. |

## Conformidade e Remediação

| Script | Uso | Objetivo |
| --- | --- | --- |
| `takedown.sh` | `takedown.sh --id PLAN_ID --reason "Solicitação legal"` | Remove um asset (tabela `plans` e `playout_queue`) e move artefatos para quarentena, preservando trilha de auditoria. |

## Comunicação e Escalonamento

- Utilize `vvtvctl incident report` para gerar o postmortem computável, arquivar no vault e disparar alertas por gravidade.
- Severidade → canais automáticos:

| Gravidade | Canais |
| --- | --- |
| Crítico | Telegram + Email + registro local |
| Alto | Telegram + registro local |
| Médio | Registro local (incluso no histórico) |
| Baixo | Registro local |

Exemplo:

```bash
vvtvctl incident report \
  --id INC-2025-04 \
  --title "Buffer abaixo de 1h" \
  --severity critical \
  --summary "Buffer caiu para 0, stream offline" \
  --impact "24 min offline" \
  --root-cause "CDN principal indisponível" \
  --action "Ativar emergency loop" \
  --prevention "Configurar alerta antecipado" \
  --timeline 2025-04-12T21:11:00Z=Alarme emitido \
  --timeline 2025-04-12T21:15:00Z=Emergency loop ativo
```

O comando gera Markdown/JSON em `vault/incident_history/` e envia notificações conforme a matriz acima (use `--dry-run` em ambientes de teste).

### Convenções Gerais

- Todos os scripts aceitam `--dry-run` quando apropriado, evitando efeitos colaterais.
- Logs estruturados são persistidos automaticamente em `/vvtv/system/logs/incident_log.md`.
- O script `test_incident_playbook.sh` pode ser executado em ambiente de desenvolvimento para validar dependências e executar fumaceamento local:

```bash
./scripts/system/test_incident_playbook.sh
```

> Sempre atualizar o `incident_log.md` após rodar qualquer runbook manual, incluindo contexto adicional (IDs, URLs, assinaturas).
