<div align="center">

# Bonzai

### A persistent bonsai that lives in your terminal.

Bonzai is a lightweight terminal companion written in Rust. It grows over real time, remembers its environment, reacts to water and light, and can be shaped through pruning.

[![CI](https://github.com/Nicolas25vlad/bonzai/actions/workflows/ci.yml/badge.svg)](https://github.com/Nicolas25vlad/bonzai/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)](https://www.rust-lang.org/)
[![Dependencies](https://img.shields.io/badge/dependencies-0-success)](#zero-dependency-core)
[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](#license)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Unix-lightgrey)](#compatibility)

**Persistent state · Procedural growth · Environmental memory · Zero external Rust dependencies**

</div>

---

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
  water   ███████░░░ 72%   light  ████████░░ 81% →
  health  █████████░ 94%
```

Most terminal toys disappear when their process exits.

Bonzai is built around the opposite idea: **the plant persists**.

Water it, move its light source, prune it, close the terminal, spend a few hours coding, and come back later. Its state and growth history are still there.

> [!NOTE]
> Bonzai is currently **early alpha software**. The core interaction loop is functional, while the biological model, renderer, pruning system and compatibility layer are still evolving.

## Overview

Bonzai combines a small persistent simulation with deterministic procedural rendering.

The project is intentionally designed around a few constraints:

- remain lightweight enough to run quietly in the background;
- make care decisions influence future structure, not only UI counters;
- keep the simulation understandable and auditable;
- feel native inside a shell, editor or tmux workflow;
- avoid turning a calm terminal toy into another productivity tracker.

There are no accounts, streaks, notifications or cloud services.

## Highlights

- **Persistent real-time simulation** based on elapsed wall-clock time
- **Environmental memory** for directional light and water stress
- **Phototropic growth bias** toward historically stronger light
- **Stress-aware foliage density** under drought or overwatering
- **Low-light stretching** inspired by plant etiolation
- **Persistent pruning pressure** that changes future branching
- **Deterministic procedural generation** using branch and foliage walkers
- **Interactive terminal mode** with direct keyboard controls
- **Short ANSI animations** for watering, lighting and pruning actions
- **Warm 256-color palette** designed for low-distraction terminal use
- **Unix socket IPC** between the CLI and background daemon
- **XDG-aware state and runtime paths**
- **Zero external Rust dependencies**
- Optional **systemd user service**

## Quick start

### Install

```bash
curl -fsSL https://raw.githubusercontent.com/Nicolas25vlad/bonzai/main/install.sh | bash
```

Ensure `~/.local/bin` is available in your `PATH`.

For Fish:

```fish
fish_add_path ~/.local/bin
```

### Plant and start

```bash
bonzai init
bonzai start
bonzai watch
```

For automatic startup with your user session:

```bash
git clone https://github.com/Nicolas25vlad/bonzai.git
cd bonzai
./install.sh --systemd
bonzai watch
```

## Usage

```text
bonzai <command> [options]
```

Run the built-in command reference at any time:

```bash
bonzai help
```

### Care

```bash
bonzai water
bonzai light left
bonzai light center
bonzai light right
bonzai prune left
bonzai prune top
bonzai prune right
```

### Observe

```bash
bonzai watch
bonzai show
bonzai status
```

### Lifecycle

```bash
bonzai init
bonzai start
bonzai stop
bonzai reset
```

### Information

```bash
bonzai help
bonzai version
```

## Interactive mode

`bonzai watch` opens the live terminal view.

```bash
bonzai watch
```

| Key | Action |
| --- | --- |
| `w` | Water the bonsai |
| `r` | Water with the rain animation |
| `a` | Move the light left |
| `s` | Center the light |
| `d` | Move the light right |
| `j` | Prune the left side |
| `k` | Prune the crown |
| `l` | Prune the right side |
| `h` / `?` | Open the in-app help |
| `q` | Leave the viewer |

Actions are rendered as small ANSI animations. Closing the viewer does **not** stop the simulation.

## Simulation model

Bonzai is not intended to be a scientific plant simulator. It uses a small set of real biological ideas as understandable gameplay rules.

### Environmental memory

The plant does not react only to its current values. It also records accumulated environmental exposure.

```text
light_left_hours
light_center_hours
light_right_hours

drought_stress
wet_stress
```

This means a temporary light change has little effect, while sustained conditions gradually influence future structure.

### Phototropism

Directional light history is converted into a growth bias.

If the right side remains brighter for a meaningful amount of time, newly generated branches and foliage become increasingly likely to favor that side.

The current lamp position is a condition. **The shape of the tree is a history.**

### Low-light response

Low light reduces branching density and increases vertical spacing between growth steps.

This is a simplified approximation of etiolation: plants under insufficient light often stretch while searching for better exposure.

### Water stress

Water is treated as more than a single health meter.

Extended drought and repeated overwatering accumulate separate stress values. Those values influence vigor, branch frequency, foliage density and growth rate.

The system is deliberately forgiving. Bonzai should reward attention without punishing absence.

### Pruning

The current pruning model records structural pressure for the left side, crown and right side of the plant. Future branch walkers are suppressed according to that history.

Branch-addressable pruning is planned for the persistent branch-graph milestone.

## Procedural growth

Bonzai uses deterministic walkers to construct the visible tree from persistent state.

```text
persistent state
      │
      ├── seed
      ├── age
      ├── growth
      ├── health
      ├── water stress
      ├── directional light history
      └── pruning history
              │
              ▼
        branch walkers
              │
              ├── directional bias
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

The same seed and state produce the same tree.

Opening Bonzai does not roll a completely unrelated plant. The visible structure changes because the underlying state changes.

## Architecture

The simulation and presentation layers are intentionally separated.

```text
┌────────────────────┐
│     bonzai CLI     │
│                    │
│ watch · water      │
│ light · prune      │
└─────────┬──────────┘
          │
      Unix socket
          │
┌─────────▼──────────┐
│   Bonzai daemon    │
│                    │
│ authoritative      │
│ plant state        │
└─────────┬──────────┘
          │
   persistent state
          │
┌─────────▼──────────┐
│ procedural renderer│
│                    │
│ branch + foliage   │
│ reconstruction     │
└────────────────────┘
```

The daemon is the single writer for plant state. It does not continuously render or run a high-frequency physics loop.

Most of the time it sleeps.

State progression is timestamp-driven, which allows Bonzai to account for elapsed time without spending CPU cycles simulating every second that passed.

## Zero-dependency core

Bonzai currently uses only the Rust standard library for:

- Unix domain sockets
- filesystem persistence
- timekeeping
- process management
- deterministic pseudo-random generation
- ANSI rendering
- terminal input through `stty`

There is no TUI framework or async runtime in the dependency graph.

This is a design constraint, not a permanent rule. A future dependency should solve a real problem that justifies its cost.

## State and runtime files

Bonzai follows XDG conventions when available.

Persistent state:

```text
$XDG_DATA_HOME/bonzai/state.txt
```

Fallback:

```text
~/.local/share/bonzai/state.txt
```

Runtime files:

```text
$XDG_RUNTIME_DIR/bonzai/
```

The state format is intentionally human-readable during early development.

## systemd user service

Install and enable the service:

```bash
./install.sh --systemd
```

Manual setup:

```bash
mkdir -p ~/.config/systemd/user
cp systemd/bonzai.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now bonzai.service
```

Inspect the service:

```bash
systemctl --user status bonzai
```

## Compatibility

Bonzai currently targets Unix-like systems because it depends on:

- Unix domain sockets
- `stty`
- ANSI terminal escape sequences

| Platform | Status |
| --- | --- |
| Linux | Primary target |
| macOS | Expected to require minor compatibility work |
| BSD | Expected to require minor compatibility work |
| WSL | Recommended Windows path |
| Native Windows | Not currently targeted |

## Design goals

### Persistent, not demanding

The plant should continue existing without becoming an obligation.

### Calm by default

Colors are muted, animations are short, and interaction is intentionally low-motion.

### State has visible consequences

Care actions should influence future growth rather than only modify numbers in a status panel.

### Small, inspectable core

The complete simulation should remain understandable without requiring a large framework or dependency graph.

### Terminal-native interaction

Bonzai should feel comfortable beside a shell, editor or tmux pane instead of imitating a desktop application inside a terminal.

## Roadmap

### 0.2.x · Interaction and environmental growth

- [x] In-app help
- [x] Cozy ANSI palette
- [x] Care animations
- [x] Directional light memory
- [x] Phototropic growth bias
- [x] Drought and overwatering memory
- [x] Stress-aware foliage density
- [ ] Better terminal resize handling
- [ ] Configurable animation speed
- [ ] Reduced-motion mode

### 0.3 · Persistent structure

- [ ] Persistent branch graph
- [ ] Stable branch identifiers
- [ ] Cursor-based branch selection
- [ ] Branch-addressable pruning
- [ ] Bud generation after cuts
- [ ] Improved crown balancing

### Future

- [ ] Seasons
- [ ] Soil profiles
- [ ] Species profiles
- [ ] Ambient weather events
- [ ] tmux and status-line integrations
- [ ] Shell prompt integration
- [ ] Optional Git activity integration
- [ ] Plant import/export
- [ ] Multi-tree gardens

## Development

Clone the repository:

```bash
git clone https://github.com/Nicolas25vlad/bonzai.git
cd bonzai
```

Run locally:

```bash
cargo run -- help
```

Validation:

```bash
cargo check --locked
cargo test --locked
cargo build --release --locked
```

GitHub Actions runs the same core validation on pushes and pull requests.

## Contributing

Contributions are welcome, particularly in:

- procedural generation
- plant-inspired simulation rules
- terminal compatibility
- low-motion animation design
- state migration
- documentation

Please read [`CONTRIBUTING.md`](CONTRIBUTING.md) before opening a pull request.

For simulation changes, prefer **small rules with visible consequences** over large amounts of hidden complexity.

## Credits

Bonzai is visually and algorithmically inspired by [`cbonsai`](https://github.com/jakobrees/cbonsai), an ncurses bonsai generator whose procedural branching approach helped shape the renderer used in this project.

Bonzai extends that idea in a different direction, focusing on persistent state, environmental history, care mechanics and long-lived growth rather than one-shot tree generation.

## License

Bonzai is distributed under the **GNU General Public License v3.0 or later**.

See [`LICENSE`](LICENSE) for the full license text.

---

<div align="center">

**No streaks. No cloud. No notifications. Just a small tree waiting in your terminal.**

</div>
