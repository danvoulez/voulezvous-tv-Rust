# Segurança de Distribuição

## Tokens de Segmento

- `DistributionSecurity` carrega a chave HMAC (`distribution.security.token_secret_path`).
- `token_ttl_minutes` controla expiração.
- Tokens emitidos via `DistributionManager::issue_token("/live/segment.ts")`.
- Auditoria em `cdn_tokens` (`metrics.sqlite`) e `/vvtv/system/logs/distribution_access.log`.

### Verificação

```rust
let security = DistributionSecurity::new(config.distribution.security.clone())?;
let token = security.issue_token("/live/segment1.ts")?;
security.validate_token(&token, "/live/segment1.ts")?;
```

## TLS e Firewall

- `tls_required = true` obriga certificados válidos (`certificate_path`).
- `firewall_sources` define origens permitidas (Tailscale + CDNs). O script de firewall deve consumir essa lista e gerar regras `iptables`/`pf`.

## Logging

`DistributionSecurity::record_access` registra IPs agregados (sem PII) para auditoria, permitindo correlação com alertas de intrusão.
