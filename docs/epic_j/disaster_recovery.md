# VVTV — Disaster Recovery Runbook

## Objetivo

Garantir que o canal possa ser hibernado e restaurado com integridade total, mesmo em hardware alternativo.

## Componentes

- `scripts/system/standby.sh`: ritual computável de desligamento.
- `scripts/system/resume.sh`: rotina de volta ao ar.
- `scripts/system/test_restore.sh`: validação de snapshots.
- `vault/snapshots/standby_<timestamp>.tar.zst`: artefatos principais.
- `vault/final_frame.jpg`: frame final assinado.

## Fluxo de Standby

1. Executar `sudo scripts/system/standby.sh`.
2. Verificar log em `/vvtv/system/logs/standby.log`.
3. Confirmar criação do snapshot (`standby_<timestamp>.tar.zst`).
4. Armazenar o frame final e assinatura no vault.

## Fluxo de Resgate

1. Dispor snapshot em `/vvtv/vault/snapshots` (via USB ou rede segura).
2. Executar `sudo scripts/system/resume.sh`.
3. Validar que serviços principais subiram (`systemctl status vvtv-*`).
4. Rodar `scripts/system/selfcheck.sh` e `scripts/system/check_power.sh`.
5. Notificar operadores sobre retomada.

## Testes Periódicos

- Mensal: `scripts/system/test_restore.sh /vvtv/vault/snapshots/standby_latest.tar.zst`.
- Trimestral: restaurar snapshot em hardware alternativo (VM) e validar stream interno.
- Registrar evidências no log de manutenção.

