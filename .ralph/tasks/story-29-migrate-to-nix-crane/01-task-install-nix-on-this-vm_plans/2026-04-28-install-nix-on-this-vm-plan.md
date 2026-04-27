# Plan: Install Nix On This VM

## References

- Task:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/01-task-install-nix-on-this-vm.md`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`
- Current repo workflow to re-validate after VM setup:
  - `Makefile`
- Current shell bootstrap surfaces that fresh-shell validation must respect:
  - `~/.bash_profile`
  - `~/.bashrc`
- Official Nix references checked on 2026-04-28:
  - `https://nixos.org/download/`
  - `https://nix.dev/manual/nix/2.32/installation/installing-binary.html`
  - `https://nix.dev/manual/nix/2.32/command-ref/conf-file.html`
  - `https://nix.dev/manual/nix/2.32/development/experimental-features.html`

## Planning Assumptions

- This turn started with no linked plan artifact, so it is a planning turn that must stop after the plan is linked.
- This is a non-code environment/bootstrap task, so the TDD exception applies.
  - Execution should still follow the `tdd` mindset of small honest verification slices, but not by inventing brittle repo tests for shell profile text or installer script contents.
  - The real checks here are executable shell validations plus the required repo validation lanes after Nix is installed.
- Current VM facts gathered during planning:
  - AlmaLinux 9.7 on `aarch64`
  - `systemd` is PID 1
  - `sudo` is available without a password prompt
  - SELinux is `Enforcing`
  - `/nix` is currently absent
  - `nix` is currently not installed
- The current official Nix manual says multi-user install is for Linux with systemd and without SELinux, while Linux with SELinux should use single-user install.
  - Because this VM is SELinux-enforcing, execution should use the official single-user path `--no-daemon`, not `--daemon`.
- If the official single-user installer fails in a way that requires unsupported SELinux hacks, disabling SELinux, or hidden error-swallowing, execution must switch this plan back to `TO BE VERIFIED` and file a bug instead of pretending the VM is ready.
- No backwards compatibility is required.
  - Do not preserve parallel non-Nix bootstrap crutches in repo files just to mask a host setup problem.

## Approval And Verification Priorities

- Highest-priority outcomes:
  - `nix --version` works from a fresh shell in this repository.
  - a flakes-enabled Nix command works from a fresh shell in this repository.
  - the chosen install path is explicit about why SELinux forced single-user install.
  - installer warnings and unresolved caveats are captured in the task notes, not ignored.
  - `make check`, `make lint`, and `make test` still pass after Nix is installed.
- Lower-priority outcomes:
  - pinning a specific Nix version in the installer URL
  - adding any repo-level documentation before task 02 establishes the canonical flake workflow

## Current State Summary

- The repository currently has no `flake.nix`, `flake.lock`, or other Nix source files.
- The current developer workflow is still driven by `Makefile`:
  - `make check`
  - `make lint`
  - `make test`
- The task is therefore pure VM/user bootstrap:
  - install Nix
  - enable the required experimental features
  - prove fresh-shell availability
  - prove the existing repo still passes its current required lanes

## Improve-Code-Boundaries Focus

- Primary boundary smell for this task:
  - host bootstrap concerns can easily leak into repo-owned workflow glue.
- Required execution stance:
  - keep Nix installation and feature enablement at the VM/user config boundary
  - do not add temporary repo wrapper scripts, fake Make targets, or command-by-command `--extra-experimental-features` hacks just to hide an incomplete install
  - keep VM-specific assumptions and installer warnings in task notes, not smeared through product code or contributor docs prematurely
- Preferred boundary shape after execution:
  - Nix is available because the VM/user environment is correctly configured
  - later story tasks can add repo Nix files cleanly without also owning shell bootstrapping hacks

## Intended Files And Surfaces To Change During Execution

