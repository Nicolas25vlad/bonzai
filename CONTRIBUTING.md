# Contributing to Bonzai

Thanks for considering a contribution.

Bonzai is intentionally small, local-first and dependency-light. Contributions are welcome, but changes should preserve those qualities unless there is a strong reason not to.

## Development setup

You need a recent stable Rust toolchain and a Unix-like environment.

```bash
git clone https://github.com/Nicolas25vlad/bonzai.git
cd bonzai
cargo check
cargo test
cargo run -- --help
```

To try the interactive viewer:

```bash
cargo run -- init
cargo run -- start
cargo run -- watch
```

Stop the daemon when you are done:

```bash
cargo run -- stop
```

## Before opening a pull request

Please make sure:

```bash
cargo check
cargo test
cargo build --release
```

If your toolchain includes them, also run:

```bash
cargo fmt --all
cargo clippy --all-targets
```

## Project direction

Changes should generally reinforce these principles:

- persistent, not demanding
- local-first and private by default
- near-zero idle work
- deterministic visual evolution where practical
- small and understandable core logic
- terminal-native interaction
- optional integrations rather than mandatory ecosystems

## Dependencies

Bonzai currently has zero external Rust dependencies.

This is intentional, but not dogmatic. If you want to introduce a dependency, explain what it buys the project and why implementing or maintaining the equivalent functionality locally would be worse.

## Pull requests

Prefer focused pull requests.

A good pull request usually does one thing well, explains the behavior change, and includes tests when the affected code can be tested reasonably.

For large features or architectural changes, open an issue first.

## Bugs

A useful bug report includes:

- operating system and version
- terminal emulator
- shell
- Rust version if building from source
- exact command run
- expected behavior
- actual behavior
- relevant error output

Please avoid including secrets, private paths or unrelated environment data.

## Procedural generation changes

Changes to the renderer can have large visual effects from very small code changes. When modifying the generation algorithm, describe:

- what visual property changes
- whether the same seed remains deterministic
- how light and pruning interact with the new behavior
- whether old saved state still behaves sensibly

## Simulation changes

Simulation values are game mechanics, not botanical claims. Keep them understandable and forgiving. Bonzai should encourage occasional interaction without becoming another obligation.

## License

By contributing, you agree that your contribution may be distributed under the project's GPL-3.0-or-later license.
