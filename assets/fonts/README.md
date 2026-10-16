# Fonts

Both fonts are compiled into `calculator.exe` with `include_bytes!`, so the app needs no font
files next to it.

| File | What | Source | License |
| --- | --- | --- | --- |
| `CalcCraftIcons.ttf` | Every icon of the app | A subset of [Fluent System Icons](https://github.com/microsoft/fluentui-system-icons) (`FluentSystemIcons-Resizable.ttf`, commit `a347af9`), built by `tools/icon-font.py` | MIT, [`LICENSE-FluentSystemIcons.txt`](LICENSE-FluentSystemIcons.txt) |
| `Inter.ttf` | The text font where Segoe UI is missing (other systems, or a Windows without it) | [Inter](https://fonts.google.com/specimen/Inter) 4.001, `Inter[opsz,wght].ttf` from [google/fonts](https://github.com/google/fonts/tree/main/ofl/inter), unmodified | SIL Open Font License 1.1, [`OFL-Inter.txt`](OFL-Inter.txt) |

`tools/icon-font.py` puts each icon at the code point of the Segoe Fluent Icons glyph that the
Windows Calculator uses for it (`src/app/icons.rs` lists them), scales it to fill the em like
Segoe Fluent Icons, and draws `÷`, `=` and the Angle converter's `∧`, which Fluent System Icons
doesn't have, from Fluent's minus stroke. To add an icon, add it to the tool's `ICONS` and run:

```
uv run tools/icon-font.py
```
