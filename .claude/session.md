# Session context

## Последняя сессия (2026-05-30, вторая за день)

Фаза 3 (совместимость). Две задачи: inline `[.role]` (закоммичено) и **xref авто-текст**
(на ветке, ожидает коммита).

### Ключевое открытие
`COMPAT-DIFF.md`/`TODO.md` устарели (числа от 2026-03-23). Свежий прогон `compare_full.py`:
старт Identical 71/273. Проверено: **п.40** (атрибуты) и **п.11** (роли блоков) уже
решены/неверно описаны. Доминирующий шум — NCR-кодировка типографики (`’` vs `&#8217;`,
229 файлов; в одиночку 0 flips). Анализ «расстояния до идентичности» дал реальные high-ROI
кластеры.

### Сделано 1: inline `[.role]` на форматировании — КОММИТ `f2dd2eb` (ветка `feat/inline-role-formatting`)
`Tag::Strong/Emphasis/Monospace` → struct `{ id, roles }`; `try_inline_attr_span` обобщён на
`_`/`*`/`` ` `` после `[…]`; рендерер эмитит id/class (`push_inline_id_class`).
`[.path]_x_`→`<em class="path">`, `[.term]*x*`→`<strong class="term">`. Корпус 71→79.

### Сделано 2: xref авто-текст — НЕ ЗАКОММИЧЕНО (ветка `feat/xref-auto-text`, поверх inline-role)
Машинерия была наполовину готова (placeholder `\x00XREF_N\x00` + резолв в `finish()` по
`toc_entries`; весь документ буферизуется → forward-refs работают). Пробелы закрыты в
`adoc-html/src/lib.rs`:
- **inter-doc** `xref:f.adoc[]`: вынес `interdoc_href` (.adoc→.html) из `if`; пустой xref
  теперь fallback-ит на этот путь (был сырой `.adoc`). Арка `Tag::CrossReference` ~1713.
- **intra-doc на блок** `<<id>>`: новое поле `block_ref_titles: Vec<(String,String)>`
  (id→title HTML); захват в `start_tag` сразу после `let meta = self.take_block_meta()`
  (~860) — там доступны и `meta.id`, и `block_title_inner_html` независимо от порядка
  `.Title`/`[#id]`. `finish()` (~718) сливает секции (экранируются) + блоки (уже HTML).
- 2-tuple `xref_placeholders` НЕ менялся (2-й элемент = ключ-и-fallback для intra / .html для inter).

### Текущий статус (верифицировано)
- `cargo build --workspace` + `--release -p adoc-cli`: OK.
- `cargo test --workspace`: ЗЕЛЁНОЕ (428 parser, 297+35 html, 6+6 html-tests, 23 integration,
  ASG, html_compat). `cargo clippy --workspace`: 0 warnings.
- Точечно байт-идентично Asciidoctor: `<a href="#ex-title">Document with a title</a>`,
  nav `xref:f.adoc[]`→текст `f.html`.
- **Корпус: Identical 71→79 (inline-role) →135 (xref). Different 209, Errors 0.**
- Регрессий нет: фиксы меняют вывод только для пустых xref / разметки с `[.role]`.

### Что дальше
- **Спросить про коммит** ветки `feat/xref-auto-text` (стекнута поверх `feat/inline-role-formatting`;
  обе НЕ в master).
- Следующие кластеры: **NCR-кодировка типографики** (`’`→`&#8217;` и т.д.; фон 229 файлов,
  нужно для байт-совместимости), **backslash перед entity** (п.15, ~10 файлов).
- Логи: `/tmp/compat_xref.log` (135), `/tmp/compat_after.log` (79), `/tmp/compat_fresh.log` (71).
  Скрипты анализа: `/tmp/disthist.py`, `/tmp/diffdump.py`.

### Предостережения
- НЕ `cargo fmt` на крейт (не fmt-clean). Коммит только по запросу пользователя.
- Верификация: `asciidoctor -e -o - -a nofooter <f>` vs `target/release/adoc --no-standalone <f>`;
  полный прогон `compare_full.py` (release-бинарь). LSP для навигации, context7 MCP для доков.
