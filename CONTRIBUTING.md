# Contributing to Omnarc

Thank you for helping. Omnarc is developed in the open; everything — code, comments, docs and commit messages —
is written in English. The product itself is translated (Uzbek, Russian and more).

## Where things are

| Path | What | Check it with |
|---|---|---|
| `protocol/` | Protobuf messages and the tensor frame spec | `buf lint` (run inside `protocol/`) |
| `node/` | Rust workspace: protocol, engine host, node agent | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace` |
| `engine/` | C++20 engine on llama.cpp (submodule + `engine/patches`) with a C ABI | `git submodule update --init`, then `cmake -S engine -B build/engine && cmake --build build/engine && ctest --test-dir build/engine` |
| `design/` | tokens, the living ring, the design concept | `python3 design/colors.py && python3 design/build.py` — generated files must be committed |
| `web/` | Svelte 5 component library with Storybook | `pnpm install`, `pnpm check`, `pnpm test`, `pnpm build-storybook`, `pnpm test:visual` (inside `web/`) |

If you have [just](https://github.com/casey/just) installed, `just check` runs everything.

## Quality gates

A pull request is merged only when CI is green on every platform (Linux, Windows, macOS, Android, iOS, web).
CI runs exactly the checks above, plus:

- **Contrast gate** — `design/colors.py` fails if any text color drops below 4.5:1 or a graphic color below 3:1.
- **Visual regression and accessibility** — every Storybook story is rendered in both themes, compared with its
  snapshot, and checked with axe (WCAG 2.2 AA). Zero violations are allowed.
- **No silent skips** — a skipped test counts as a failure.

To update snapshots after an intended visual change, run `pnpm test:visual --update-snapshots` in
`web/packages/ui`, look at every changed image, and commit them with the change.

## Design rules

- Use tokens only (`design/tokens.css`); never hard-code a color.
- Font sizes come from the scale: 11, 12, 13, 14, 16, 20, 28, 40, 56.
- One accent color — turquoise. Status colors always appear with an icon and a label, never alone, and text never
  wears a status color.
- Animate only `transform` and `opacity`; respect `prefers-reduced-motion`.

## Commits and pull requests

- Keep commits focused; write the subject in the imperative ("Add tensor frame decoder").
- One stage of the roadmap (`PLAN.md` §11) is delivered through one or more pull requests, each with its gate.
- By contributing you agree that your work is licensed under the Apache License 2.0.
