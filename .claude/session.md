# Session context

## Последняя сессия (2026-05-30) — Фаза 4: декомпозиция `parse_inline`

Контекст: продолжение пункта Фазы 4 «декомпозиция гигантских функций» (после `start_tag`).
Взята `InlineState::parse_inline` (393 строки). План: `~/.claude/plans/dynamic-twirling-hedgehog.md`.

### Ветка `refactor/decompose-parse-inline` (от master; НЕ закоммичено)

Всё в `adoc-parser/src/inline.rs`. Чистый рефакторинг, **поведение не изменено ни на байт**.
- Структура `parse_inline`: пролог + `while { match b { ~40 arms } }`, где почти каждый arm уже
  минимален (`if self.try_X(events, &mut text_start) { continue } self.pos += 1`).
- Декомпозиция = 4 под-диспетчера `-> bool`, вызываемые в ИСХОДНОМ порядке (true = обработан):
  `handle_inline_escape` (5×`\` + hard-break), `handle_inline_passthrough` (+++/++/+),
  `handle_inline_formatting` (QUOTES: */_/"/'/`/#/^/~), `handle_inline_macro` (всё от `<<` до
  catch-all `a..z`, incl. inline-attr-span `[` — оставлен на исходной позиции, без переупорядочивания).
- Перенос arm'ов дословный: `continue`→`return true`, хвост `pos+=1`→`true`; `&mut text_start`→
  `text_start`; в escape `text_start`→`*text_start`. Каждый метод завершается `_ => false`.
- `handle_inline_macro` получил параметр `has_quotes` (нужен guard'у attr-span).
- Размер `parse_inline`: **393 → 32 строки** (пролог + цикл из 4 вызовов + `self.pos += 1`).

### Корректность порядка (ключевое)
`match` порядко-зависим; сохранён точный порядок arm'ов. Группы вызываются escape→passthrough→
formatting→macros, что совпадает с исходной последовательностью arm'ов (QUOTES-блок 267-350 шёл до
`<<` 353; attr-span остался среди макросов на месте). Нулевое переупорядочивание → строго безопасно.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings.
- `cargo test --workspace`: зелёное, 0 failed (parser 429, html lib 302, html_output 35,
  adoc_html_tests 6, author_rendering 6, html_compat 1, integration 25, parsing_lab 1).
- Корпус `compare_full.py` (release): **Identical 135 / Different 209 / Errors 0** — без изменений.
- TODO.md: `parse_inline` отмечен `[x]`.

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `refactor/decompose-parse-inline` (только по запросу).
- Осталось из Фазы 4: декомпозиция `scan_next_block_once` (~380, block.rs — каскад `if let`-детекторов,
  деликатный: разнородные return None+rescan / Some / self.next()); дедуп `try_*_macro` (14 шт.);
  doc-тесты публичного API (0); README `233`→`238`.
- P3 кластеры совместимости: bare-links class+rel (п.14), backslash-entity (п.15), типографские
  замены (п.37), link-text (п.38). Доминирующий шум — NCR-типографика.

### Предостережения
- НЕ `cargo fmt` на крейт (не fmt-clean, компактный стиль). Коммит только по запросу.
- Верификация совместимости: `python3 /mnt/c/tmp/adoc-test/compare_full.py` (release-бинарь
  `target/release/adoc`, 344 файла). Baseline: Identical 135 / Different 209 / Errors 0.
- LSP (rust-analyzer) для навигации, context7 MCP для доков библиотек.
