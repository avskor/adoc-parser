# Session context

## Сессия (2026-06-22) — F-AX: UI-макросы btn/menu inline-субституция (ветка `fix/ui-macro-btn-menu-inline-subst`, off master `1aed67a`, НЕ закоммичена)

Запрос «начни следующую задачу из TODO.md». Master `1aed67a` чист (F-AW `0d1cc1f`/`1aed67a` смержен+запушен).
Открытые `[ ]` синтетические «0 корпусного выигрыша».

### Задача (триаж adoc2docx)
`frontier_parity.py /mnt/c/tmp/adoc2docx`: 40 identical / 9 clean-div. `menu.adoc` (80 diff, позиционный десинк = single-root).
`btn:[~_Ok_~]` → мы `<b class="button">~_Ok_~</b>`, asciidoctor `<b class="button"><sub><em>Ok</em></sub></b>`;
`menu:View[_Zoom_ > Reset]` → submenu `_Zoom_` сырой, asciidoctor `<em>Zoom</em>`.

### Корень (verified asciidoctor 2.0.23 substitutors.rb + 5 проб — тот же класс, что F-AW)
`sub_macros` в цепочке `normal` ПОСЛЕ specialchars+quotes+replacements → markup в `[…]` UI-макроса субституируется ДО
извлечения макроса. Оба наших движка (subst `macros.rs::try_btn`/`try_menu`, legacy `inline.rs::try_btn_macro`/`try_menu_macro`)
отдают контент СЫРЫМ `Text` → рендерер эскейпил без субституции. Квотированный menu `"a > b"` (`build_menuseq`) УЖЕ
переразбирал сегменты — асимметрия с формальным `menu:t[…]`.

### Реализация (3 файла, ЧИСТО РЕНДЕРЕР — оба движка эмитят одинаковый raw-Text, рендерер = единая точка)
1. `lib.rs` — хелпер `render_ui_macro_inline(output, value, escape_quotes)`: парсит `parse_str_with_subs_options(value,
   current_subs(), opts)`; no-markup fast-path (len==1 && Text==value) → `html_escape_preserving_refs` (escape_quotes=true,
   menu — эскейпит `"`) / `html_escape_text_preserving_refs` (false, btn — `"` литерал); иначе `push_event` каждое.
2. `events.rs` button-ветка Text: `button_mode=false` → `render_ui_macro_inline(output,&text,false)` → `button_mode=true`
   (чистка чтобы вложенные Text шли нормальным path).
3. `inline.rs::render_menu`: target (menuref + menuseq) и каждая submenu/item часть через хелпер (escape_quotes=true).
   Split на `>` ОСТАЁТСЯ на сырых items (разделители литеральные до субституции), субституция ПОСЛЕ split.
kbd НЕ тронут (своя `kbd_mode`-ветка, split на `+`/`,`).

### Тесты (+2 html, после test_menu_no_items_html)
`test_btn_inline_subst_html` (`~_Ok_~`→sub/em, `*Bold*`→strong, char-ref `a&#167;b` сохранён, литерал `"q"`),
`test_menu_segment_inline_subst_html` (`_Zoom_`→em; `Save As...`→`…\u{200b}` ellipsis-replacements).

### Верификация
- clippy 0; **test --workspace 1270 зелёных** (html 513→515).
- **Гейт 344/344 байт-в-байт** vs master `1aed67a` (`gate_check.py` 0 diff). Базовый `/tmp/adoc_base` собран из `1aed67a`
  (stash→build→cp→pop→rebuild).
- **Frontier 250 + adoc2docx 52 new-vs-base sweep = РОВНО 1 файл** (menu.adoc, 80→0 diff, байт-в-байт), 0 регрессий.
- **adoc2docx 40→41 identical (+1).** menu.adoc showdiff пустой.
- 5 CLI-проб == asciidoctor 2.0.23.

### Побочно (улучшение, не регрессия)
menu-item `...`/`--` теперь кёрлятся replacements (semantically == asciidoctor; raw UTF-8 `…​` vs его NCR
`&#8230;&#8203;` — фоновая typographic-NCR разница, не флипает в одиночку).

### Состояние
Закоммичено? НЕТ. Коммит/merge --no-ff/push — ПО ЗАПРОСУ. TODO.md обновлён (F-AX в начало FRONTIER-секции).
ВАЖНО: ветка создана ПОСЛЕ правок (правки делались на master, перенесены через `git checkout -b`) — рабочее дерево чистое на ветке.

### Остаток adoc2docx clean-div (8, для будущего триажа)
xref(1495 — xrefstyle:full реф-лейблы фигур, архитектурный), test(1105), source(682), xml(291), callouts(196),
links(89 — `https://…[]` пустой-bracket autolink в кавычках + `[.overline]#`/role/id-title формы, возможно мульти-root),
sections(55 — нумерация спец-секций abstract/preface при sectnums), images(1 — xrefstyle:full, архитектурный).
