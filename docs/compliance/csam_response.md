# CSAM Incident Response

Fluxo completo para lidar com detecções de CSAM (Child Sexual Abuse Material).
**Tolerância zero**: siga à risca e envolva autoridades competentes.

## Detecção

1. Execução automatizada via `scripts/system/compliance_scan.sh` ou
   `vvtvctl compliance csam --media-dir /vvtv/storage/ready`.
2. Conferir relatório JSON e identificar arquivos sinalizados.

## Contenção

1. Isolar mídias em `/vvtv/storage/quarantine/<timestamp>`.
2. Invalidar PLANs associados via `./vvtvctl plan blacklist --add`.
3. Remover qualquer referência da fila `queue.sqlite`.

## Notificação

1. Acionar imediatamente o responsável legal listado em `VVTV INDUSTRIAL
   DOSSIER.md` (Apêndice J).
2. Preparar dossiê com hashes (`sha256sum arquivo`) e evidências de logs.
3. Notificar autoridade competente conforme jurisdição (ex.: Polícia Federal
   no Brasil, hotline INHOPE na UE).

## Registro

- Criar `vault/compliance/reports/csam_<timestamp>.md` contendo: identificação,
  hash, origem, timestamp, ações tomadas.
- Assinar com `logline sign` e armazenar somente no vault.

## Recuperação

1. Executar auditoria completa:
   ```bash
   ./vvtvctl compliance audit --format json
   ./vvtvctl compliance drm --format json
   ```
2. Revalidar queue e buffer (`./vvtvctl queue summary`).
3. Atualizar `docs/compliance/compliance_policies.md` com medidas preventivas.

## Referências

- `VVTV INDUSTRIAL DOSSIER.md`, Bloco V, seção "CSAM" (linhas 3210–3230)
- `docs/compliance/license_audit.md`
