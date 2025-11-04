# VoulezVous.TV Documentation Hub

Este diretório reúne os artefatos operacionais do VoulezVous.TV Industrial. A
estrutura segue o plano do `VVTV INDUSTRIAL DOSSIER.md` e está organizada para
facilitar a vida dos operadores físicos e dos agentes autônomos.

## Índice Rápido

- [Deployment](./deployment.md): rituais de instalação, provisionamento e
  hardening inicial do nó broadcast.
- [Failover](./failover.md): como executar o swap entre origins, CDN e backups.
- [Maintenance](./maintenance.md): rotinas diárias/semanais/mensais e planos de
  manutenção preventiva.
- [Compliance Policies](./compliance_policies.md): políticas, checklist
  regulatório e fluxos de auditoria.
- [Operations](./operations/README.md): runbooks detalhados, manual do operador
  e procedimentos físicos.
- [Compliance Toolkit](./compliance/README.md): referência para auditorias de
  licenças, varredura CSAM e detecção DRM.
- [QA & CI](./qa/ci.md): pipeline de testes locais, cobertura e integração com o
  `cargo tarpaulin`.
- [ADR-0001](./adr/adr-0001-docs-structure.md): justificativa para a
  reorganização desta árvore de documentação.

> **Fonte de verdade**: sempre confirme detalhes no `VVTV INDUSTRIAL
> DOSSIER.md`. Este hub aponta para as seções relevantes e inclui atalhos de
> execução rápida.

## Convenções

- Todos os arquivos possuem versão assinada via `logline sign` quando aplicável.
- Scripts citados nos guias residem em `scripts/system/` e são idempotentes.
- Use `vvtvctl --format json` para gerar artefatos auditáveis dos comandos
  mencionados.
- Diagramas utilizam Mermaid e podem ser atualizados diretamente no Markdown.

Boa leitura e bons turnos! 🛰️
