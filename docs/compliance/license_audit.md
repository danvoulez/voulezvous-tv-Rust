# License Audit Protocol

Guia de auditoria dos registros de consentimento e licenciamento. Usa como base
os JSONL armazenados em `/vvtv/vault/compliance/license_logs/`.

## Formato do Registro

```json
{
  "plan_id": "plan-abc123",
  "license_proof": "https://drive.link/prova.pdf",
  "consent_source": "Email produtor 2025-01-04",
  "verified_at": "2025-01-10T18:22:01Z",
  "expires_at": "2026-01-10T18:22:01Z",
  "jurisdiction": "EU",
  "notes": "Contrato CC-BY-SA"
}
```

## Procedimento Semanal

1. `./vvtvctl compliance audit --logs-dir /vvtv/vault/compliance/license_logs --format json > reports/license_audit_<data>.json`
2. Revisar findings `MissingProof`, `ExpiredConsent` e `VerificationStale`.
3. Renovar ou remover PLANs impactados via `./vvtvctl plan blacklist --add`.
4. Atualizar esta página com resumo e ações.
5. Assinar o relatório JSON com `logline sign`.

## Estrutura Recomendada

```
## <AAAA-MM-DD> — Auditoria Semanal
- Entradas analisadas: <n>
- Findings por tipo:
  - MissingProof: <n>
  - ExpiredConsent: <n>
  - VerificationStale: <n>
- Ações tomadas:
  - PLAN-123: renovado (link)
  - PLAN-456: removido
- Observações adicionais
```

## Automatização

Agende o script `scripts/system/compliance_scan.sh` e aponte-o para os diretórios
de log corretos. Os relatórios JSON serão criados automaticamente em
`/vvtv/system/logs`.

## Referências

- `VVTV INDUSTRIAL DOSSIER.md`, Bloco V (linhas 1120–1300)
- `docs/compliance/compliance_policies.md`
