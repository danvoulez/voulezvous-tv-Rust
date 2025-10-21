# VVTV — Manutenção Física e Ciclos Preventivos

> Atualizado em 2025-10-21

## Visão Geral

O objetivo deste guia é garantir que o nó físico da VoulezVous.TV mantenha integridade operacional por anos. Todos os procedimentos abaixo são executáveis offline e priorizam transparência auditável.

## Cadência de Atividades

### Diário

- Verificar painel `monitor/dashboard.html` e checar alertas de temperatura, potência e desgaste SSD.
- Confirmar que `scripts/system/selfcheck.sh` executou com sucesso (sem `checks_failed`).
- Observar fisicamente o Mac Mini/servidor: ruídos anômalos, luzes estáveis, cabos firmes.
- Validar que o UPS está em linha e sem alarmes (LED verde contínuo).

### Semanal

- Executar `scripts/system/check_power.sh` e `scripts/system/check_thermal.sh`; registrar valores no log de operação.
- Rodar `smartctl -H <disco>` para validar saúde SMART.
- Forçar `scripts/system/backup_hot.sh` e verificar logs em `/vvtv/system/logs/backup_hot.log`.
- Revisar `tailscale status` para confirmar sessões ativas e remover peers obsoletos.

### Mensal

- Aplicar `scripts/system/security_rotate_keys.sh` e registrar assinatura no vault.
- Executar `scripts/system/security_audit.sh`; revisar relatório `security/audit_<data>.txt`.
- Limpar fisicamente ventiladores com ar comprimido leve e verificar filtros.
- Registrar desgaste SSD (`Percent_Lifetime_Used`) e atualizar planilha de manutenção.
- Testar `scripts/system/test_restore.sh <snapshot>` para validar processo de recuperação.

### Trimestral

- Reaplicar pasta térmica (se necessário) ou agendar visita técnica.
- Executar `scripts/system/sandbox_enforce.sh` para reafirmar imutabilidade e limites de cgroup.
- Rodar `scripts/system/apply_firewall.sh` e auditar logs de acesso indevido.
- Rotacionar chaves Tailscale (remover chaves antigas e reaplicar `tailscale up`).
- Inspecionar UPS: teste de autonomia rápida (desligar energia por 1 minuto) e registrar minutos de runtime.

### Anual

- Verificar inventário de peças sobressalentes (ventoinhas, SSD spare, cabos).
- Revisar documentos legais e contrato de custódia (Apêndice O do dossiê).
- Testar ritual completo `standby.sh` → `resume.sh` em ambiente secundário.

## Checklist de Registro

Para cada atividade realizada, registrar em `/vvtv/system/logs/maintenance.log` com o formato:

```
[2025-01-15T12:00:00Z] [maintenance] atividade=<descrição> status=<ok|warn|fail> notas="observações"
```

Este log é ingerido pelo `MetricsStore` e mantido no vault como evidência auditável.

