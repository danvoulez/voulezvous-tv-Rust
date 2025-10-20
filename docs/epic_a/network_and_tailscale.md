# Rede Operacional e Malha Tailscale

Este documento complementa o PR A1, detalhando como inicializar e auditar a malha `voulezvous.ts.net` nos nós físicos.

## 1. Topologia Recomendada

- **Segmento Primário (LAN Operacional):** Mac Mini principal, storage externo, monitor de produção.
- **Segmento Secundário (LAN de Backup):** Nó de contingência, sensores ambientais, workstation de manutenção.
- **VPN Mesh (Tailscale):** Hostnames fixos `vvtv-node-primary`, `vvtv-node-backup`, `vvtv-ops-laptop`.

![Topologia Tailscale](../visual_identity/network_topology_placeholder.png)

## 2. Pré-Requisitos

- Conta Tailscale com ACL configurada para restringir tráfego a portas 22, 80, 1935, 8080 dentro do domínio.
- Token de autenticação reutilizável (Machine Key) armazenado no cofre físico.

## 3. Automação de Provisionamento

Execute o script abaixo no nó recém-instalado:

```bash
sudo ./scripts/provision/setup_tailscale.sh \
  --hostname vvtv-node-primary \
  --auth-key tskey-xxxxxxxxxxxxxxxx \
  --advertise-tags "tag:vvtv,tag:broadcast"
```

O script garante a instalação do pacote (Homebrew ou APT), aplica hardening básico e registra o estado em
`/vvtv/system/logs/tailscale_setup.log`.

## 4. Verificações Pós-Provisionamento

1. `tailscale status` deve listar os nós com latência < 20 ms na mesma região.
2. `tailscale ip -4` deve retornar IP na sub-rede CGNAT (100.64.0.0/10). Documente o endereço em `docs/epic_a/layout_index.md`.
3. Valide conectividade RTMP (`nc -vz <peer> 1935`) e HLS (`curl http://<peer>:8080/status`).
4. Execute o health check de malha disponível em `scripts/provision/setup_tailscale.sh --verify-only` quando precisar revalidar.

## 5. Hardening de Rede

- Configure firewall local permitindo apenas tráfego proveniente do CIDR Tailscale.
- Desative serviços mDNS/Bonjour, AirDrop e discovery automático em produção.
- Registre qualquer alteração de ACL no vault (`/vvtv/vault/network/acl_change_log.md`).

## 6. Checklist

- [ ] Hostname registrado na Tailscale.
- [ ] Tags aplicadas (`tag:vvtv`, `tag:broadcast` ou equivalentes).
- [ ] Firewall atualizado para restringir tráfego a RTMP/HLS/SSH.
- [ ] Latência e IP documentados.
- [ ] Logs arquivados em `/vvtv/system/logs/tailscale_setup.log`.
