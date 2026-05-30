# Session context

## Последняя сессия (2026-05-30) — P2: единая дисциплина экранирования + D7

Полный отчёт аудита: `~/.claude/plans/sequential-dreaming-zebra.md`. План этой сессии:
`~/.claude/plans/polished-purring-bubble.md`.

**Контекст:** все дефекты аудита D1-D6 закрыты и в master (`51a650e` P0, `bc7c1b2` P1).
Взят следующий пункт аудита — P2 «единая дисциплина экранирования» (системный корень D1).

### Ветка `fix/attr-escaping-discipline` (от master; НЕ закоммичено)

Всё в `adoc-html/src/lib.rs`:
- **Хелпер `write_attr(output, name, value)`** (рядом с `html_escape`) — эмитит ` name="value"`
  c `html_escape` значения. Канонический путь для любого single-value атрибута.
- **D7 (новая XSS, найдена и закрыта):** `style_name` упорядоченного списка (`meta.style`) писался
  сырым в `<ol class>`/`<div class="olist …">` (`[<b>x]` → инъекция тега; `<`/`>`/`&` проходят).
  D1 чинил только media/image. Закрыто.
- **Экранирование на границе эмиссии:** `write_meta_attrs` теперь экранирует `default_class`
  (защищает ВСЕ типы блоков, включая `wrapper_class` ol); из `image_base_class` убран локальный
  `html_escape` float/align (теперь экранируется один раз на границе → нет двойного экранирования).
- **Миграция single-value атрибутов на `write_attr`:** id/href/target/src/alt/width/height/
  poster/data-lang/title (link, image ×2, video width/height/poster, audio src, icon title,
  source data-lang). Class-циклы по ролям и URL-фрагменты (`#t=`/`start`/`end`) НЕ трогал.
- **Тесты:** `test_attribute_escaping_invariant` (10 каналов: ol-style D7, img-align, section-id,
  icon-role/title, block-id/role, link-url, image-target, video-width) + `_no_overescape`.

### Статус (верифицировано)
- `cargo clippy --workspace`: 0 warnings. `cargo test --workspace`: зелёное
  (parser 429, html lib **300→302**, html_output 35, html_compat 1, adoc_html_tests 6+6,
  integration 25; 0 failed).
- CLI: `[<b>x]\n. one` → `<ol class="&lt;b&gt;x">` (было сырое); `[loweralpha]`/`image align=center`
  неизменно; D1 video-инъекция → `&quot;` без пробоя; img/link байт-в-байт с Asciidoctor.
- Корпус `compare_full.py` (release): **Identical 135 / Different 209 / Errors 0** — baseline без
  изменений (фикс корпус-нейтрален).

### Что дальше
- **Спросить про коммит/мерж/пуш** ветки `fix/attr-escaping-discipline` (по правилу — только по
  запросу).
- Осталось из P2: декомпозиция гигантских функций (`start_tag` ~960 стр., `parse_inline`,
  `scan_next_block`), дедуп `try_*_macro`, doc-тесты публичного API (0), README `233`→`238`,
  сноска FEATURES.md (синтаксис vs HTML-совместимость), метаданные Cargo (license/description/
  repository) + пиннинг semver.
- P3: кластеры совместимости (bare-links class+rel п.14, backslash-entity п.15, типографские
  замены п.37, link-text п.38).

### Предостережения
- НЕ `cargo fmt` на крейт (не fmt-clean). Коммит только по запросу пользователя.
- Верификация совместимости: `compare_full.py` в `/mnt/c/tmp/adoc-test/` (release-бинарь по пути
  `target/release/adoc`), корпус 344 `.adoc`.
- LSP для навигации, context7 MCP для доков.
