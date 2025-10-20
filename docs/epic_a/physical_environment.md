# PR A1 — Provisionar ambiente físico e rede de operação

Este guia operacionaliza o inventário do hardware, a validação ambiental e a padronização visual exigida pelo Bloco I do dossiê.
Execute os passos na ordem a seguir antes de instalar qualquer software.

## 1. Inventário de Hardware

1. Conecte todos os dispositivos (Mac Mini principal, unidade de armazenamento externa NVMe, UPS ≥1500 VA).
2. Faça login com um usuário administrativo e execute:
   ```bash
   sudo ./scripts/provision/inventory_environment.sh
   ```
   O script coleta CPU, memória, armazenamento, versão do sistema operacional e dados básicos de sensores (temperatura/umidade
   quando disponíveis). Os relatórios são persistidos em `/vvtv/system/logs/hardware_inventory_*.md`.
3. Valide manualmente os resultados e anexe fotos do número de série ao relatório para auditoria física.

## 2. Validação de Temperatura e Umidade

- Utilize um sensor externo (Shelly Plus HT, Aqara ou equivalente). Registre os valores no relatório gerado pelo script.
- O ambiente deve permanecer entre **20–24 °C** e **45–55% de umidade relativa**. Caso contrário, registre o incidente e acione a
  climatização.

## 3. Infraestrutura Elétrica

1. Conecte a UPS em circuito dedicado e rotule os cabos de alimentação conforme o template disponível em
   `docs/visual_identity/cable_labels.svg` (placeholder para impressão, substitua pelo design final).
2. Utilize o checklist no final deste documento para confirmar:
   - Capacidade da UPS ≥1500 VA.
   - Mac Mini e storage conectados à UPS.
   - Monitoramento de consumo ativado (funcionalidade nativa da UPS ou sensor externo).
   - Documentação fotográfica arquivada em `/vvtv/vault/layout/`.

## 4. Malha Tailscale

A parte lógica da Tailscale é detalhada em [`network_and_tailscale.md`](network_and_tailscale.md). Entretanto, durante a fase
física:

- Identifique o hostname padrão do nó (ex.: `vvtv-node-primary`).
- Certifique-se de que a UPS e demais dispositivos críticos estejam ligados à mesma VLAN do Mac Mini.
- Verifique se a saída da UPS está aterrado corretamente para evitar ruído na rede.

## 5. Padrões Visuais e Layout

- Instale tapetes antiestáticos e organize os cabos utilizando canaletas pretas com etiquetas cinza.
- Operadores devem utilizar **luvas cinza** e **unhas grafite fosco**, conforme Apêndice H do dossiê.
- Documente o layout físico, incluindo fotos panorâmicas, dimensões e a posição dos equipamentos. Armazene os arquivos em
  `/vvtv/vault/layout/` e registre o índice em `docs/epic_a/layout_index.md` (crie o arquivo ao capturar as fotos).

## 6. Checklist de Conclusão

- [ ] Relatório de inventário exportado em `/vvtv/system/logs/hardware_inventory_*.md`.
- [ ] Temperatura/umidade dentro da faixa e registradas.
- [ ] UPS configurada, cabos rotulados e fotos arquivadas.
- [ ] Layout documentado e disponível no vault.
- [ ] Hostname reservado para a malha Tailscale.

> Após completar este checklist, avance para [`network_and_tailscale.md`](network_and_tailscale.md).
