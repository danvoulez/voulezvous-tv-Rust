# VVTV — Procedimentos Legais e Institucionais

## Objetivo

Padronizar a transferência de custódia do VoulezVous.TV Industrial, preservando licenças computáveis, a cadeia de assinaturas LogLine e o legado cultural do acervo.

## Licenciamento Computável

- Cada snapshot gerado por `scripts/system/standby.sh` inclui a licença **LogLine Heritage**, anexada em `vault/snapshots/<timestamp>/LICENSE.lll`.
- A licença concede direito de reativação a detentores autorizados e, em caso de dissolução da VoulezVous Foundation, prevê migração automática para **LogLine Open Heritage**.
- Hashes públicos dos snapshots devem ser publicados pela fundação para validação independente.
- Toda reativação (`logline revive`) registra uma nova genealogia no ledger (`vault/ledger/genealogy.log`):
  ```
  generation: <n>
  ancestor_signature: sha256:<hash_do_snapshot_anterior>
  ```

## Transferência de Custódia

### Cenários e Ações

| Situação | Ação Imediata |
| --- | --- |
| Afastamento do operador primário | Entregar snapshot atual + chave `voulezvous_custodian.pem` para `custodian.lll` (LogLine Foundation). |
| Venda ou mudança de controle | Executar `logline resign --key voulezvous_foundation.pem` e registrar novo mantenedor no manifesto. |
| Migração para hardware alternativo | Restaurar snapshot via `logline revive` e validar automações (`selfcheck.sh`, watchdogs, dashboards). |

### Passo a Passo — Transferir Custódia

1. Garantir standby recente (`scripts/system/standby.sh`).
2. Copiar artefatos para mídia segura (`standby_<timestamp>.tar.zst`, `final_frame.jpg`, `Last Transmission Manifest`).
3. Assinar termo físico ou digital apontando novo custodiante.
4. Entregar chaves (`voulezvous_custodian.pem`, `voulezvous_foundation.pem` quando aplicável) via canal seguro.
5. Registrar transferência em `vault/logs/institutional_transfers.log`:
   ```
   date = 2025-10-13T23:59:59Z
   from = "Dan Amarilho"
   to = "LogLine Foundation / custodian.lll"
   snapshot = "standby_20251013.tar.zst"
   signature = "sha256:..."
   ```
6. Confirmar recebimento por contra-assinatura (`logline sign --ack institutional_transfer.md`).

## Procedimentos Legais Complementares

- **Documentação Notarial:** anexar cópias digitalizadas de termos de cessão em `vault/legal/<ano>/<evento>.pdf`.
- **Registro de Licenças de Conteúdo:** manter comprovantes de direitos autorais em `vault/legal/licenses/` e vincular `license_id` nos manifests dos vídeos.
- **Compliance Pós-Transferência:** novo custodiante deve executar `scripts/system/selfcheck.sh` + `scripts/system/test_restore.sh` em até 24h.
- **Transparência Pública:** publicar comunicado resumido (Markdown) em `docs/epic_j/notices/` quando houver mudança institucional significativa.

## Checklist Rápido

- [ ] Snapshot mais recente assinado e verificado (`logline verify`).
- [ ] Licença LogLine Heritage anexada ao pacote entregue.
- [ ] Chaves criptográficas transferidas com recibo.
- [ ] Ledger genealógico atualizado com nova geração.
- [ ] Termos legais arquivados e comunicados emitidos.
- [ ] Novo custodiante validou restauração e integridade.
