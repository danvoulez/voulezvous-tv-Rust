# Operações de PLANs — VVTVCTL

Este guia resume os comandos CLI implementados no epic C para operadores acompanharem o ciclo de vida dos PLANs.

## Auditoria

```bash
vvtvctl --config configs/vvtv.toml plan audit --min-age-hours 6
```

- `--min-age-hours`: filtra findings mais antigos que o limite (padrão `0`).
- `--kind`: restringe para um tipo específico (`expired`, `missing_license`, `hd_missing`, `stuck`).
- Saída padrão em texto; use `--format json` para JSON estruturado.

## Lista de planos

```bash
vvtvctl plan list --status selected --limit 20
```

- Mostra identificadores, status, tipo (`kind`), score, duração estimada e flags (`hd_missing`).
- Datas são exibidas em ISO 8601 UTC.

## Blacklist de domínios

Adicionar domínio problemático:

```bash
vvtvctl plan blacklist add example.com --reason "CDN instável"
```

Remover domínio:

```bash
vvtvctl plan blacklist remove example.com
```

Listar entradas:

```bash
vvtvctl plan blacklist list
```

## Importação manual de planos

Permite inserir seeds a partir de arquivos JSON.

```bash
vvtvctl plan import /vvtv/system/seeds/sample_plans.json --overwrite
```

Formato aceito:

- Array de objetos `Plan` completos.
- Array de objetos `{ "plan": { ... }, "overwrite": false }`.
- Objeto único (será convertido internamente em array).

Use `--overwrite` para forçar atualização de registros existentes.

## Observabilidade

O comando `vvtvctl status` agora agrega `plan_metrics` com totais, média de score e contagem de `hd_missing`, auxiliando decisões de planejamento.