- Repo-tracked:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/01-task-install-nix-on-this-vm.md`
    - add execution notes, warning summary, prerequisite summary, acceptance checkmarks, and final pass state
- VM/user config, not repo-tracked:
  - `/nix`
  - `~/.nix-profile`
  - installer-managed shell hooks
  - `~/.config/nix/nix.conf` or the equivalent user-level Nix config file actually read by the installed client
- Explicitly avoid changing unless execution proves strictly necessary:
  - `README.md`
  - `Makefile`
  - repo source files

## Execution Slices

### Slice 1: Preflight And Task Notes

- [x] Reconfirm the planning facts immediately before installation:
  - `getenforce`
  - `id`
  - `/etc/os-release`
  - `command -v nix` should fail
- [x] Add task notes capturing the VM prerequisites and the reason the plan chose single-user installation on this AlmaLinux VM.
- [x] If the VM state materially differs from planning assumptions, switch this plan back to `TO BE VERIFIED` before installing anything.

### Slice 2: Official Nix Installation

- [x] Run the official single-user installer:
  - `bash <(curl -L https://nixos.org/nix/install) --no-daemon`
- [x] Review the installer output carefully.
  - Capture any warning, caveat, or manual follow-up in the task notes.
  - Do not ignore profile-loading or `/nix` ownership messages.
- [x] If the installer exits non-zero or leaves a half-installed state, stop and capture the real failure instead of patching around it silently.

### Slice 3: Fresh-Shell Availability

- [x] Verify the installed profile integration in a fresh login shell, not just the current process:
  - `bash -lc 'command -v nix && nix --version'`
- [x] If a fresh shell does not find `nix`, fix the real shell-init boundary once, then re-run the same fresh-shell check.
- [x] Do not depend on ad hoc `source .../nix.sh` commands in the final verification path.

### Slice 4: Enable Required Experimental Features

- [x] Configure the user-level Nix config actually loaded by the installed client.
- [x] Set the required features explicitly:
  - `experimental-features = nix-command flakes`
- [x] Prefer editing one real config file over passing per-command flags.
- [x] Verify the configuration is loaded in a fresh shell:
  - `bash -lc 'nix show-config | rg \"^experimental-features = .*nix-command.*flakes\"'`
- [x] If the installed client wants `extra-experimental-features` instead, record the exact reason and resulting file contents in task notes instead of guessing.

### Slice 5: Real Flakes Command

- [x] Run a real flakes-enabled command from the project workspace in a fresh shell.
- [x] Because the repo does not yet contain a flake, use a command that still exercises flakes honestly from this workspace, for example:
  - `bash -lc 'cd /home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool && nix flake metadata github:NixOS/nixpkgs'`
- [x] Capture the exact command and whether it succeeded in the task notes.
- [x] If flakes still require per-command flags, the install/config boundary is incomplete and execution must keep fixing that instead of accepting it.

### Slice 6: Required Repo Validation

- [x] Run `make check`
- [x] Run `make lint`
- [x] Run `make test`
- [x] Do not run `make test-long` for this task.
- [x] If any lane fails, treat it as real fallout from the environment change or latent repo issues and resolve or document it honestly before marking the task complete.

### Slice 7: Completion And Boundary Review

- [x] Update the task markdown with:
  - installation mode chosen and why
  - Nix version actually installed
  - whether installer warnings were resolved or remain as follow-up concerns
  - exact fresh-shell verification commands used
  - exact flakes command used
- [x] Tick all acceptance boxes that are genuinely complete.
- [x] Do one final `improve-code-boundaries` pass:
  - confirm this task did not push VM bootstrap hacks into repo workflow files
  - confirm later Nix/crane tasks can build on a clean VM-level Nix install instead of a repo-local workaround

## Expected Outcome

- This VM has an officially installed Nix client that works from a fresh shell.
- Flakes and `nix-command` are enabled through real Nix config, not ad hoc command flags.
- The repo remains unchanged except for task notes because task 01 is an environment bootstrap boundary, not the place to start the flake migration itself.
- The next story task can focus on adding `flake.nix` and crane-based workflows without also solving shell bootstrap.

Plan path: `.ralph/tasks/story-29-migrate-to-nix-crane/01-task-install-nix-on-this-vm_plans/2026-04-28-install-nix-on-this-vm-plan.md`

NOW EXECUTE
