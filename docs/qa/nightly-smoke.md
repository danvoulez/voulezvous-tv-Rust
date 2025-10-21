# QA Nightly Smoke Playbook

Este guia descreve como executar e analisar os smoke tests noturnos do curator usando o tooling de QA introduzido no Epic D.

## Pré-requisitos

- `browser.toml` atualizado com as seções de `fingerprint`, `retry`, `ip_rotation` e `observability`.
- `metrics.sqlite` migrado com o schema de `sql/metrics.sql`.
- Binário `vvtvctl` compilado com suporte ao comando `qa`.
- Tailscale configurado para permitir rotação de exit-nodes definidos em `browser.toml`.
- (Opcional) `ffmpeg` disponível no PATH para capturas de vídeo.

## Executando o smoke test

```bash
vvtvctl qa smoke-test \
    --url "https://exemplo.com/player" \
    --mode headed \
    --screenshot-dir /vvtv/artifacts/screenshots \
    --record-video \
    --video-dir /vvtv/artifacts/video
```

Parâmetros importantes:

- `--mode`: `headed` exibe o navegador para inspeção manual; `headless` usa automação invisível.
- `--no-screenshot`: desabilita captura automática de screenshot ao final do PBD.
- `--record-video`: ativa o `SessionRecorder`; caso `ffmpeg` não esteja disponível será gerado placeholder com aviso no relatório.
- `--record-duration`: limita a duração da captura de vídeo (padrão 30s).

Ao término, o comando reporta:

- Status (sucesso/falha) do fluxo Play-Before-Download.
- Caminhos de screenshot e vídeo.
- Métricas de tentativa (número de retries, rotações de proxy, taxa PBD em %).

Todos os resultados são persistidos em `metrics.sqlite` via tabelas `curator_runs`, `curator_failures` e `proxy_rotations`, e os detalhes estruturados são anexados em `logs/curator_failures.log`.

## Dashboard HTML

Para gerar o dashboard com estatísticas agregadas:

```bash
vvtvctl qa report --output /vvtv/artifacts/qa/dashboard.html
```

O relatório inclui:

- Número total de execuções e taxa de sucesso.
- Médias de duração e rotações de proxy.
- Contagem de detecções anti-bot.
- Timestamp da última execução.

## Agenda recomendada

1. **00:15 UTC** – executar smoke test headed em domínio crítico.
2. **00:45 UTC** – executar smoke headless para uma segunda origem.
3. **01:00 UTC** – gerar dashboard e anexar ao relatório diário.

## Pós-execução

- Verificar alertas no `curator_failures.log` para categorias `BotDetection`.
- Confirmar que rotações de Tailscale sucederam (`proxy_rotations` > 0 em incidentes).
- Abrir follow-up no Tasklist se alguma origem falhar em 2 noites consecutivas.

## Troubleshooting rápido

| Sintoma | Ação |
| --- | --- |
| `qa smoke-test` aborta com `captcha triggered` | Verificar rotação de IP (`tailscale status`), revisar fingerprinting. |
| Vídeo ausente no relatório | Confirmar instalação do `ffmpeg` ou revisar permissões de captura. |
| Dashboard vazio | Garantir que `metrics.sqlite` contém entradas em `curator_runs`. |

## Observabilidade

- Logs estruturados: `logs/curator_failures.log` (JSON por linha).
- Métricas: `metrics.sqlite` nas tabelas `curator_runs`, `curator_failures`, `proxy_rotations`.
- Para análises ad-hoc:

```bash
sqlite3 data/metrics.sqlite \
  "SELECT ts, url, category FROM curator_failures ORDER BY ts DESC LIMIT 5;"
```

Mantenha este guia sincronizado com ajustes futuros na automação e políticas de retry.
