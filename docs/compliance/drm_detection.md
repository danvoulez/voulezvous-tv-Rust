# DRM / EME Detection Runbook

Procedimento para investigar e responder a marcas de DRM ou EME encontradas
nas playlists/manifests.

## Execução

```bash
./vvtvctl compliance drm --input /vvtv/broadcast/hls --format text
```

O relatório apresenta o arquivo analisado, o padrão encontrado e um snippet
contextual.

## Ações Imediatas

1. Remover PLAN/asset associado e mover mídia para `/vvtv/storage/quarantine`.
2. Atualizar `plan_blacklist` com o domínio de origem, se necessário.
3. Registrar incidente em `docs/compliance/compliance_policies.md`.
4. Se recorrente, ajustar `browser.toml` (seções `[fingerprint]` e `[proxy]`).

## Evidências

- Salvar o relatório JSON (`--format json`) em
  `/vvtv/system/logs/drm_scan_<timestamp>.json`.
- Gerar captura da página via `./vvtvctl qa smoke-test` com `--mode headed`.

## Referências

- `VVTV INDUSTRIAL DOSSIER.md`, Bloco V (linhas 1360–1480)
- `docs/compliance/compliance_policies.md`
