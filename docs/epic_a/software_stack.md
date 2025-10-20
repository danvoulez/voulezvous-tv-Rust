# PR A3 — Instalação da Stack Base de Software

Este documento descreve como instalar a stack necessária (FFmpeg, SQLite, NGINX-RTMP, aria2, Chromium, Rust toolchain e Tailscale)
e aplicar ajustes mínimos de hardening.

## 1. Automatização

Execute o script:

```bash
sudo ./scripts/install/install_base_stack.sh --with-firewall
```

O script suporta macOS (Homebrew) e Debian-based Linux (APT) e realiza as ações:

- Instala dependências obrigatórias com codecs habilitados (`ffmpeg` com `--enable-libx264 --enable-libx265 --enable-libopus`).
- Instala SQLite3, aria2, Chromium/Chromium-browser, Rustup com toolchain estável e Tailscale.
- Verifica versões mínimas: FFmpeg ≥ 6.1, SQLite ≥ 3.41, Rust ≥ 1.74.
- Configurações opcionais (`--with-firewall`) para aplicar regras básicas via `ufw` ou `pfctl`.
- Desativa serviços indesejados (Spotlight, Sleep, Time Machine no macOS; suspend/hibernate no Linux) quando `--harden` for
  especificado.

## 2. Flags de Compilação

- Para macOS, o script utiliza `brew install ffmpeg --with-srt --with-opus` (Homebrew 3.x) e valida suporte a `rtmp`, `hls`, `srt`.
- Para Linux, utiliza pacotes `ffmpeg`, `libnginx-mod-rtmp`, `aria2`, `chromium`, além de compilar módulos ausentes quando
  necessário via `apt source` + `dpkg-buildpackage` (placeholder com instruções).

## 3. Health Check Script

Ao final, o script registra `/vvtv/system/bin/check_stream_health.sh` em `crontab` opcional através da flag `--register-healthcron`:

```bash
sudo ./scripts/system/check_stream_health.sh --dry-run
sudo ./scripts/install/install_base_stack.sh --register-healthcron
```

## 4. Hardening Complementar

- Desabilite Spotlight (macOS): `sudo mdutil -a -i off`.
- Desative Time Machine automático: `sudo tmutil disable`.
- Ajuste suspensão: `sudo systemsetup -setcomputersleep Never`.
- No Linux, configure `systemctl mask sleep.target suspend.target hibernate.target`.
- Configure firewall interno com as portas necessárias (22/Tailscale, 1935 RTMP, 8080 HLS) e logue alterações em
  `/vvtv/system/logs/firewall_changes.log`.

## 5. Validação

- `ffmpeg -hide_banner -codecs | grep -E "(h264|aac|libx265|opus|srt)"` deve listar os codecs habilitados.
- `nginx -V` deve listar o módulo RTMP.
- `rustc --version` ≥ 1.74.0.
- `tailscale status` executa sem erros.

## 6. Checklist

- [ ] Dependências instaladas com versões mínimas garantidas.
- [ ] Serviços indesejados desativados.
- [ ] Firewall configurado e logado.
- [ ] Health check registrado conforme necessário.
- [ ] Logs de instalação arquivados em `/vvtv/system/logs/install_base_stack.log`.
