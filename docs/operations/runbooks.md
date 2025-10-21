# Runbooks Operacionais

Runbooks condensados para falhas específicas. Consulte sempre os scripts
relacionados e o `VVTV INDUSTRIAL DOSSIER.md` correspondente.

## Curator — Recuperação do Browser

1. `./scripts/system/browser_diagnose.sh --verbose`
2. Reiniciar perfil afetado:
   ```bash
   ./vvtvctl plan blacklist --add https://dominio-problematico.com
   pkill -9 chromium
   ```
3. `./scripts/system/fill_buffer.sh --min-hours 4`
4. Validar com `./vvtvctl qa smoke-test --url <player>`

## Broadcaster — Encoder Travado

1. `./scripts/system/restart_encoder.sh`
2. Se falhar, executar `./scripts/system/inject_emergency_loop.sh`
3. Validar com `./scripts/system/check_stream_health.sh --external`
4. Registrar incidente em `incident_log.md`

## Falha de Energia

1. Acione UPS manualmente e valide tempo restante.
2. Se necessário, execute `./scripts/system/standby.sh`.
3. Após retorno, `./scripts/system/resume.sh` + `./scripts/system/selfcheck.sh`
4. Verifique integridade `logline verify /vvtv/vault/snapshots/*.manifest`

## Corrupção de Banco SQLite

1. `./scripts/system/run_sqlite_integrity.sh /vvtv/data`
2. Caso falhe, restaurar último snapshot: `./scripts/system/backup_cold.sh --restore`
3. Reaplicar `./scripts/optimize_databases.sh`
4. Registrar novo hash no vault.

## Compliance — Incidente CSAM

1. `./vvtvctl compliance csam --media-dir /vvtv/storage/ready`
2. Isolar arquivos listados em `/vvtv/storage/quarantine`
3. Notificar autoridades conforme `docs/compliance/csam_response.md`
4. Documentar no `vault/compliance/compliance_journal.md`

## Compliance — DRM/EME

1. `./vvtvctl compliance drm --input /vvtv/broadcast/hls`
2. Conferir snippet do relatório e remover asset impactado
3. Atualizar `docs/compliance/drm_detection.md`
4. Revisar heurísticas de `browser.toml` se necessário

## Pós-Incidente

- Rodar `./scripts/system/compliance_scan.sh`
- Atualizar `docs/maintenance.md` com lições aprendidas
- Planejar retro com equipe (ver `Tasklist.md`, seção Epic K)
