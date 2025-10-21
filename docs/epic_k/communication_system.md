# Sistema de Comunicação e Escalonamento VVTV

Esta referência complementa o Apêndice B do dossiê industrial e descreve como o módulo de incidentes implementado no PR K3 funciona.

## Matriz de Severidade

| Gravidade | Canais | SLA sugerido |
| --- | --- | --- |
| Crítico | Telegram + Email + registro local | Imediato |
| Alto | Telegram + registro local | 30 min |
| Médio | Registro local (vault/incident_history) | 6 h |
| Baixo | Registro local | 24 h |

Os canais são controlados pelo arquivo `vvtv.toml` na seção `[communications]`. Ajuste a matriz conforme necessário para ambientes de teste.

## Configuração

```toml
[communications]
incident_history_dir = "/vvtv/vault/incident_history"

[communications.routing]
critical = ["telegram", "email"]
high = ["telegram"]
medium = ["log"]
low = ["log"]

[communications.telegram]
command = "telegram-send"
args = ["--config", "/vvtv/vault/keys/telegram.conf"]

[communications.email]
command = "sendmail"
sender = "VVTV Incident Bot <incident@voulezvous.tv>"
recipients = ["ops@voulezvous.tv", "foundation@voulezvous.tv"]
subject_prefix = "[VVTV]"
```

- **`incident_history_dir`**: pasta onde os relatórios Markdown/JSON são persistidos. Por padrão aponta para `vault/incident_history`.
- **`routing`**: mapa de severidade → canais. Valores aceitos: `telegram`, `email`, `log`.
- **`telegram`**: binário (`command`) e argumentos adicionais (`args`) usados para invocar `telegram-send`. Configure o arquivo `.conf` com o token e o chat ID antes de usar em produção.
- **`email`**: integração com `sendmail` (ou binário equivalente). Defina `recipients` e `sender`; o `subject_prefix` é anexado automaticamente ao assunto.

Para ambientes de desenvolvimento utilize `--dry-run` com o comando CLI para evitar disparar webhooks reais.

## Fluxo Operacional

1. Executar `vvtvctl incident report` com as flags do incidente.
2. O comando gera o postmortem Markdown (e opcionalmente JSON) no diretório configurado.
3. O `IncidentNotifier` envia mensagens para os canais apropriados. Falhas são reportadas diretamente na saída.
4. O histórico fica auditável no vault, permitindo reconstrução do evento.

### Exemplo rápido

```bash
vvtvctl incident report \
  --id INC-2025-07 \
  --title "Falha no encoder" \
  --severity high \
  --summary "Encoder travou durante transcode" \
  --impact "5 minutos de tela preta" \
  --root-cause "Thread FFmpeg bloqueada" \
  --action "Reiniciar encoder" \
  --lesson "Habilitar watchdog" \
  --dry-run
```

A flag `--dry-run` mantém o fluxo intacto (arquivos + registro), mas marca os canais como `skipped: dry-run`, útil para exercícios de resposta.
