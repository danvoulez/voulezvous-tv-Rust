# Compliance Policies & Checklist

Documento mestre para políticas de conformidade do VoulezVous.TV. Referências
principais: Bloco V, Apêndice E e Apêndice J do `VVTV INDUSTRIAL DOSSIER.md`.

## Política Geral

1. **Consentimento explícito**: todo conteúdo precisa de prova verificável
   (contrato, e-mail, licença CC-BY-SA) anexada ao PLAN.
2. **Play-Before-Download**: nenhum asset é baixado sem playback validado em
   tempo real.
3. **Zero tolerância CSAM/DRM**: detecção automática aborta fluxo e gera alerta
   humano em tempo real.
4. **Retenção mínima**: logs e métricas em SQLite seguem `limits.logs_retention_days`.

## Checklist de Auditoria

| Item                               | Frequência | Procedimento                                                         |
|------------------------------------|------------|----------------------------------------------------------------------|
| License logs atualizados           | Semanal    | `vvtvctl compliance audit --format json` + anexar ao vault           |
| DRM markers ausentes               | Diário     | `vvtvctl compliance drm --format text` sobre `/vvtv/broadcast/hls`   |
| Hash CSAM atualizado               | Mensal     | Atualizar `vault/compliance/csam/hashes.csv` e registrar origem      |
| Incidentes de takedown documentados| Sob demanda| `docs/compliance/license_audit.md` + `incident_log.md`               |
| Revisão GDPR/DPoA                  | Trimestral | Validar storage mínimo, anonimização e consentimento de audiência    |

## Fluxos de Escalonamento

- **DRM detectado** → Abortar plan, registrar em `docs/compliance/drm_detection.md`,
  acionar responsável jurídico.
- **CSAM detectado** → Isolar ativo (`scripts/system/takedown.sh`), notificar
  autoridades conforme legislação local e registrar `csam_report_<timestamp>.md`.
- **Consentimento expirado** → Executar `vvtvctl compliance audit` com
  `--expiry-grace-days 3`, suspender PLANs afetados até renovação.

## Documentos Auxiliares

- [`docs/compliance/license_audit.md`](./compliance/license_audit.md)
- [`docs/compliance/drm_detection.md`](./compliance/drm_detection.md)
- [`docs/compliance/csam_response.md`](./compliance/csam_response.md)
- [`docs/operations/manual_do_operador.md`](./operations/manual_do_operador.md)

## Integrações Automatizadas

- `scripts/system/compliance_scan.sh` gera relatórios JSON e texto assinados.
- Workflow local `.github/workflows/ci.yml` executa `cargo fmt`, `cargo clippy`,
  `cargo test` e `cargo tarpaulin` garantindo que regressões sejam detectadas
  antes do merge.
- Logs de consentimento são armazenados em `/vvtv/vault/compliance/license_logs`
  e versionados via `logline`.

## Registro de Conformidade

Mantenha o arquivo `vault/compliance/compliance_journal.md` com entradas no
formato:

```
### <AAAA-MM-DD> — Auditoria Semanal
- Auditor: <nome>
- Ferramentas executadas: compliance_scan.sh, risk_review.sh
- Findings: <resumo + links>
- Correções aplicadas: <detalhes>
- Assinatura: logline sign compliance_journal.md
```

## Referências Regulativas

- GDPR (Art. 6, 17) — Direitos do titular, retenção mínima.
- Diretiva AVMS — Conteúdo audiovisual em território europeu.
- Marco Civil da Internet (Brasil) — Logs e guarda de dados.
- Estatuto da Criança e do Adolescente (Brasil) — Responsabilidade penal CSAM.
