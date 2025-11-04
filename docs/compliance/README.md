# Compliance Toolkit

Coleção de guias e procedimentos para auditorias de consentimento, DRM e CSAM.
Alinhado ao Epic L4.

## Documentos

- [License Audit](./license_audit.md)
- [DRM Detection](./drm_detection.md)
- [CSAM Response](./csam_response.md)

## Ferramentas

- `vvtvctl compliance audit|drm|csam|suite`
- `scripts/system/compliance_scan.sh`
- Diretório `vault/compliance/` contendo:
  - `license_logs/` (JSONL assinados)
  - `csam/hashes.csv`
  - `reports/` (histórico de auditorias)

## Rotina Recomendada

1. Rodar `compliance_scan.sh` diariamente.
2. Atualizar `license_audit.md` semanalmente com findings.
3. Sincronizar hashset CSAM mensalmente e registrar procedência.
