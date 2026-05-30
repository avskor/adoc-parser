# Session context

## Последняя сессия (2026-05-30) — P2/Гигиена: Cargo-метаданные + сноска FEATURES.md

Контекст: все дефекты аудита D1-D7 закрыты и в master (`51a650e` P0, `bc7c1b2` P1,
`1024813` единое экранирование + D7). Взят следующий невыполненный пункт аудита из раздела
«Из аудита 2026-05-30» — **Гигиена** (метаданные Cargo + сноска FEATURES.md). Это был
единственный оставшийся `[ ]` в audit-секции.

### Ветка `chore/cargo-metadata-and-features-note` (от master; НЕ закоммичено)

8 файлов, только манифесты + документация (кода не трогал):
- **root `Cargo.toml`**: добавлен `[workspace.package]` с общими `license = "MIT"` и
  `repository = "https://github.com/avskor/adoc-parser"` (источник правды — файл `LICENSE`,
  MIT © 2026 Alexey Skorobogatov; remote `git@github.com:avskor/adoc-parser.git`).
- **6 крейтов**: `description` (inline, уникальный на крейт) + `license.workspace = true` +
  `repository.workspace = true`. Наследование выбрано т.к. license/repository общие (6→1);
  description уникален → inline.
- **Пиннинг semver** (по Cargo.lock, до минора): `adoc-cli` clap `4`→`4.5`;
  `adoc-compat-tests` serde `1`→`1.0`; `adoc-html-tests` similar `2`→`2.7`. chrono оставлен
  `0.4` (для 0.x минор = единица semver-совместимости; тоньше пинить = patch, не нужно).
  Каждая из 4 внешних зависимостей живёт ровно в одном крейте → `[workspace.dependencies]`
  не нужен (нет дублирования).
- **FEATURES.md**: «**Покрытие: 100% полное (202/202)**» → «**Покрытие синтаксиса: 100%
  (202/202)**[^coverage]» + сноска: это покрытие *синтаксических конструкций*, а не
  побайтовая HTML-совместимость; совместимость рендеринга = корпус 135/344 (Identical 135 /
  Different 209 на 2026-05-30).

### Статус (верифицировано)
- `cargo metadata --no-deps`: OK (манифесты парсятся).
- `cargo clippy --workspace`: 0 warnings.
- `cargo test --workspace`: зелёное, 0 failed (parser 429, html lib 302, html_output 35,
  adoc_html_tests 6, author_rendering 6, html_compat 1, integration 25, parsing_lab 1).
- `Cargo.lock` НЕ изменился (пиннинги удовлетворяются уже зафиксированными версиями:
  clap 4.5.58, serde 1.0.228, similar 2.7.0, chrono 0.4.44).
- TODO.md: пункт «Гигиена» отмечен `[x]` с деталями.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `chore/cargo-metadata-and-features-note`
  (по правилу CLAUDE.md — коммит только по запросу пользователя).
- Осталось из аудита/P2: декомпозиция гигантских функций (`start_tag` ~960 стр.,
  `parse_inline`, `scan_next_block`), дедуп `try_*_macro` в inline.rs, doc-тесты публичного
  API (0), README `233`→`238`.
- P3 кластеры совместимости: bare-links class+rel (п.14), backslash-entity (п.15),
  типографские замены (п.37), link-text (п.38). Доминирующий шум корпуса — NCR-типографика
  (229 файлов, в одиночку 0 flips — чинить в связке).

### Предостережения
- НЕ `cargo fmt` на крейт (не fmt-clean, компактный стиль). Коммит только по запросу.
- Верификация совместимости: `compare_full.py` в `/mnt/c/tmp/adoc-test/` (release-бинарь
  `target/release/adoc`), корпус 344 `.adoc`. Baseline: Identical 135 / Different 209 / Errors 0.
- LSP (rust-analyzer) для навигации, context7 MCP для доков библиотек.
