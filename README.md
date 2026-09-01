<div align="center">

# 🌱 Bonzai

### A living bonsai for your terminal.

A tiny persistent terminal companion that grows over real time, reacts to light, needs water, and can be shaped through pruning.

[![CI](https://github.com/Nicolas25vlad/bonzai/actions/workflows/ci.yml/badge.svg)](https://github.com/Nicolas25vlad/bonzai/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)](https://www.rust-lang.org/)
[![Dependencies](https://img.shields.io/badge/dependencies-0-success)](#why-zero-dependencies)
[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](#license)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Unix-lightgrey)](#compatibility)

**Small process. Persistent state. Procedural growth. No framework. No account. No cloud.**

</div>

---

Your terminal already has enough dashboards.

Give it something alive.

```text
                              %%%
                         %%%*%%%%%%
                      %%+%%%%%&%%%%
                         \\ | /
                   %%&%%  \\|/  %%%
                %%%%%%%\\  |  /%%%%%
                       \\ | /
                        \\|/
                         |
                         |
                    .-~~~~-.
                   /        \
                  /__________\
                    \______/

  bonzai v0.1.0   age 3d   growth 31.7%
  💧 ███████░░░ 72%   ☀ ████████░░ 81% →   ♥ █████████░ 94%
```

Bonzai is an experimental terminal bonsai written in Rust. It keeps a small persistent simulation in the background and reconstructs the tree deterministically from its state. Water it, choose where the light comes from, prune it, close the terminal, go write some code, and come back later.

The tree keeps living.

> [!IMPORTANT]
> Bonzai is currently **early alpha software**. The core loop works, but the simulation, rendering, pruning model, compatibility and installation story are still evolving.

## Why Bonzai?

Most terminal toys are ephemeral. They animate while the process is open and disappear when you close the pane.

Bonzai is built around a different idea: **persistence**.

The plant has an age, a seed, health, water, light, growth and pruning history. Its appearance is derived from those values, so interacting with it today changes what you see tomorrow.

That makes Bonzai less of a screensaver and more of a tiny ambient simulation that happens to live next to your editor.

## Features

- 🌱 **Persistent growth** based on real elapsed time
- 💧 **Watering** with a lightweight health model
- ☀️ **Directional light** that influences growth direction
- ✂️ **Pruning** that changes future branch generation
- 🌳 **Procedural tree generation** using deterministic branch walkers
- 🧬 **Stable identity** through a persistent random seed
- 💤 **Tiny background daemon** instead of a continuous physics loop
- 🔌 **Local Unix socket IPC** between the CLI and daemon
- 📁 **XDG-aware storage** for state and runtime files
- 🧱 **Zero external Rust dependencies**
- 🖥️ **ANSI terminal renderer** with no TUI framework
- ⚙️ Optional **systemd user service**

## Quick start

### 1. Install

```bash
curl -fsSL https://raw.githubusercontent.com/Nicolas25vlad/bonzai/main/install.sh | bash
```

Make sure `~/.local/bin` is in your `PATH`.

For Fish:

```fish
fish_add_path ~/.local/bin
```

### 2. Plant your bonsai

```bash
bonzai init
```

### 3. Start the background process

```bash
bonzai start
```

### 4. Watch it grow

```bash
bonzai watch
```

That is it.

## Installation

### Remote installer

```bash
curl -fsSL https://raw.githubusercontent.com/Nicolas25vlad/bonzai/main/install.sh | bash
```

The installer builds an optimized release binary and installs it to:

```text
~/.local/bin/bonzai
```

You can change the prefix:

```bash
BONZAI_PREFIX="$HOME/.local" ./install.sh
```

### From source

Requirements:

- Rust stable
- Cargo
- `stty` for interactive watch mode
- a Unix-like operating system with Unix domain sockets

```bash
git clone https://github.com/Nicolas25vlad/bonzai.git
cd bonzai
./install.sh
```

Or manually:

```bash
cargo build --release
install -Dm755 target/release/bonzai ~/.local/bin/bonzai
```

### systemd user service

Install and enable Bonzai as a user service:

```bash
./install.sh --systemd
```

Or configure it manually:

```bash
mkdir -p ~/.config/systemd/user
cp systemd/bonzai.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now bonzai.service
```

Check it with:

```bash
systemctl --user status bonzai
```

## Usage

```text
bonzai init
bonzai start
bonzai stop
bonzai status
bonzai show
bonzai watch
bonzai water
bonzai light left
bonzai light center
bonzai light right
bonzai prune left
bonzai prune top
bonzai prune right
bonzai reset
```

### Interactive controls

Inside `bonzai watch`:

| Key | Action |
| --- | --- |
| `w` | Water the bonsai |
| `a` | Move light to the left |
| `s` | Center the light |
| `d` | Move light to the right |
| `j` | Prune the left side |
| `k` | Prune the top |
| `l` | Prune the right side |
| `q` | Close the viewer |

Closing the viewer does **not** stop the bonsai.

## How it works

Bonzai deliberately separates the simulation from the renderer.

```text
                     ┌──────────────────┐
                     │    bonzai CLI    │
                     │                  │
                     │ watch / water /  │
                     │ light / prune    │
                     └────────┬─────────┘
                              │
                         Unix socket
                              │
                     ┌────────▼─────────┐
                     │  Bonzai daemon   │
                     │                  │
                     │ authoritative    │
                     │ plant state      │
                     └────────┬─────────┘
                              │
                       persistent state
                              │
                     ┌────────▼─────────┐
                     │ procedural tree │
                     │ reconstruction  │
                     └──────────────────┘
```

The daemon owns a compact state object containing values such as:

```text
seed
born_at
last_tick
water
light
health
growth
light_direction
pruning_history
```

When the viewer asks for a snapshot, the renderer generates the tree from that state.

The same state produces the same tree.

That property matters: the bonsai should evolve, not visually teleport into a completely unrelated tree every time you run the command.

## Growth model

The renderer is inspired by the procedural visual language of [`cbonsai`](https://github.com/jakobrees/cbonsai).

Bonzai uses a walker-based model:

1. A trunk walker starts near the center of the pot.
2. The walker has a finite lifetime.
3. At every step it moves mostly upward.
4. Small random changes alter its horizontal drift.
5. Directional light biases that drift.
6. Mature walkers may spawn shorter branch walkers.
7. Pruning suppresses future branches in selected regions.
8. Foliage walkers build small leaf clouds around living branch tips.

Conceptually:

```text
               branch walker
                    /
             ------*
            /
           /
          *  trunk walker
          |
          |
          |
       ___|___
      /       \
```

The current implementation intentionally favors a compact, readable algorithm over botanical realism.

Bonzai is a game-like ambient simulation, not a horticulture model.

## Time without timers

One of the most important design choices in Bonzai is that biological simulation does **not** require a high-frequency loop.

The state stores the last time it was updated:

```text
last_tick = 18:00 Monday
```

If you open Bonzai again on Wednesday, it computes the elapsed duration and advances the simulation accordingly.

```text
Monday 18:00
     │
     │ terminal closed
     │ laptop suspended
     │ daemon stopped
     │
Wednesday 09:30
     │
     ▼
advance simulation by elapsed time
```

This gives Bonzai persistence without asking the CPU to role-play a greenhouse 24/7.

## Why zero dependencies?

Because Bonzai should be boring to install and cheap to leave running.

The current implementation uses only the Rust standard library for:

- Unix sockets
- process management
- file persistence
- timekeeping
- deterministic pseudo-random generation
- terminal output
- simulation logic

There is no TUI framework, async runtime, serialization framework, database or background service dependency.

This is not a rule carved into stone. If a dependency provides a clear long-term benefit, it can be considered. But the default direction is intentional minimalism.

## Runtime footprint

Bonzai is designed around very small amounts of work:

- the simulation advances from elapsed wall-clock time
- the daemon sleeps between socket checks
- the tree is generated only when it needs to be displayed
- state is a tiny text file
- there are no network requests
- there is no telemetry

Precise memory and CPU benchmarks will be published once the runtime architecture stabilizes enough for those numbers to be meaningful.

## State and XDG paths

Plant state is stored in:

```text
$XDG_DATA_HOME/bonzai/state.txt
```

or, when `XDG_DATA_HOME` is not set:

```text
~/.local/share/bonzai/state.txt
```

Runtime files use:

```text
$XDG_RUNTIME_DIR/bonzai/
├── bonzai.sock
└── bonzai.pid
```

If `XDG_RUNTIME_DIR` is unavailable, Bonzai falls back to a runtime directory under its data directory.

The current state format is intentionally human-readable while the project is young.

## Design principles

Bonzai is small enough that a few principles can meaningfully shape the entire project.

### 1. Persistent, not demanding

The bonsai should reward returning to it without punishing you for having a life.

Neglect should slow growth before it becomes destructive. Bonzai should never turn into a second inbox.

### 2. Shape should carry history

Water and health affect growth. Light influences direction. Pruning changes what is generated later.

The goal is for two long-lived bonsai to become visibly different because their owners treated them differently.

### 3. Idle means idle

Background execution should cost almost nothing when nothing is happening.

### 4. The terminal is the interface

No browser. No account. No tray application. No cloud service required.

### 5. Small core, interesting edges

The simulation core should stay understandable. More elaborate integrations can live around it.

## Compatibility

Current target:

| Environment | Status |
| --- | --- |
| Linux | Primary target |
| Arch Linux / Omarchy | Expected to work |
| systemd user services | Supported |
| Fish / Bash / Zsh | Supported as launch shells |
| tmux | Expected to work |
| Kitty / Ghostty / Alacritty | Expected to work |
| macOS | Experimental / unverified |
| Windows | Not currently supported |

Bonzai currently depends on Unix domain sockets and `stty`, so native Windows support will require an abstraction layer or a separate backend.

## Roadmap

The current implementation proves the basic loop. The interesting work starts from here.

### Near term

- [ ] Branch-level pruning instead of directional pruning
- [ ] Terminal-size-aware rendering
- [ ] Better raw terminal handling
- [ ] Graceful signal handling
- [ ] More robust daemon lifecycle management
- [ ] Config file with sane defaults
- [ ] Simulation tests
- [ ] Renderer snapshot tests
- [ ] Release binaries through GitHub Actions
- [ ] Shell completions
- [ ] Man page

### Simulation

- [ ] Soil moisture and drainage
- [ ] Nutrient model
- [ ] Seasons
- [ ] Dormancy
- [ ] Different bonsai species
- [ ] Branch thickness and aging
- [ ] New shoots
- [ ] Leaf density driven by health
- [ ] Recovery after aggressive pruning
- [ ] Environmental temperature model

### Developer integrations

These are intentionally optional. Your plant should not require Git to exist.

- [ ] Git commit events as subtle growth bonuses
- [ ] Pomodoro integration
- [ ] tmux status integration
- [ ] Starship module
- [ ] shell prompt status
- [ ] local plugin/event interface

### Long term

The long-term vision is a procedural tree whose individual branches persist as entities instead of being reconstructed from aggregate pruning history.

That would enable true branch selection, branch age, scars, branch-specific health, wiring and substantially richer shaping mechanics.

## Project status

Bonzai began as a proof of concept for a simple question:

> What if `cbonsai` were not just something you watched grow once, but something you actually kept alive?

Version `0.1.x` should be treated as experimental. State formats, commands and behavior may change before the first stable release.

If you are trying it today, feedback about terminal compatibility, simulation pacing and rendering quality is especially useful.

## Contributing

Contributions are welcome.

Before opening a large pull request, consider opening an issue first so implementation direction can be discussed without wasting anyone's work.

For setup and contribution guidelines, see [`CONTRIBUTING.md`](CONTRIBUTING.md).

Useful contribution areas include:

- terminal compatibility
- procedural generation
- simulation design
- Rust cleanup
- testing
- documentation
- packaging

Small pull requests are preferred over giant rewrites.

## Inspiration and provenance

Bonzai's visual direction and high-level branch-walker approach are inspired by [`jakobrees/cbonsai`](https://github.com/jakobrees/cbonsai), a terminal bonsai generator written in C.

Bonzai is a separate Rust implementation built around persistent state, care mechanics and background simulation rather than a direct line-by-line port.

The project intentionally acknowledges `cbonsai` because good open-source software should make its lineage obvious.

## Security and privacy

Bonzai is entirely local.

It does not:

- create accounts
- send telemetry
- contact remote services
- upload plant state
- inspect your source code

The daemon communicates with the local CLI through a Unix domain socket.

If future integrations read developer activity, they should remain opt-in and local-first.

## License

Bonzai is distributed under the **GNU General Public License v3.0 or later** (`GPL-3.0-or-later`).

See [`LICENSE`](LICENSE) for the repository license notice and the GNU GPL documentation for the full license terms.

---

<div align="center">

**Grow code. Grow tree.**

`bonzai watch`

</div>
