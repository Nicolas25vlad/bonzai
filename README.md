<div align="center">

# Bonzai

### A persistent bonsai that lives in your terminal.

Bonzai is a lightweight terminal companion written in Rust. It grows over real time, remembers its environment, reacts to water and light, and can be shaped through pruning.

[![CI](https://github.com/Nicolas25vlad/bonzai/actions/workflows/ci.yml/badge.svg)](https://github.com/Nicolas25vlad/bonzai/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)](https://www.rust-lang.org/)
[![Dependencies](https://img.shields.io/badge/Rust_dependencies-0-success)](#small-by-design)
[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](#credits--license)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Unix-lightgrey)](#compatibility)

**Persistent state · Procedural growth · Environmental memory · Zero external Rust dependencies**

</div>

---

```text
                         &&&&&&&&
                    &&&&&&&&&&&&&&
                 &&&&&&&     &&&&&&
                    \\|       |/
            &&&&&    \\|_____/    &&&&
               \\_____\|/  &&&&&&
                       /|\\
                 _____/ | \\_
                /       |   \
                       /~
                       \\|

                 .-----------------.
                  \\               /
                   \\_____________/
                   (_)         (_)

      water ━━━━━━━─── 72%   light ━━━━━━━━── 81% →   health ━━━━━━━━━─ 94%
```

Most terminal toys disappear when their process exits. **Bonzai persists.**

Water it, move its light, prune it, close the terminal, spend a few hours coding, and come back later. The same tree is still there, carrying the history of how it was treated.

> [!NOTE]
> Bonzai is early alpha software. The core loop is usable, tested and intentionally small, but the growth model and pruning system are still evolving.

## Quick start

Install the latest version:

```bash
curl -fsSL https://raw.githubusercontent.com/Nicolas25vlad/bonzai/main/install.sh | bash
```

If `~/.local/bin` is not already in your `PATH`, Fish users can add it with:

```fish
fish_add_path ~/.local/bin
```

Plant a tree and open the live view:

```bash
bonzai init
bonzai start
bonzai watch
```

Future updates are built in:

```bash
bonzai update
```

## Why Bonzai

| | |
| --- | --- |
| **Persistent** | Growth is based on elapsed real time, not how long the viewer stays open. |
| **Environmental** | Directional light, drought and overwatering leave memory that influences future growth. |
| **Procedural** | The visible tree is reconstructed deterministically from its persistent state. |
| **Terminal-native** | It is designed to sit beside your shell, editor or tmux session rather than imitate a desktop app. |
| **Local** | No account, cloud service, telemetry or network daemon is required. |
| **Small** | The Rust core has zero external crate dependencies. |

No streaks. No notifications. No productivity guilt. It is just a small tree that waits for you.

## Care for the tree

The same actions are available from the shell and from `bonzai watch`.

| Action | CLI | Live key |
| --- | --- | :---: |
| Water | `bonzai water` | `w` / `r` |
| Light from the left | `bonzai light left` | `a` |
| Light from above | `bonzai light center` | `s` |
| Light from the right | `bonzai light right` | `d` |
| Prune left | `bonzai prune left` | `j` |
| Prune crown | `bonzai prune top` | `k` |
| Prune right | `bonzai prune right` | `l` |
| Help | `bonzai help` | `h` / `?` |
| Leave the viewer | | `q` |

Care animations happen inside the existing scene. The viewer does not switch to a separate animation screen.

## Growth with memory

Bonzai is not a scientific plant simulator. It borrows a few plant behaviors and turns them into readable rules:

```text
          elapsed time + care history
                     │
         ┌───────────┼───────────┐
         │           │           │
       water       light       pruning
         │           │           │
         └───────────┼───────────┘
                     ▼
              persistent state
                     │
                     ▼
              branch growth
                     │
                     ▼
               foliage pads
                     │
                     ▼
               terminal tree
```

Keep the light on one side and new growth gradually favors it. Leave the tree in low light and shoots stretch more. Drought and repeated overwatering reduce vigor. Pruning suppresses future growth in the affected region.

The current environment is a condition. **The shape of the tree is a history.**

## Small by design

Bonzai uses a tiny local architecture:

```text
┌──────────────┐       Unix socket       ┌──────────────┐
│ CLI / viewer │ ◄─────────────────────► │    daemon    │
└──────┬───────┘                         └──────┬───────┘
       │                                      │
       │ local render                         │ authoritative state
       ▼                                      ▼
  ANSI terminal                         XDG state file
```

The daemon owns mutable plant state and spends most of its time sleeping. The viewer keeps a local snapshot, synchronizes periodically, and renders locally instead of hammering the daemon on every frame.

`bonzai watch` uses the terminal alternate screen and repaints the existing frame in place. Short care effects update the same scene instead of clearing and rebuilding the entire terminal between animation frames.

The core currently relies only on Rust's standard library for Unix sockets, persistence, timekeeping, process management, deterministic pseudo-random generation and ANSI output.

## Commands

```text
bonzai init
bonzai start
bonzai stop
bonzai watch
bonzai show
bonzai status
bonzai water
bonzai light left|center|right
bonzai prune left|top|right
bonzai reset
bonzai update
bonzai help
bonzai --version
```

For automatic startup with your user session:

```bash
git clone https://github.com/Nicolas25vlad/bonzai.git
cd bonzai
./install.sh --systemd
```

## Quality

Every push and pull request runs the same quality gate:

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

The current suite includes **10 model/renderer unit tests** and **5 end-to-end CLI/daemon regression tests** covering state migration, deterministic rendering, simulation bounds, watering, directional light, pruning, persistence and daemon survival under repeated actions.

## Project status

The next major milestone is persistent branch structure. Today the tree is deterministically reconstructed from state; the longer-term goal is to give individual branches stable identities so pruning can operate on the actual visible branch rather than on a regional pressure value.

Near-term work includes:

- persistent branch graph and stable branch IDs
- cursor-based, branch-addressable pruning
- better terminal resize handling
- richer crown balancing and growth silhouettes
- optional seasons and species profiles

## Development

```bash
git clone https://github.com/Nicolas25vlad/bonzai.git
cd bonzai
cargo run -- watch
```

Before opening a pull request:

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for contribution guidelines and [`SECURITY.md`](SECURITY.md) for security reporting.

## Compatibility

| Platform | Status |
| --- | --- |
| Linux | Primary target |
| WSL | Recommended Windows path |
| macOS | Expected to work with minor compatibility caveats |
| BSD | Expected to work with minor compatibility caveats |
| Native Windows | Not currently targeted |

Bonzai currently depends on Unix domain sockets, ANSI terminal support and `stty`.

## Credits & license

Bonzai is strongly inspired by [`cbonsai`](https://github.com/jakobrees/cbonsai), particularly its terminal-first visual language and procedural branching approach. Bonzai takes that idea in a different direction by focusing on a persistent tree, environmental history and interactive care.

Bonzai is distributed under **GPL-3.0-or-later**. See [`LICENSE`](LICENSE) for the project licensing notice and GPL reference.

---

<div align="center">

**A quiet little tree for the space between commits.**

</div>
