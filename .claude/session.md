# Session context

## Последняя сессия (2026-05-30) — Фаза 4: дедуп bracket-макросов

Контекст: пункт Фазы 4 аудита «дедупликация `try_*_macro`». План:
`~/.claude/plans/dynamic-twirling-hedgehog.md`.

### Ветка `refactor/dedup-bracket-macros` (от master; НЕ закоммичено)

Всё в `adoc-parser/src/inline.rs`. Чистый рефакторинг, **поведение не изменено ни на байт**.
- **Уточнение премисы:** «14 функций → один хелпер» завышено. Большинство `try_*`
  (footnote/link/mailto/xref/cross_reference/autolink/anchor/index/attr_span/custom) имеют
  существенно разный разбор — НЕ трогали. Реальный дедуп — у двух семейств:
  - `parse_bracket_macro(&self, prefix_len) -> Option<(&'a str, usize)>` — `name:[content]`;
    callers: `try_kbd_macro`/`try_btn_macro`/`try_stem_macro`/`try_pass_macro`.
  - `parse_target_bracket_macro(&self, prefix_len) -> Option<(&'a str, &'a str, usize)>` —
    `name:target[items]`; callers: `try_menu_macro`/`try_icon_macro`.
- Хелперы (`&self`) делают только разбор `[…]` + расчёт `new_pos`; flush/эмиссия событий/политика
  пустоты остались в каждом caller. `new_pos` побайтово равен исходному `start_pos+prefix+end+1`.
  Использован `let Some((..)) = ... else { return false; };` (let-else, edition 2024).
- `try_stem_macro` сохранил параметр `prefix_len` (stem=5/latexmath=10/asciimath=10).

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings.
- `cargo test --workspace`: зелёное, 0 failed (parser 429, html lib 302, html_output 35,
  adoc_html_tests 6, author_rendering 6, html_compat 1, integration 25, parsing_lab 1).
- Корпус `compare_full.py` (release): **Identical 135 / Different 209 / Errors 0** — без изменений.
- TODO.md: пункт дедупа отмечен `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `refactor/dedup-bracket-macros` (только по запросу).
- Осталось из Фазы 4: doc-тесты публичного API (`to_html`/`push_html`/`Parser` — сейчас 0);
  README `233`→`238`. (Декомпозиция гигантских функций и дедуп — закрыты.)
- P3 кластеры совместимости: bare-links class+rel (п.14), backslash-entity (п.15), типографские
  замены (п.37), link-text (п.38). Доминирующий шум — NCR-типографика.

### Предостережения
- НЕ `cargo fmt` на крейт (не fmt-clean, компактный стиль). Коммит только по запросу.
- Верификация совместимости: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release-бинарь
  `target/release/adoc`, 344 файла). Baseline: Identical 135 / Different 209 / Errors 0.
- LSP (rust-analyzer) для навигации, context7 MCP для доков библиотек.
