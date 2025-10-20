# PR A5 — Esquemas SQLite Iniciais

Esta seção reúne as migrações e rotinas de integridade para `plans.sqlite`, `queue.sqlite`, `metrics.sqlite` e `economy.sqlite`.

## 1. Migrações

- [`sql/plans.sql`](../../sql/plans.sql)
- [`sql/queue.sql`](../../sql/queue.sql)
- [`sql/metrics.sql`](../../sql/metrics.sql)
- [`sql/economy.sql`](../../sql/economy.sql)

Aplique as migrações com:

```bash
sudo -u vvtv sqlite3 /vvtv/data/plans.sqlite < sql/plans.sql
sudo -u vvtv sqlite3 /vvtv/data/queue.sqlite < sql/queue.sql
sudo -u vvtv sqlite3 /vvtv/data/metrics.sqlite < sql/metrics.sql
sudo -u vvtv sqlite3 /vvtv/data/economy.sqlite < sql/economy.sql
```

## 2. Testes de Integridade

Utilize o helper [`scripts/system/run_sqlite_integrity.sh`](../../scripts/system/run_sqlite_integrity.sh):

```bash
sudo -u vvtv ./scripts/system/run_sqlite_integrity.sh
```

O script executa `PRAGMA integrity_check`, valida índices obrigatórios e registra os resultados em
`/vvtv/system/logs/sqlite_integrity.log`.

## 3. Métricas e Observabilidade

- As tabelas incluem gatilhos para manter `updated_at` e contadores de falhas (ver migrações).
- Integre com o health check (`check_stream_health.sh`) para coletar métricas de fila/buffer/temperatura.
- Scripts adicionais podem ser adicionados em `scripts/system/metrics/` conforme evolução dos Epics B e C.

## 4. Checklist

- [ ] Migrações executadas com sucesso.
- [ ] Integridade validada (`PRAGMA integrity_check` retornando `ok`).
- [ ] Logs armazenados em `/vvtv/system/logs/sqlite_integrity.log`.
- [ ] Índices disponíveis conforme especificação.
