<div align="center">

# 🌱 Bonzai

### A living bonsai for your terminal.

A tiny persistent terminal companion that grows over real time, remembers where its light came from, reacts to water stress, and can be shaped through pruning.

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

  age 4d   growth 38.2%   light memory 27.4h
  💧 water  ███████░░░ 72%   ☀ light ████████░░ 81% →
  ♥ health █████████░ 94%
```

Bonzai is an experimental terminal bonsai written in Rust. It keeps a compact simulation in the background and reconstructs the tree deterministically from its history.

Water it. Move its light. Prune it. Close the terminal. Go write some code. Come back later.

**The tree keeps living.**

> [!IMPORTANT]
> Bonzai is early alpha software. The core loop works, but the biological model, renderer, pruning system and compatibility surface are still intentionally evolving.

## The idea

Bonzai is not meant to become another productivity tracker.

There are no streaks, points, deadlines, accounts or notifications designed to pull you back in. The goal is closer to a small desk plant: something quiet to check on between long coding sessions.

Open `bonzai watch`, spend thirty seconds moving the light or watering the soil, watch a tiny animation, then return to your editor.

```bash
bonzai watch
```

The viewer can close. The plant does not.

## Features

- 🌱 **Persistent real-time growth** based on elapsed wall-clock time
- 🧠 **Environmental memory** instead of purely instantaneous state
- ☀️ **Directional light history** that produces simple phototropism
- 🌿 **Light-dependent foliage placement** toward the brighter side
- 📏 **Low-light stretching** inspired by etiolation
- 💧 **Drought and overwatering stress** that suppress vigor and foliage
- ✂️ **Persistent pruning pressure** that alters future branching
- 🌳 **Deterministic procedural generation** using branch walkers
- 🎞️ **Tiny terminal animations** for watering, light changes and pruning
- 🎨 **Warm 256-color ANSI palette** designed for a calm terminal view
- ⌨️ **Interactive controls** inside the live viewer
- 💤 **Tiny background daemon** instead of a continuous rendering loop
- 🔌 **Local Unix socket IPC** between the CLI and daemon
- 📁 **XDG-aware storage**
- 🧱 **Zero external Rust dependencies**
- ⚙️ Optional **systemd user service**

## Quick start

### Install

```bash
curl -fsSL https://raw.githubusercontent.com/Nicolas25vlad/bonzai/main/install.sh | bash
```

Make sure `~/.local/bin` is in your `PATH`.

For Fish:

```fish
fish_add_path ~/.local/bin
```

Then:

```bash
bonzai init
bonzai start
bonzai watch
```

Or install and enable the user service in one go:

```bash
git clone https://github.com/Nicolas25vlad/bonzai.git
cd bonzai
./install.sh --systemd
bonzai watch
```

## CLI

Run:

```bash
bonzai help
```

or:

```bash
bonzai --help
```

Available commands:

```text
CARE
  bonzai water
  bonzai light left
  bonzai light center
  bonzai light right
  bonzai prune left
  bonzai prune top
  bonzai prune right

OBSERVE
  bonzai watch
  bonzai show
  bonzai status

LIFECYCLE
  bonzai init
  bonzai start
  bonzai stop
  bonzai reset

INFO
  bonzai help
  bonzai version
```

## Cozy interactive mode

`bonzai watch` is the intended way to spend time with the tree.

```bash
bonzai watch
```

Inside the viewer:

| Key | Action |
| --- | --- |
| `w` | Water the bonsai |
| `r` | Water it with the rain-style animation |
| `a` | Move the light to the left |
| `s` | Center the light |
| `d` | Move the light to the right |
| `j` | Prune the left side |
| `k` | Prune the top |
| `l` | Prune the right side |
| `h` / `?` | Open the in-app help |
| `q` | Leave the viewer |

Actions are animated directly with ANSI frames. There is no game engine and no TUI dependency hiding underneath.

Closing the viewer **does not stop Bonzai**.

## A small biological model

Bonzai does not try to simulate botany at research-grade fidelity. Instead, it borrows a handful of real plant responses and turns them into legible terminal mechanics.

### Phototropism

Plants bias new growth toward useful light. Bonzai approximates this by recording effective light exposure on three axes:

```text
light_left_hours
light_center_hours
light_right_hours
```

The renderer calculates a directional bias from that accumulated history.

Keep the lamp on the right for long enough and the tree does not instantly rotate. Instead, **future growth gradually favors the right side**.

That distinction matters: the current lamp position is a condition; the tree shape is a history.

### Leaf distribution

Foliage walkers receive a probability bias toward the historically brighter side of the crown. A one-minute light change will barely matter. Sustained exposure will.

This creates asymmetric trees naturally instead of choosing from fixed ASCII presets.

### Low light and stretching

When light quality is poor, Bonzai increases vertical spacing and reduces some branching probability. This is a deliberately simplified nod to **etiolation**, where plants in insufficient light often develop elongated growth while seeking better exposure.

### Water stress

Water is not just a health bar.

The simulation accumulates:

```text
drought_stress
wet_stress
```

Long periods of low water reduce vigor, branch frequency and leaf density. Repeated overwatering can also accumulate stress.

The tree is forgiving by design. Bonzai should be a quiet break, not a guilt machine.

### Pruning

Pruning currently works as persistent structural pressure rather than selecting individual branch IDs. Cutting the left, right or crown increases suppression for future walkers in that region.

A future milestone will replace this with branch-addressable pruning.

## How growth is generated

Bonzai borrows the visual idea of procedural branch walkers from [`cbonsai`](https://github.com/jakobrees/cbonsai) and builds a persistent simulation around it.

At render time, a deterministic RNG seeded by the plant's identity walks through a character canvas.

```text
state
  │
  ├── seed
  ├── growth
  ├── health
  ├── water history
  ├── directional light history
  └── pruning history
       │
       ▼
 branch walkers
       │
       ├── phototropic bias
       ├── vigor
       ├── internode spacing
       └── pruning suppression
       │
       ▼
 foliage walkers
       │
       ├── light-side preference
       └── stress-dependent density
       │
       ▼
 ANSI canvas
