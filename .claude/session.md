# Session context

## Сессия (2026-06-23, 8-я) — F-BK: ordered-список, утечка had_blank_line → `+`-continuation к внешнему пункту

Запрос «начни следующую задачу». master `feb2e38` (F-BJ смержен).

### Триаж (notes-корпус, метрика семантического DOM)
- `frontier_parity.py /mnt/c/Work/docs/notes/modules` → 73 identical, 8 чистых расхождений (по убыванию diff):
  plan(230)/qwen(194)/sbertech-index(134)/wsl(95)/keycloak(52)/tips(41)/synapse(23)/**antora-index(15)**.
- Взял самое чистое — `antora/pages/index.adoc` (15 token-diff). НЕ доверял метке прошлой сессии («admonition-в-списке»),
  проверил исходник + showdiff.

### Сделано — F-BK (ветка `fix/ordered-list-blank-line-leak` от master `feb2e38`)
**Баг:** `antora/index.adoc` — NOTE через `+` после последнего пункта ВЛОЖЕННОГО ordered-списка
(`.. Копируем токен` + `+` + `NOTE:`) у asciidoctor сидит ВНУТРИ `<li>` внутреннего списка, у нас выпадал наружу
к ВНЕШНЕМУ пункту.
**Корень (bisect-матрица минимальных проб):** баг требует ОДНОВРЕМЕННО (1) ведущий блок (абзац/секция) перед списком,
(2) вложенность ≥2. CRLF и trailing-space — красная селёдка (исключены пробами).
- `skip_blank_lines` (block.rs:354) ставит `had_blank_line=true` на пустой строке между ведущим блоком и списком.
- `scan_unordered_list_item` СБРАСЫВАЛ `had_blank_line=false` (стр. 3947), а `scan_ordered_list_item` — **НЕТ** →
  флаг утекал через все ordered-пункты до обработчика `+` (стр. 1351:
  `if had_blank_line && is_in_list_context() { close_nested_list_items() }` — закрывает всё до САМОГО ВНЕШНЕГО пункта).
- При пустой строке ПРЯМО перед `+` оба движка корректно цепляют к внешнему (asciidoctor 2.0.23 verified) — там blank
  ставит флаг заново непосредственно перед `+`; фикс это поведение СОХРАНЯЕТ.

**Фикс (1 строка, ТОЛЬКО adoc-parser):** `block.rs` — добавлен `self.had_blank_line = false;` в конец
`scan_ordered_list_item` (зеркало `scan_unordered_list_item`).
**Тесты (adoc-html/src/tests.rs):** +2 — `test_ordered_nested_continuation_after_leading_block_html` (NOTE цепляется
к вложенному пункту, инвариант close-sequence) + `test_ordered_nested_continuation_blank_before_plus_stays_outer_html`
(blank перед `+` → к внешнему, регресс-гард на фикс).

### Верификация
- clippy `--workspace` **0**. **test --workspace 1303 passed, 0 failed** (html 535→**537**, parser 647, compat 233,
  render-core 25, integration 29).
- **БАЙТ-НЕЙТРАЛЬНО на старых корпусах** (паттерн «ведущий блок + вложенный ordered + `+`» там отсутствует):
  - gate 344 — `gate_check.py` **0 diff** vs base `/tmp/adoc_base` (master `feb2e38`).
  - frontier(250)+adoc2docx(52)=302 — `/tmp/sweep_bvn.py` **0 diff**.
- **notes Identical 73→74** (antora/index.adoc → identical, выпал из списка расхождений).
- Матрица проб: ordered+para DIFFERS→IDENTICAL; unordered+para IDENTICAL; blank-before-+ IDENTICAL (сохранён);
  depth1+para, triple-nest+para — IDENTICAL.

### Состояние репо
- Ветка `fix/ordered-list-blank-line-leak` от master `feb2e38`, НЕ закоммичена (ждёт запроса коммит/merge/push).
- Изменено: `adoc-parser/src/block.rs` (1 строка в `scan_ordered_list_item`), `adoc-html/src/tests.rs` (+2 теста),
  TODO.md (+F-BK), session.md.
- `/tmp/adoc_base` = бинарь master `feb2e38` (актуален как база регресс-гарда этой сессии).

### Остаток notes (7 чистых расхождений — кандидаты на следующие сессии)
plan(230)/qwen(194)/sbertech-index(134)/wsl(95)/keycloak(52)/tips(41)/synapse(23). По убыванию diff; синапс(23) самый
чистый из оставшихся. Известные классы из прошлой сессии: admonition-в-списке (теперь частично закрыт F-BK),
ordered-list десинк, ложная Rouge-подсветка `[source,yaml]` без `:source-highlighter:`.

### Методология (без изменений)
`frontier_parity.py <roots>` / `showdiff.py <file>` (семантический DOM, ПРАВИЛЬНАЯ метрика для не-verbatim — байт только
ВНУТРИ `<pre>`, см. [[feedback_html_byte_parity_scope]]). `gate_check.py` + `/tmp/sweep_bvn.py` — байт регресс-гард.
Бинарь: `cargo build --release -p adoc-cli`. asciidoctor 2.0.23 для проб. НЕ доверять метке прошлой сессии — bisect-матрица
проб (см. [[feedback_frontier_triage]]). Источник реальных корпусов: `/mnt/c/Work/docs/notes/modules/` (81 .adoc).
