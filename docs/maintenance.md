# Maintenance & Health Checklist

Guia consolidado de manutenção preventiva e corretiva do VoulezVous.TV. Baseado
nos Apêndices G e H do `VVTV INDUSTRIAL DOSSIER.md`.

## Tarefas Diárias

- [ ] Executar `./vvtvctl status` e anexar output ao log diário.
- [ ] Rodar `./scripts/system/check_stream_health.sh --external`.
- [ ] Revisar `metrics.sqlite` via `./vvtvctl qa report --output /vvtv/monitor/dashboard.html`.
- [ ] Validar temperatura ambiente (`docs/operations/manual_do_operador.md`).
- [ ] Rodar `./scripts/system/compliance_scan.sh` e anexar relatório.

## Tarefas Semanais

- [ ] `./scripts/system/optimize_databases.sh /vvtv/data`.
- [ ] `./scripts/system/risk_review.sh --mode weekly`.
- [ ] Revisar `docs/compliance/license_audit.md` e atualizar findings.
- [ ] Atualizar snapshots quentes: `./scripts/system/backup_hot.sh`.

## Tarefas Mensais

- [ ] `./scripts/system/backup_warm.sh` + verificação `logline verify`.
- [ ] Rodar `./scripts/system/security_audit.sh` e anexar relatório.
- [ ] Revisar estado do hardware: `./scripts/system/check_power.sh`,
  `./scripts/system/check_thermal.sh`, inspeção física.
- [ ] Validar plano de hibernação (`docs/operations/runbooks.md#modo-hibernacao`).

## Indicadores de Saúde

| Indicador                | Threshold | Ação                                                         |
|-------------------------|-----------|--------------------------------------------------------------|
| Buffer operacional       | < 3 h     | Executar `./scripts/system/fill_buffer.sh`                   |
| VMAF ao vivo             | < 85      | Rerodar processor, verificar transcode                       |
| Temperatura CPU         | > 75 °C   | Ajustar airflow, reduzir carga, chamar manutenção física     |
| Falhas CSAM/DRM         | > 0       | Acionar fluxo em `docs/compliance/csam_response.md`          |
| Falhas de watchdog      | > 2/dia   | Revisar `vvtv-watchdog` e logs de encoder                    |

## Automação Sugerida

Adicionar ao `crontab` do usuário `vvtv`:

```
0 3 * * * /vvtv/system/bin/compliance_scan.sh
30 3 * * 1 /vvtv/system/bin/risk_review.sh --mode weekly
0 4 1 * * /vvtv/system/bin/backup_warm.sh
```

> As versões em `/vvtv/system/bin` devem apontar para os scripts desta árvore.

## Relatórios

- Armazene outputs JSON em `/vvtv/system/logs` e assine com `logline sign`.
- Atualize o dashboard físico com indicadores chave ao final do turno.

## Referências

- `docs/operations/manual_do_operador.md`
- `docs/compliance/compliance_policies.md`
- `VVTV INDUSTRIAL DOSSIER.md`, Apêndice H (linhas 5080–5400)
