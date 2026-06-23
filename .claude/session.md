# Session context

## Сессия (2026-06-23) — F-BD: именованные атрибуты ссылок `id=`/`title=` на `<a>` (adoc2docx links.adoc корень B)

Запрос «начни следующую задачу». master `1c85cd2` (F-BC смержен; прошлый session.md устарел — описывал F-BC как
«не закоммичено», фактически смержено `49252b7`/`1c85cd2`). Триаж: adoc2docx ближайший clean-divergence —
`links.adoc` (62 позиц. diff). Разбор: позиционный differ раздут (после первого десинка @[58] всё смещено на 1).
**Два реальных корня:**
- **Корень A** (строки 17, 19): `[.role]#text#` (атрибутированный span) внутри метки ссылки. Сканер метки макроса
  закрывает label на ПЕРВОМ `]` (после `[.overline`). asciidoctor гонит quotes ДО macros → `[.overline]#overline#`
  становится `<span>` раньше, чем link-макрос хватает метку. У нас порядок macros-ПЕРЕД-quotes (зеркало legacy) →
  баг воспроизводится. **Архитектурный, НЕ взят** — см. остаток.
- **Корень B** (строка 30): именованные `id=`/`title=` ссылки НЕ рендерились на `<a>`. **СДЕЛАН.**

Ветка `fix/link-label-span-and-named-attrs` (от master `1c85cd2`, НЕ закоммичена — паттерн F-*: коммит ПО ЗАПРОСУ).

### Сделано — корень B (links.adoc: строка 30 → байт-в-байт)
Изучено правило asciidoctor (html5.rb `convert_inline_anchor` тип `:link` + `append_link_constraint_attrs`):
порядок атрибутов `<a>` = **href, id, class(role), title, target, rel**. Эмпирически подтверждено на link:/bare-URL/mailto.
- **attributes.rs** `LinkAttrs`: +поля `id`/`title` (`Option<&str>`); `parse_link_attrs` ловит `"id"`/`"title"` в named-арме.
- **event.rs** `Tag::Link`: +поля `id`/`title` (`Option<CowStr>`) в def + clone-impl (`into_owned`).
- **adoc-html/events.rs** рендер `Tag::Link`: `id` (после href, до class) + `title` (после class, до target) через
  `write_attr` + `resolve_inline_attr_value` (как role/href — macros-до-attributes оставляет `{attr}` литералом).
- **adoc-parser/subst/macros.rs** (новый движок, дефолт): `build_link` +параметры `id`/`title`; проводка в `try_link`,
  `try_mailto`, autolink-`url[text]` (из `attrs.id/title`); 3 bare-сайта (`<url>`/bare/email) → None. Sentinel-guard
  расширен на id/title (verbatim-хранение → punt при сентинеле, как role/window).
- **adoc-parser/inline.rs** (legacy fallback): 4 link_attrs-сайта проводят id/title; bare/autolink/email → None;
  24+2 тест-сайта → `id: None, title: None`. **subst/mod.rs**: 8 тест-сайтов; **adoc-html/tests.rs**: +1 тест.

### Верификация
- clippy `--workspace` 0 (3 warning'а только под `--all-targets` — пред-существующие в тестах, есть на master).
- **test --workspace: 0 упавших** (html 528→**529** +test_link_id_and_title_attrs_html; parser 645, compat 233, html-compat ok).
- **Гейт 344/344 байт-в-байт** vs master `1c85cd2` (база `/tmp/adoc_base` = свежий master-бинарь; gate_check.py 0 diff).
  Ни один гейт-файл не использует link id/title.
- **Sweep base-vs-new (frontier 250 + adoc2docx 52 = 304): РОВНО 1 изменённый** — links.adoc (целевой). 0 регрессий.
- links.adoc строка 30: `<a href=... id="home" title="Project home page">Home</a>` — байт-в-байт с asciidoctor 2.0.23.
  CLI-пробы (link:/bare-URL/mailto/named-only-bare с id/title) все совпали.

### Состояние репо
- Ветка `fix/link-label-span-and-named-attrs` (от master `1c85cd2`, НЕ закоммичена). master чист == origin.
- Изменены: adoc-parser/{attributes.rs, event.rs, subst/macros.rs, subst/mod.rs(тесты), inline.rs},
  adoc-html/{src/events.rs, src/tests.rs(+1 тест)}.

### Остаток / следующая работа
- **links.adoc корень A** — `[.role]#text#` в метке ссылки (строки 17, 19). Архитектурный: macros-до-quotes vs
  asciidoctor quotes-до-macros. Правило фикса ПОНЯТО и подтверждено на asciidoctor: при поиске закрывающего `]`
  метки link/url-макроса ПРОПУСКАТЬ `]`, который (а) закрывает ВНУТРЕННИЙ `[…]` (есть unescaped `[` строго между
  открывающей скобкой макроса и кандидатом) И (б) сразу за ним идёт constrained/unconstrained quote-маркер,
  формирующий ВАЛИДНЫЙ span. Граничные случаи проверены: `[a [b] c]`→close на первом `]` (нет маркера);
  `[label]*next*`→close на `]` (нет ВНУТРЕННЕГО `[`); `[a [.role]#span# b]`→skip→финальный `]`. Требует детекции
  валидности спана (quotes.rs `constrained_open_close`/`simple_pair_open_close` уже `pub(super)`) на сыром src в
  macros.rs — отдельный осторожный инкремент (затрагивает критичное сканирование скобок макроса). Флипнет links.adoc → Identical.
- **Крупные adoc2docx** (НЕ триажены, вероятно мульти-root): test 1105, source 681, xml 291, callouts 195.
- frontier single-diffs архитектурны (CHANGELOG replacements-before-macros, migration {asciidoctor-version} intrinsic).
- Методология: `frontier_parity.py /mnt/c/tmp/adoc2docx`, `showdiff.py <file>`, gate_check.py (база `/tmp/adoc_base`),
  base-vs-new sweep (inline в bash: find frontier+adoc2docx, diff base vs new). Бинарь: `cargo build --release -p adoc-cli`.
  asciidoctor 2.0.23 gem: `/usr/share/rubygems-integration/all/gems/asciidoctor-2.0.23/lib/asciidoctor/converter/html5.rb`.
