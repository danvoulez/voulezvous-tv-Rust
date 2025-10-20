# PR A4 — Configuração do NGINX-RTMP Origin

Este guia descreve a configuração mínima do NGINX-RTMP com saída HLS/HLS fallback conforme especificado no Epic A.

## 1. Arquivo de Configuração

O template [`configs/nginx/nginx.conf`](../../configs/nginx/nginx.conf) deve ser copiado para `/vvtv/broadcast/nginx.conf`.

Principais características:

- `rtmp` escuta na porta 1935 com aplicação `live`.
- Restrições de publicação/playback via tokens/allowlist (`publish_notify`/`play_restart` placeholders).
- Saída HLS em `/vvtv/broadcast/hls` com segmentos de 4s e playlist de 48 minutos (720 segmentos).
- Endpoint `/status` exposto via módulo `stat` com autenticação básica.
- Cache-control explícito para evitar armazenamento intermediário indesejado.

## 2. Systemd Unit

Crie `/etc/systemd/system/vvtv-nginx.service` baseado no snippet abaixo:

```ini
[Unit]
Description=VVTV NGINX-RTMP Origin
After=network.target

[Service]
User=vvtv
Group=vvtv
ExecStart=/usr/sbin/nginx -c /vvtv/broadcast/nginx.conf
ExecReload=/bin/kill -s HUP $MAINPID
Restart=on-failure
RestartSec=5
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

## 3. Validação do Serviço

1. `sudo systemctl daemon-reload && sudo systemctl enable --now vvtv-nginx.service`.
2. `sudo systemctl status vvtv-nginx.service` deve mostrar processo ativo.
3. Faça publish de teste: `ffmpeg -re -i sample.mp4 -c copy -f flv rtmp://localhost/live/ingest`.
4. Valide playlist: `curl http://localhost:8080/hls/live/index.m3u8`.
5. Consulte métricas: `curl http://localhost:8080/status` (usar autenticação definida no template).

## 4. Monitoramento Básico

- O script [`check_stream_health.sh`](../../scripts/system/check_stream_health.sh) verifica a presença de segmentos recentes e se
  o processo FFmpeg está ativo.
- Adicione ao `crontab` para execução a cada 5 minutos (`*/5 * * * *`).
- Configure alertas via webhook/Telegram (placeholder). Os incidentes devem ser logados em
  `/vvtv/system/logs/nginx_health.log`.

## 5. Checklist

- [ ] `nginx.conf` provisionado em `/vvtv/broadcast/nginx.conf`.
- [ ] Serviço systemd habilitado e ativo.
- [ ] Segmentos HLS gerados a cada 4 segundos.
- [ ] Endpoint `/status` validado.
- [ ] Monitoramento básico integrado.
