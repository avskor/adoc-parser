# Session context

## Сессия (2026-06-23, 6-я) — F-BI: пустая строка в content-слоте пустой секции

Запрос «начни следующую задачу». master `8001e22` (F-BH смержен; прошлая session.md ошибочно помечала F-BH
«не закоммичена» — git log показал merge).

### Триаж дефернутых остатков (3 из 5 кандидатов ОКАЗАЛИСЬ УЖЕ ИСПРАВЛЕНЫ — метки устарели)
Проверено фактически (`adoc` vs asciidoctor 2.0.23, NCR-нейтрализация):
- **mailto/email курлинг** — УЖЕ работает (`Don’t`==`Don&#8217;t`). Метка устарела.
- **backtick-apostrophe `` `' ``** (старая задача 11) — УЖЕ работает (`It’s`). Устарела.
- **em-dash в inline code** (старая задача 13) — УЖЕ совпадает (`<code>--debug</code>`). Устарела.
- **arrow в bare URL** — РЕАЛЬНОЕ расхождение, но бой с архитектурой: наш порядок `macros`→`replacements` (поглощаем
  макрос целиком), у asciidoctor наоборот (`replacements`→`macros`), плюс наш URL-сканер стопает на `>`. Риск, отложено.
- **пустая секция** — РЕАЛЬНОЕ, чистый рендер-слой. ВЗЯТО.

### Сделано — F-BI (ветка `fix/empty-section-blank-line` от master `8001e22`)
**Корень:** шаблон секции asciidoctor оборачивает контент в `\n#{content}\n`. Непустое тело: завершающий `\n` даёт
последний дочерний блок (`</div>\n` параграфа) — совпадали. Пустое тело: `\n` терялся → sect1 `<div
class="sectionbody">\n</div>` вместо `…\n\n</div>`; sect2+ `</h3>\n</div>` вместо `…\n\n</div>`.
**Фикс (3 файла, ТОЛЬКО adoc-html):**
- `lib.rs`: поле `section_content_start: Vec<usize>` (+ init в `new()`).
- `blocks.rs` `start_section_div`: push placeholder `usize::MAX` (рядом с другими section-стеками).
- `events.rs` `TagEnd::SectionTitle`: `if let Some(slot)=...last_mut(){ *slot=output.len() }` (после `</hN>\n` и
  открытия `sectionbody` для sect1). Doctitle идёт через SectionTitle но НЕ через start_section_div (в header) → стек
  пуст → no-op, баланс с sect0_stack/sectionbody_stack сохранён.
- `events.rs` `TagEnd::Section`: pop `content_start`; в ветке `!is_sect0` если `output.len()==content_start` (тело
  ничего не написало) → `output.push('\n')` ПЕРЕД закрывающими `</div>`.
- +1 html-тест `test_empty_section_emits_blank_content_slot_html` (sect1 пуст / sect2 пуст / comment-only пуст +
  regression: непустой не получает лишнюю пустую строку).

### Верификация
- clippy `--workspace` **0**.
- **test --workspace 0 упавших** (html 533→**534**, parser 647, compat 233, render-core 25, html-compat зелёные —
  whitespace невидим семантическому DOM-сравнению, поэтому html-compat не ломается).
- **БАЙТ-ПАРИТЕТ vs asciidoctor — 0 регрессий, 10 файлов улучшено, 7 → полная байт-идентичность** (difcount через
  difflib, NCR-нейтрализация, content-регион `<div id="content">…</div></body>`):
  - Гейт(344): abstract/part/outline → **identical (0 diff)**; appendix 9→1, section 18→2 (остаток = sect0).
  - Sweep frontier+adoc2docx(302): include-with-leading-blank-line / multi-special-ex / sections / toc →
    **identical**; callouts 37→36 (остаток = Rouge-подсветка).
- gate_check.py: 5 файлов new≠base — ВСЕ улучшения (base→new diff = только добавленные пустые строки `>`, 0 удалений).

### Важный методологический вывод
Прошлая «корпуса исчерпаны» опиралась на НОРМАЛИЗОВАННУЮ метрику (`frontier_parity.py`/`showdiff.py` гонят
`normalize_html`, схлопывают whitespace → СЛЕПЫ к пустым строкам). **Байт-паритет НЕ был исчерпан.** Для поиска
байт-остатков нужно байтовое сравнение content-региона (см. `scratchpad/difcount.py`,`bytecmp.py`), не нормализованное.

### Новый найденный остаток (НЕ в scope F-BI)
**sect0 blank-between-siblings:** asciidoctor даёт пустую строку МЕЖДУ двумя соседними `<h1 class="sect0">` (level-0
секции / книжные части). Мы — нет (appendix/section residual: `…</h1>\n\n<h1 class=sect0>` vs наш `…</h1>\n<h1>`).
Отдельный sect0-баг (мой фикс намеренно его не трогает — гард `!is_sect0`, sect0 без wrapper-div). Кандидат на след.
задачу + потенциально целый класс byte-only расхождений (искать байтовым sweep).

### Состояние репо
- Ветка `fix/empty-section-blank-line` (от `8001e22`). **НЕ закоммичена** (коммит/merge — ПО ЗАПРОСУ).
- Изменены: `adoc-html/src/lib.rs`, `adoc-html/src/blocks.rs`, `adoc-html/src/events.rs` (+тест в `tests.rs`).
  TODO.md (+F-BI) и session.md обновлены.
- `/tmp/adoc_base` = бинарь master `8001e22` (для gate_check/sweep). `/tmp/adoc_fixed` = бинарь с фиксом.

### Методология (без изменений)
`gate_check.py` (base=`/tmp/adoc_base`, пересобирать из master через stash!), `/tmp/sweep_bvn.py` (frontier+adoc2docx
base-vs-new), `frontier_parity.py`/`showdiff.py` (НОРМАЛИЗОВАННАЯ метрика — НЕ видит whitespace!). Бинарь:
`cargo build --release -p adoc-cli` (имя `adoc`, нужен файл, `-a nofooter`). asciidoctor 2.0.23 для проб.
Байт-сравнение: см. `scratchpad/difcount2.py` (difflib opcode-diff, NCR-нейтрализация).
