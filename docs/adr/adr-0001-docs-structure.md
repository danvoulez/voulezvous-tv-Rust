# ADR-0001 — Reorganização da Árvore de Documentação

## Contexto

Antes do Epic L a documentação estava distribuída em arquivos isolados (`epic_*`)
sem um índice central. Era difícil localizar runbooks, políticas e guias de
manutenção.

## Decisão

Criar um hub em `docs/` contendo:

- Guias principais (`deployment.md`, `failover.md`, `maintenance.md`,
  `compliance_policies.md`).
- Diretórios dedicados para operações (`docs/operations/`) e compliance
  (`docs/compliance/`).
- Scripts associados documentados explicitamente (`compliance_scan.sh`).
- ADR registrado para rastrear a mudança.

## Alternativas Consideradas

1. Manter estrutura antiga com `epic_*` — rejeitado (não escalava para operação
   física).
2. Migrar tudo para wiki externa — rejeitado (sem acesso offline garantido).

## Consequências

- Operadores têm um ponto único de referência.
- Facilita auditorias (tudo mapeado com relatórios e scripts).
- Necessidade de manter índice atualizado a cada alteração relevante.

## Status

Aceito — 2025-11-05
