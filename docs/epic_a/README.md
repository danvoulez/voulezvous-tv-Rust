# Epic A — Fundamentos Físicos e Setup Base

Este diretório consolida os artefatos necessários para entregar o Epic A do VoulezVous.TV Industrial. Cada subdocumento e script
foi estruturado a partir do dossiê técnico para permitir que um operador execute o setup em hardware real com validações
reprodutíveis.

## Componentes

- [`physical_environment.md`](physical_environment.md): inventário de hardware, requisitos ambientais e procedimentos visuais.
- [`network_and_tailscale.md`](network_and_tailscale.md): configuração de rede operacional e integração com a malha Tailscale.
- [`filesystem_and_permissions.md`](filesystem_and_permissions.md): criação da hierarquia `/vvtv`, permissões e scripts utilitários
  iniciais.
- [`software_stack.md`](software_stack.md): automação de instalação das dependências base e hardening dos serviços.
- [`nginx_origin.md`](nginx_origin.md): blueprint do serviço NGINX-RTMP/HLS e monitoração associada.
- [`sqlite_foundations.md`](sqlite_foundations.md): esquemas de banco de dados e rotinas de integridade.

Cada documento referencia scripts, templates de configuração e migrações versionados neste repositório. Execute os passos na
ordem indicada para obter um nó operacional.
