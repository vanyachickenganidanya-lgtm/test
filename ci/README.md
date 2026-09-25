# CI / GitHub Actions

GitHub **не разрешает** приложениям (в том числе этому агенту) создавать или изменять
файлы в `.github/workflows/` — push отклоняется с ошибкой
`refusing to allow a GitHub App to create or update workflow ... without workflows permission`.

Поэтому готовые workflow лежат здесь. Скопируйте их вручную — два файла, одна команда:

```bash
mkdir -p .github/workflows
cp ci/check.yml   .github/workflows/check.yml
cp ci/release.yml .github/workflows/release.yml
git add .github
git commit -m "ci: checks and release builds"
git push
```

## Что делает каждый файл

| Файл | Триггер | Результат |
|---|---|---|
| `check.yml` | каждый push и PR | `cargo fmt --check`, `cargo clippy`, `cargo build` на Linux, Windows и macOS + проверка целостности `docs/` |
| `release.yml` | тег `v*.*.*` или кнопка *Run workflow* | сборка релизных бинарников для Linux (x86_64), Windows (x86_64), macOS (x86_64 и aarch64) + WASM-бандл; архивы прикладываются к GitHub-релизу |

### Веб-сборка (`game/`)

Джоба `web` собирает `trunk build --release --public-url game/` и выкладывает результат
артефактом `web-build`. Чтобы игра запускалась прямо на сайте, распакуйте артефакт
в `docs/game/`:

```bash
unzip web-build.zip -d docs/game
git add docs/game && git commit -m "web build" && git push
```

Пока папки `docs/game/` нет, сайт сам показывает подсказку и ведёт кнопку
«Играть» на блок загрузок.

## Как выпустить версию

```bash
git tag v0.1.0
git push origin v0.1.0
```

Через ~10–20 минут в разделе *Releases* появятся четыре архива, а страница сайта
(`docs/index.html`) подтянет их автоматически через GitHub API.

## Секреты

Не нужны: `release.yml` использует встроенный `GITHUB_TOKEN`
(в репозитории должно быть разрешено `contents: write` — включено по умолчанию).
