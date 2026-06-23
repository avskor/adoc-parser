# Session context

## Сессия (2026-06-23, 7-я) — F-BJ: авто-ID секции из СУБСТИТУИРОВАННОГО заголовка

Запрос «начни следующую задачу». master `e188b95` (F-BI + doc-поправка байт-паритета смержены).

### Триаж: старые корпуса исчерпаны → расширил frontier на РЕАЛЬНЫЕ доки пользователя
- gate `/mnt/c/tmp/adoc-test/` 344/344; frontier `/mnt/c/tmp/adoc-frontier/` 230/250 (остаток: manpage — другой бэкенд +
  `{asciidoctor-version}`/`{localtime}` env-intrinsics, нерешаемо); adoc2docx 45/52 (остаток — Rouge-подсветка, ×4 файла).
- **Расширил frontier на `/mnt/c/Work/docs/notes/modules/` (81 реальных .adoc, 0 include) → 72 identical, 9 чистых.**
  Метрика — `frontier_parity.py` (семантический DOM, ПРАВИЛЬНАЯ по поправке прошлой сессии: байт только в verbatim).
- Девять расхождений notes (по убыванию diff): plan/qwen/sbertech-index/wsl/keycloak/ansible-tips/synapse/antora-index +
  **fa/index.adoc (1 diff — взято, самое чистое)**. Прочие — отдельные баги (admonition-в-списке, ordered-list десинк,
  ложная Rouge-подсветка `[source,yaml]` без `:source-highlighter:`, и т.д.) — кандидаты на следующие сессии.

### Сделано — F-BJ (ветка `fix/auto-id-substituted-title` от master `e188b95`)
**Баг:** `fa/index.adoc` `== Решение -- {product}` (`:product: FORSed Architect`) → asciidoctor ID
`_решениеforsed_architect`, наш `_решение_forsed_architect`.
**Корень (verified asciidoctor 2.0.23 `section.rb`/`abstract_block.rb`/`rx.rb` + пробы):** `Section.generate_id`
слугифицирует `Section#title` = `apply_title_subs(@title)` — заголовок ПОСЛЕ субституций. `InvalidSectionIdCharsRx`
удаляет entity/теги; спейсед-em-dash replacement ` -- `→`&#8201;&#8212;&#8201;` ПОГЛОЩАЕТ окружающие пробелы → между
«решение» и «forsed» нет разделителя. Мы генерили ID из СЫРОГО заголовка → ` -- ` = разделители → `_`.
**3 паттерна (оба движка):** spaced em-dash `x -- y`→`_xyz`; word em-dash `pre--post`→`_prepost`; ellipsis `a...b`→`_ab`
(+ бонус `(C)`/`(R)` глиф вместо `_c_`).
**Фикс (2 файла, ТОЛЬКО adoc-parser):**
- `block.rs`: новый `fn generate_title_id(&self, title)` = `resolve_title_attr_refs` →
  `crate::inline::apply_typographic_replacements(&t, true, true)` → `scanner::generate_id`. 4 call-сайта переведены
  (doc-header простой + с pre-attrs, section, discrete-heading).
- **Ключ:** наша `apply_typographic_replacements` производит UNICODE-глифы (`\u{2014}`/`\u{2009}` thin-sp/`\u{2026}`/
  `\u{200B}` ZWSP/arrows), НЕ HTML-entity; `generate_id` уже пропускает все не-alnum/не-ASCII-разделители → глифы
  дропаются, спейсед em-dash даёт смежность. **Slugify (scanner.rs) НЕ менялась.**
- `tests.rs` (adoc-html): +1 тест `test_section_id_from_substituted_title_html` (spaced/word em-dash, ellipsis, `(C)`,
  attr+em-dash real-case, regression lone-hyphen `well-known`→`_well_known` / triple `a---b`→`_a_b`).

### Верификация
- clippy `--workspace` **0**. **test --workspace 0 упавших** (html 534→**535**, parser 647, compat 233, render-core 25).
- **БАЙТ-НЕЙТРАЛЬНО на всех старых корпусах** (паттерн em-dash/ellipsis в ЗАГОЛОВКАХ там не встречается):
  - gate 344 — `gate_check.py` **0 diff** vs свежий base `/tmp/adoc_base` (пересобран из master `e188b95`).
  - frontier(250)+adoc2docx(52)=302 — `/tmp/sweep_bvn.py` **0 diff** (new vs base).
- **notes Identical 72→73** (`fa/index.adoc` → identical, выпал из списка расхождений).
- 23 CLI-пробы == asciidoctor 2.0.23 (включая формат-маркеры `*A* -- *B*`→`_ab`, multi `a -- b -- c -- d`→`_abcd`,
  trailing/leading `--`, link-в-title).

### Остаток (отдельный класс, ДЕФЕРНУТ, base==new — НЕ регресс)
**Формат-маркеры в заголовке (QUOTES, не REPLACEMENTS):** `_em_ text`→asciidoctor `_em_text`, наш `__em_text`;
asciidoctor гонит quotes (`<em>`, срезает тег), мы трактуем `_`/`#` как разделитель. Корректный фикс требует quotes-пасс
на title или различение литерального `_` (snake_case) от italic-маркера — риск; не в notes как чистый кейс. Дефернуто.

### Состояние репо
- Ветка `fix/auto-id-substituted-title` от master `e188b95`, НЕ закоммичена (ждёт запроса коммит/merge/push).
- Изменено: `adoc-parser/src/block.rs` (хелпер + 4 call-сайта), `adoc-html/src/tests.rs` (+1 тест). TODO.md (+F-BJ), session.md.
- `/tmp/adoc_base` = свежий бинарь master `e188b95` (АКТУАЛЕН для этой сессии).

### Методология (без изменений)
`frontier_parity.py <roots>` / `showdiff.py <file>` (семантический DOM, ПРАВИЛЬНАЯ метрика для не-verbatim — байт-паритет
только ВНУТРИ `<pre>`, см. [[feedback_html_byte_parity_scope]]). `gate_check.py` (байт gate vs base) + `/tmp/sweep_bvn.py`
(байт frontier+adoc2docx vs base) — регресс-гард. Бинарь: `cargo build --release -p adoc-cli`. asciidoctor 2.0.23 для проб.
**Новый источник реальных корпусов: `/mnt/c/Work/docs/` (notes/modules 81 шт., http-api-design, kubernetes-best-practices,
mgp) — 8 расхождений notes ещё не разобраны.**
