# VVTV Risk Register (Operacional)

Esta visão consolida a matriz R1–R15 descrita no dossiê industrial (Apêndice A). A fonte canônica de dados computáveis é `configs/risk_register.json`.

## Matriz de Riscos (Resumo)

| ID | Risco | Probabilidade | Impacto | Dono | SLA |
| --- | --- | --- | --- | --- | --- |
| R1 | Violação de DRM/EME ao simular play | Alta | Crítico | Eng. Automação / Jurídico | 1 h |
| R2 | Uso indevido de imagem / sem consentimento | Média | Crítico | Curador / Jurídico | 4 h |
| R3 | CSAM (material ilegal) | Baixa | Catastrófico | Compliance | Imediato |
| R4 | Violação GDPR / coleta excessiva | Média | Alto | DPO / Eng. Dados | 24 h |
| R5 | Fila de streaming vazia | Alta | Alto | Eng. Operações | 15 min |
| R6 | Downloads corrompidos (tokens expirados) | Média | Médio | Eng. Curadoria | 2 h |
| R7 | Explosão de inodes / IO HLS | Alta | Médio | Infra / Storage | 6 h |
| R8 | Exploit em ffmpeg / navegador | Média | Crítico | Eng. Segurança | 2 h |
| R9 | Banimento de CDN / host | Média | Crítico | Ops / Legal | 30 min |
| R10 | Monetização congelada | Média | Alto | Financeiro / Legal | 24 h |
| R11 | Latência alta (>9s) | Média | Médio | Eng. Vídeo | 4 h |
| R12 | Fingerprint bloqueado / anti-bot | Alta | Médio | Eng. Automação | 2 h |
| R13 | Falha em logs | Média | Médio | Eng. Observabilidade | 1 h |
| R14 | Falha elétrica / sobrecarga térmica | Baixa | Alto | Eng. Infraestrutura | 10 min |
| R15 | Incidente jurídico / bloqueio CNPD | Baixa | Crítico | Jurídico / DPO | 12 h |

## Gatilhos Computáveis

Os gatilhos a seguir são avaliados pelo script `risk_review.sh`:

- **R5 (Buffer <3 h)** → Executa `check_queue.sh`; se <2 h, rodar `inject_emergency_loop.sh`.
- **R11 (Latência média >9 s)** → Consultar `metrics.sqlite` e reconfigurar pipeline/CDN.
- **R14 (Temperatura >75 ºC)** → Rodar `check_thermal.sh`, verificar UPS e ventilação.

## Agenda de Revisão

| Frequência | Ação | Responsável | Entregável |
| --- | --- | --- | --- |
| Mensal | Auditoria legal / consentimento | Jurídico | `VVTV_Compliance_Audit.md` |
| Semanal | Teste de buffer + loop emergência | Eng. Vídeo | `buffer_test.log` |
| Diário | Sandbox integrity check | Eng. Segurança | `security_check_report.json` |
| Contínuo | Monitoramento UPS e temperatura | Infraestrutura | Alertas Telegram/Email |
| Quinzenal | Revisão de monetização | Financeiro | `ledger_reconciliation.csv` |

## Fluxo Automatizado

1. Configure um cron local diário para executar `risk_review.sh --json` e arquivar o resultado em `/vvtv/system/reports/risk_snapshot_<data>.json`.
2. Em caso de alertas, registre entrada manual no `incident_log.md` com ações corretivas.
3. Atualize `configs/risk_register.json` sempre que novos riscos, owners ou SLAs forem ajustados.

> Referência primária: [VVTV INDUSTRIAL DOSSIER — Apêndice A](../VVTV%20INDUSTRIAL%20DOSSIER.md).
