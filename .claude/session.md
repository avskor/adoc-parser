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

### ⚠ ПОПРАВКА ПОЛЬЗОВАТЕЛЯ (2026-06-23) — F-BI РАБОТА ВНЕ ЦЕЛИ
После merge/push пользователь: «для html байт паритет нужен только в листингах и примерах». То есть байт-паритет
значим ТОЛЬКО в verbatim `<pre>` (listing/source/literal/примеры); пустая строка между блочными тегами (`<div>` и т.п.)
браузером СХЛОПЫВАЕТСЯ → на рендеринг не влияет. html-compat (семантический DOM) её игнорировал — и это ПРАВИЛЬНО,
нормализованная метрика НЕ «слепа», а корректно не видит неважное. F-BI технически совпал с asciidoctor, но эффект
нулевой. Решение пользователя — **ОСТАВИТЬ** F-BI (безвреден, 0 регрессий), принцип применять к будущим задачам.
Память: [[feedback_html_byte_parity_scope]]; [[compat_corpus_methodology]] исправлена.

### НЕ кандидат на задачу: sect0 blank-between-siblings
asciidoctor даёт пустую строку между соседними `<h1 class="sect0">` (appendix/section residual). Это ТОТ ЖЕ класс —
whitespace вне `<pre>`, рендерингу безразличен → **НЕ браться** (по поправке выше). Не гнаться за байтовыми
whitespace-остатками; compat-триаж: байт-паритет только внутри verbatim-блоков, прочее — семантикой DOM.

### Состояние репо
- master `27ba193` (F-BI смержен `--no-ff`, запушен в origin; ветка `fix/empty-section-blank-line` удалена).
- Изменено: `adoc-html/src/{lib,blocks,events,tests}.rs`. TODO.md (+F-BI с поправкой) и session.md обновлены.
- `/tmp/adoc_base` = бинарь master `8001e22` (СТАРЫЙ — пересобрать из `27ba193` перед след. gate_check/sweep!).

### Методология
`gate_check.py` (base=`/tmp/adoc_base`, пересобирать из master через stash!), `/tmp/sweep_bvn.py` (frontier+adoc2docx
base-vs-new) — БАЙТОВЫЕ, держать как РЕГРЕСС-гард, не parity-таргет для не-verbatim. `frontier_parity.py`/`showdiff.py`
(нормализованная, семантический DOM — ПРАВИЛЬНАЯ метрика для не-verbatim). Бинарь: `cargo build --release -p adoc-cli`
(`adoc`, нужен файл, `-a nofooter`). asciidoctor 2.0.23 для проб. Байт-сравнение verbatim: `scratchpad/difcount2.py`.
