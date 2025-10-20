# PR A2 — Estrutura de Diretórios e Permissões `/vvtv`

Este documento orienta a criação dos diretórios raiz, do usuário `vvtv` (UID 9001) e dos scripts operacionais iniciais.

## 1. Criação da Hierarquia

Execute o utilitário automatizado:

```bash
sudo ./scripts/provision/create_vvtv_directories.sh
```

O script realiza as ações:

- Cria `/vvtv/system`, `/vvtv/data`, `/vvtv/cache`, `/vvtv/storage`, `/vvtv/broadcast`, `/vvtv/monitor`, `/vvtv/vault`.
- Cria subdiretórios padrão (`/vvtv/system/bin`, `/vvtv/system/logs`, `/vvtv/broadcast/hls`, etc.).
- Ajusta permissões alinhadas ao dossiê (binários 755, bancos 660, logs 750, vault 700).
- Habilita SELinux/AppArmor quando suportado via rótulos básicos (placeholder para ajustes manuais).

## 2. Usuário `vvtv` (UID 9001)

- Em sistemas Linux, o script utiliza `useradd` com `--uid 9001 --system --create-home`.
- Em macOS, utiliza `dscl` para criar o usuário, seguindo as instruções do dossiê.
- O arquivo `/etc/sudoers.d/vvtv` não é criado automaticamente; conceda apenas os comandos necessários via `sudo` manual.

## 3. Scripts Utilitários Iniciais

Os scripts a seguir são instalados em `/vvtv/system/bin`:

- [`check_stream_health.sh`](../../scripts/system/check_stream_health.sh): valida se NGINX, FFmpeg e filas estão saudáveis.
- [`halt_stream.sh`](../../scripts/system/halt_stream.sh): interrompe o broadcast de forma segura.
- [`fill_buffer.sh`](../../scripts/system/fill_buffer.sh): aciona o pipeline de preenchimento de buffer via filas SQLite.

Cada script inclui logging estruturado em `/vvtv/system/logs`. Leia a seção "Validação" abaixo antes de executar em produção.

## 4. Logrotate e Retenção

- O template [`configs/logrotate/vvtv`](../../configs/logrotate/vvtv) implementa retenção de 14 dias para os logs do sistema.
- Instale copiando o arquivo para `/etc/logrotate.d/vvtv` e execute `sudo logrotate -f /etc/logrotate.d/vvtv` para validar.
- O script de diretórios cria um link simbólico em `/vvtv/system/logrotate/vvtv` para facilitar revisões.

## 5. Validação

1. Rode `sudo ./scripts/provision/create_vvtv_directories.sh --verify` para confirmar permissões.
2. Execute `sudo -u vvtv /vvtv/system/bin/check_stream_health.sh --dry-run` e verifique o log.
3. Liste as migrações: `ls -R /vvtv | head` para validar a hierarquia.

## 6. Checklist

- [ ] Usuário `vvtv` existe com UID 9001.
- [ ] Hierarquia `/vvtv` criada com permissões corretas.
- [ ] Scripts utilitários disponíveis e executáveis.
- [ ] Logrotate configurado para 7–14 dias.
- [ ] Logs de validação arquivados.
