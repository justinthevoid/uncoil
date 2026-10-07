# AGENTS.md

Instructions for AI coding agents live in [CLAUDE.md](CLAUDE.md). They apply to every agent, not only
Claude. Read it before changing anything.

The short version:

- Commands: the full list is under "Commands" in CLAUDE.md (fmt, clippy with `-D warnings` and tests for
  every crate, and `pnpm --dir apps/uncoil check`); the GUI crate needs `pnpm --dir apps/uncoil build` first.
- **Never write a device's onboard memory without the user's explicit consent for that write.** Always
  restore normal mode after driver mode. Stop `uncoild` before running raw HID probes.
- Never commit keystroke captures, Synapse logs, serial numbers or usernames.
- Never imply Razer affiliation. Voice and design rules: `PRODUCT.md`, `.impeccable/surfaces/`.
- Commit only when asked, with a `Co-Authored-By:` trailer.