```

The same state and seed produce the same tree. The tree changes because its state changes, not because opening the program rolls a completely unrelated tree.

## Architecture

Bonzai separates simulation from presentation.

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

The daemon does not need to render frames in the background. Most of the time it sleeps.

When time passes, state evolution is calculated from timestamps. This keeps idle work tiny while preserving the illusion of a plant that continued living while you were away.

## Why a daemon?

The daemon gives Bonzai a single authoritative writer for state and a clean place for future integrations.

It also keeps the CLI simple:

```bash
bonzai water
bonzai status
bonzai watch
```

all talk to the same living plant.

The simulation itself remains timestamp-driven, so Bonzai does not require a high-frequency background loop.

## Why zero dependencies?

Because the constraint is part of the project.

Bonzai currently uses only the Rust standard library for:

- Unix sockets
- filesystem persistence
- timing
- process management
- ANSI rendering
- input handling through `stty`
- deterministic pseudo-random generation

That keeps the dependency graph empty, builds fast, and makes the implementation unusually easy to audit.

If a future feature genuinely earns a dependency, this policy can change. Zero dependencies is a design pressure, not a religion.

## State and files

Bonzai follows XDG paths when available.

Persistent state:

```text
$XDG_DATA_HOME/bonzai/state.txt
```

Fallback:

```text
~/.local/share/bonzai/state.txt
```

Runtime socket and PID data live under:

```text
$XDG_RUNTIME_DIR/bonzai/
```

The state format is intentionally human-readable during alpha development.

## systemd user service

Enable Bonzai when your user session starts:

```bash
./install.sh --systemd
```

Or manually:

```bash
mkdir -p ~/.config/systemd/user
cp systemd/bonzai.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now bonzai.service
```

Check it:

```bash
systemctl --user status bonzai
```

## Compatibility

The current implementation targets Unix-like environments because it uses:

- Unix domain sockets
- `stty`
- ANSI terminal escape sequences

Primary target:

- Linux

Likely workable with small adjustments:

- macOS
- BSDs

Native Windows support is not currently a goal. WSL should be the easiest path on Windows.

## Design principles

### Persistent, not demanding

Bonzai should reward returning without punishing absence.

### Calm by default

The interface uses muted earth, leaf, water and sunlight tones. Animations are intentionally short and low-motion.

### State should matter

A care action should influence future structure, not just increment a number beside an icon.

### Small core

The simulation should remain understandable enough that one person can read it without excavating a framework.

### Terminal-native

Bonzai should feel at home beside a shell, editor or tmux pane instead of imitating a desktop GUI inside text cells.

## Roadmap

### 0.2.x

- [x] Interactive help
- [x] Cozy ANSI palette
- [x] Care animations
- [x] Directional light memory
- [x] Phototropic growth bias
- [x] Water-stress memory
- [x] Stress-dependent foliage
- [ ] Better terminal resize handling
- [ ] Configurable animation speed
- [ ] Reduced-motion mode

### 0.3

- [ ] Persistent branch graph
- [ ] Select individual branches with a cursor
- [ ] Branch-addressable pruning
- [ ] New buds after cuts
- [ ] Better crown balancing

### Later

- [ ] Seasons
- [ ] Soil types
- [ ] Species profiles
- [ ] Weather-inspired ambient events
- [ ] tmux/status-line integrations
- [ ] Shell prompt integration
- [ ] Optional Git activity integration
- [ ] Import/export of plants
- [ ] Multiple bonsai garden

## Development

Clone and run:

```bash
git clone https://github.com/Nicolas25vlad/bonzai.git
cd bonzai
cargo run -- help
```

Useful checks:

```bash
cargo check
cargo test
cargo build --release
```

The GitHub Actions workflow runs the same core validation on pushes and pull requests.

## Contributing

Contributions are welcome, especially around:

- procedural generation
- terminal compatibility
- low-motion animation design
- plant-inspired simulation rules
- state migration
- documentation

Please read [`CONTRIBUTING.md`](CONTRIBUTING.md) before opening a pull request.

For simulation changes, prefer small rules with visible consequences over large amounts of hidden complexity.

## Acknowledgements

Bonzai is visually and algorithmically inspired by [`cbonsai`](https://github.com/jakobrees/cbonsai), the excellent ncurses bonsai generator whose branching approach helped shape this project's procedural renderer.

Bonzai is not a drop-in rewrite of `cbonsai`. Its main focus is persistent state, care mechanics and long-lived growth rather than one-shot tree generation.

Because the project draws from GPL-licensed work and ideas, Bonzai is distributed under **GPL-3.0-or-later**.

## License

GNU General Public License v3.0 or later.

See [`LICENSE`](LICENSE).

---

<div align="center">

**No streaks. No cloud. No notifications. Just a small tree waiting in your terminal.**

`bonzai watch`

</div>
