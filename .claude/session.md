# Session context

## Последняя сессия (2026-05-30) — Фаза 4: doc-тесты публичного API

Контекст: пункт Фазы 4 аудита «doc-тесты для публичного API (to_html, push_html, Parser)».
Аддитивная задача (только документация), без plan-режима.

### Ветка `docs/public-api-doctests` (от master; НЕ закоммичено)

Было 0 doctests во всём workspace → стало 3, все зелёные.
- `adoc-html/src/lib.rs`: крейт-докстрока `//!` (обзор + ссылки на `to_html`/`push_html`);
  doctest на `to_html` (`to_html("Hello *world*")` → содержит `<strong>world</strong>`);
  doctest на `push_html` (`Parser::new("Hello")` → буфер содержит `Hello`).
- `adoc-parser/src/parser.rs`: докстрока на `struct Parser<'a>` + doctest
  (`Parser::new("Hello *world*").any(|ev| matches!(ev, Event::Start(Tag::Strong { .. })))`).

### Статус (верифицировано)
- `cargo test --workspace --doc`: adoc_html 2, adoc_parser 1 — все passed.
- `cargo clippy --workspace`: 0 warnings.
- `cargo test --workspace`: зелёное, 0 failed (parser 429, html lib 302, html_output 35,
  adoc_html_tests 6, author_rendering 6, html_compat 1, integration 25, parsing_lab 1,
  + doctests adoc_html 2 / adoc_parser 1).
- Корпус не запускал — изменение только в doc-комментариях, рендеринг не затронут.
- TODO.md: пункт doc-тестов отмечен `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `docs/public-api-doctests` (только по запросу).
- Осталось из Фазы 4: **README `233`→`238`** (последний пункт; устаревшее «233 cases» в README:16
  → фактические числа тестов/ASG-пар).
- Декомпозиция гигантских функций, дедуп try_*_macro, метаданные Cargo, сноска FEATURES.md,
  doc-тесты — все закрыты.
- P3 кластеры совместимости: bare-links class+rel (п.14), backslash-entity (п.15), типографские
  замены (п.37), link-text (п.38). Доминирующий шум — NCR-типографика.

### Предостережения
- НЕ `cargo fmt` на крейт (не fmt-clean, компактный стиль). Коммит только по запросу.
- Верификация совместимости: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release-бинарь
  `target/release/adoc`, 344 файла). Baseline: Identical 135 / Different 209 / Errors 0.
- LSP (rust-analyzer) для навигации, context7 MCP для доков библиотек.
