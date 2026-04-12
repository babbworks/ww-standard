# Workwarrior Documentation

Workwarrior is a profile-based productivity system that unifies TaskWarrior, TimeWarrior, JRNL, and Hledger under a single CLI and browser UI.

→ [Why Workwarrior](whyworkwarrior.md)

---

## Guides

| Page | Description |
|------|-------------|
| [Getting Started](guides/getting-started.md) | Install, first profile, basic usage |
| [Installation](guides/install.md) | Platform notes, dependency management |
| [Profiles](guides/profiles.md) | Isolation, resources, UDAs, backup, removal |
| [Commands](guides/commands.md) | Full command surface, shell functions |
| [Usage Examples](guides/usage-examples.md) | Practical workflows |
| [Browser UI](guides/browser.md) | Web interface, CMD input, panels, API |
| [Heuristic Engine](guides/heuristics.md) | Natural language, 627 rules, self-improvement |
| [Weapons](guides/weapons.md) | Gun, Sword, Next, Schedule |
| [Services](guides/services.md) | All 20+ service domains |
| [Architecture](guides/architecture.md) | Directory structure, env vars, internals |

## GitHub Sync

| Page | Description |
|------|-------------|
| [User Guide](guides/github-sync-guide.md) | Two-way sync walkthrough |
| [Configuration](guides/github-sync-configuration.md) | Setup and config reference |
| [Troubleshooting](guides/github-sync-troubleshooting.md) | Common issues and fixes |
| [Issues Service](guides/issues-troubleshooting.md) | Bugwarrior debugging |

## Search

| Tool | Guide |
|------|-------|
| Tasks | [guides/search/task.md](guides/search/task.md) |
| Time | [guides/search/time.md](guides/search/time.md) |
| Journals | [guides/search/journal.md](guides/search/journal.md) |
| Ledgers | [guides/search/ledger.md](guides/search/ledger.md) |
| Lists | [guides/search/list.md](guides/search/list.md) |

## Development

| Page | Description |
|------|-------------|
| [Service Development](guides/service-development.md) | Build and register services |
| [Testing Guide](guides/testing-guide.md) | Manual testing procedures |
| [Release Checklist](guides/release-checklist.md) | Production readiness gates |

## Technical Reference

Per-component documentation for every library, service, and subsystem.

| Section | Contents |
|---------|----------|
| [reference/bin/](reference/bin/) | ww dispatcher, ww-init.sh bootstrap |
| [reference/lib/](reference/lib/) | 17 core libraries (profile-manager, sync engine, logging, etc.) |
| [reference/services/](reference/services/) | Service docs (github-sync, UDA, urgency, questions, groups, models, etc.) |
| [reference/cross-cutting/](reference/cross-cutting/) | Sync engine overview, conflict resolver, annotation sync, installer, config loader |
| [reference/extensions/](reference/extensions/) | TaskWarrior extension registry (150+ repos) and TimeWarrior extensions |
| [reference/source-map.yaml](reference/source-map.yaml) | Maps each doc to its source files |
