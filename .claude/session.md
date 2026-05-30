# Session context

## Последняя сессия (2026-05-30) — Фаза 4: README (уточнение метрик) — ФАЗА 4 ЗАКРЫТА

Контекст: последний пункт Фазы 4 аудита «README 233→238». **Премиса оказалась ложной.**

### Ветка `docs/readme-test-counts` (от master; НЕ закоммичено)

- ⚠️ **«238» — ложная находка аудита.** Верифицировано: в `vendor/asciidoc-parsing-lab/test/tests`
  ровно 233 `*-input.*` + 233 `*-output.json` + 233 пары; тест `parsing_lab` печатает
  `Total: 233, Passed: 233`; submodule запинен (d46f77d). README «233/233 passing» уже был верен.
  Менять 233→238 НЕЛЬЗЯ — сделало бы README неверным.
- По выбору пользователя — вместо ложной правки **уточнён смысл метрики** (`README.md`, только docs):
  - строка `adoc-compat-tests` → «Structural conformance vs asciidoc-parsing-lab ASG fixtures
    (233/233 passing)»;
  - добавлена строка `adoc-html-tests` (HTML-output compatibility vs Asciidoctor, semantic DOM);
  - пояснение под таблицей: 233/233 = *структурная* конформность; побайтовая HTML-совместимость —
    отдельно (adoc-html-tests);
  - в раздел Testing добавлена команда `cargo test -p adoc-html-tests`.
  - Числа 135/344 НЕ вносил (внешний корпус `/mnt/c/tmp/adoc-test/`, не в репозитории; подвижное).

### Статус
- Правка только в `README.md` (не компилируется) — build/test/clippy не требуются и не запускались.
- TODO.md: пункт отмечен `[x]`. **ВСЯ Фаза 4 закрыта** (декомпозиция ×3, дедуп, doc-тесты,
  Cargo-метаданные, FEATURES.md сноска, README).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `docs/readme-test-counts` (только по запросу).
- Фаза 4 (качество/архитектура) и все пункты аудита P0-P3/D1-D7 + гигиена — закрыты.
- Остаётся **Фаза 3 — совместимость с Asciidoctor** (основной объём, P3-кластеры): bare-links
  class+rel (п.14), backslash-entity (п.15), типографские замены (п.37), link-text (п.38),
  остаток source-регрессий (п.40-смежное). Доминирующий шум корпуса — NCR-типографика
  (229 файлов, в одиночку 0 flips — чинить в связке). Baseline: Identical 135/344.

### Предостережения
- НЕ `cargo fmt` на крейт (не fmt-clean, компактный стиль). Коммит только по запросу.
- **Верифицировать находки аудита перед действием** — «238» был ложным (как и предупреждала память
  audit_2026-05-30). Корпус: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release `target/release/adoc`,
  344 файла), baseline Identical 135 / Different 209 / Errors 0.
- LSP (rust-analyzer) для навигации, context7 MCP для доков библиотек.
