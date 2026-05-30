# Session context

## Последняя сессия (2026-05-30, вторая за день)

Фаза 3 (совместимость). Реализован кластер **inline `[.role]` на форматировании** (п.13/16).
Работа в ветке `feat/inline-role-formatting` — **НЕ закоммичено** (CLAUDE.md: коммит только
по запросу пользователя; жду явной команды).

### Ключевое открытие сессии
`COMPAT-DIFF.md`/`TODO.md` устарели (числа от 2026-03-23, до Фаз 1–2). Свежий прогон
`compare_full.py`: до фикса Identical 71/Different 273. Проверено: **п.40** (атрибуты) —
рендерер уже резолвит `AttributeReference` из `document_attrs`, описание неверно; **п.11**
(роли блоков) — уже исправлено. Доминирующий шум — NCR-кодировка типографики (`’` vs
`&#8217;`) в 229 файлах (в одиночку 0 flips). Анализ «расстояния до идентичности» дал реальные
high-ROI кластеры. Пользователь выбрал inline `[.role]`.

### Что сделано (в ветке feat/inline-role-formatting)
Проблема: `[.path]_x_` рендерился как литерал `[.path]` + `<em>`. Корень: `try_inline_attr_span`
(`inline.rs:1902`) обрабатывал только `#` после `[…]`; теги Strong/Emphasis/Monospace — unit
без полей.
- **`adoc-parser/src/event.rs`**: `Tag::Strong/Emphasis/Monospace` → struct-варианты
  `{ id: Option<CowStr>, roles: Vec<CowStr> }`. Обновлены `into_static` (клон через `cow_owned`)
  и `to_end` (`{ .. }`).
- **`adoc-parser/src/inline.rs`**: 14 конструирующих мест (6 диспетчер 269/275/298/312/318/324
  + 8 тестовых ассертов) → `{ id: None, roles: Vec::new() }`. `try_inline_attr_span` обобщён:
  marker `#`/`_`/`*`/`` ` `` → InlineSpan/Emphasis/Strong/Monospace с `{id,roles}`; constrained +
  unconstrained; для `` ` `` subs без REPLACEMENTS. Диспетчер `[` (503-505) не менялся (уже
  маршрутизирует `[.`/`[#`).
- **`adoc-html/src/lib.rs`**: новый хелпер `push_inline_id_class<S: AsRef<str>>` (перед
  `write_meta_attrs`); арки `Tag::Strong/Emphasis/Monospace` (1642-1656) эмитят id/class.
- **`adoc-compat-tests/src/builder.rs:241-243`** и **`adoc-parser/tests/integration.rs:169,273`**:
  паттерны `{ .. }` / конструктор с пустыми полями.

### Текущий статус (верифицировано)
- `cargo build --workspace` + `--release -p adoc-cli`: OK.
- `cargo test --workspace`: ЗЕЛЁНОЕ (428 parser, 297+35 html, 6+6 html-tests, 23 integration,
  ASG-пары, html_compat). `cargo clippy --workspace`: 0 warnings.
- Точечные дифы (document-structure, wrap-values, preprocessor): целевые строки **байт-идентичны**
  Asciidoctor (`<em class="path">`, `<strong class="term">`, `<strong id=... class="term">`).
- **Корпус ПОСЛЕ фикса: Identical 71→79, Different 273→265, Errors 0.**
  `attr_diff on <strong>` 20→1, `<em>` 7→2 (остаток — рассинхрон по др. причинам, не class).
- Регрессий нет: для разметки без `[.role]` вывод байт-идентичен прежнему (по построению).

### Что дальше
- **Спросить пользователя про коммит** ветки `feat/inline-role-formatting`.
- Следующие high-ROI кластеры (пользователь сказал «можно комбинировать»):
  1. **xref авто-текст** `<<id>>`/`xref:f.adoc[]` → заголовок/путь цели (~25 файлов в d≤2;
     внутридок. случай требует пред-прохода id→заголовок).
  2. **NCR-кодировка типографики** `’`→`&#8217;` и т.д. (фон в 229 файлах; нужен для
     байт-совместимости; снижает каскадный шум).
  3. **backslash перед entity** (п.15, ~10 файлов).
- Логи прогонов: `/tmp/compat_after.log` (после), `/tmp/compat_fresh.log` (до).
  Скрипты анализа: `/tmp/disthist.py`, `/tmp/diffdump.py` (расстояние/дифы по корпусу).

### Ключевые предостережения (детали в TODO.md)
- НЕ `cargo fmt` на крейт — не fmt-clean.
- Верификация совместимости: `asciidoctor -e -o - -a nofooter <f>` vs
  `target/release/adoc --no-standalone <f>`; полный прогон `compare_full.py` (release-бинарь).
- LSP для навигации (CLAUDE.md), context7 MCP для доков.
