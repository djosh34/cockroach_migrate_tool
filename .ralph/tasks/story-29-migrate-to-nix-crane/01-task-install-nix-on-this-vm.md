## Task: Install Nix On This VM <status>done</status> <passes>true</passes>

<description>
**Goal:** Install Nix on this VM so this repository can be built, tested, and developed through a reproducible Nix toolchain. The higher order goal is to prepare the local execution environment for a full migration to a Nix + crane setup without relying on host-installed language, Docker, or Make tooling.

In scope:
- install Nix on the VM used by the agents for this project
- enable the Nix features needed by flakes and modern Nix development
- verify that `nix --version` works in a fresh shell
- verify that `nix flake --version` or an equivalent flakes-enabled command works
- document any VM-level assumptions or prerequisites directly in the task notes while executing the task
- avoid silently ignoring installer warnings or errors

Out of scope:
- migrating the repository build itself to Nix
- changing CI, Docker image generation, or developer fallback workflows
- adding repository source files unless they are strictly needed to document the VM installation outcome

Decisions already made:
- this is an environment setup task requested as part of the larger Nix migration story
- errors must not be swallowed or ignored
- if the VM cannot support Nix installation, fail this task and file a bug/task describing the blocker rather than pretending the setup is complete

</description>


<acceptance_criteria>
- [x] Nix is installed on this VM and is available from a fresh shell used in this repository.
- [x] Flakes and the required experimental features for this project are enabled and verified with a real Nix command.
- [x] The installer output and any warnings are reviewed; unresolved warnings or failures are captured in task notes or a follow-up bug/task.
- [x] Manual verification: `nix --version` succeeds in the project workspace.
- [x] Manual verification: a flakes-enabled Nix command succeeds in the project workspace.
</acceptance_criteria>

<plan>.ralph/tasks/story-29-migrate-to-nix-crane/01-task-install-nix-on-this-vm_plans/2026-04-28-install-nix-on-this-vm-plan.md</plan>

<execution_notes>
- Preflight on 2026-04-28:
  - `getenforce` -> `Enforcing`
  - `id` -> `uid=501(joshazimullah) gid=1000(joshazimullah)`
  - `/etc/os-release` -> `AlmaLinux 9.7 (Moss Jungle Cat)` on this VM
  - `command -v nix` -> not found
  - `/nix` -> absent before installation
  - `~/.config/nix/nix.conf` -> absent before installation
- Installation mode decision:
  - Use the official single-user installer with `--no-daemon`.
  - Reason: this VM is Linux with SELinux enforcing, so the plan intentionally avoids the multi-user daemon path and keeps the bootstrap at the user/VM boundary.
- Fresh-shell boundary before installation:
  - `~/.bash_profile` sources `~/.bashrc` and `~/.cargo/env`.
  - `~/.bashrc` has no existing Nix init block.
- Installer execution:
  - Ran `bash <(curl -L https://nixos.org/nix/install) --no-daemon`
  - Installed version: `nix (Nix) 2.34.6`
  - Installer-created shell hooks:
    - appended `if [ -e /home/joshazimullah.linux/.nix-profile/etc/profile.d/nix.sh ]; then . /home/joshazimullah.linux/.nix-profile/etc/profile.d/nix.sh; fi # added by Nix installer` to `~/.bash_profile`
    - appended the same Nix init hook to `~/.zshrc`
  - Installer output reviewed:
    - informational note: multi-user install is possible, but not chosen because SELinux is enforcing
    - post-install reminder to re-login or source `~/.nix-profile/etc/profile.d/nix.sh`
  - Unresolved installer warnings: none
- User-level Nix config:
  - Created `~/.config/nix/nix.conf`
  - Contents: `experimental-features = nix-command flakes`
- Fresh-shell verification commands:
  - `bash -lc 'command -v nix && nix --version'`
  - Result: `/home/joshazimullah.linux/.nix-profile/bin/nix` and `nix (Nix) 2.34.6`
  - `bash -lc 'nix config show | rg "^experimental-features = .*flakes.*nix-command|^experimental-features = .*nix-command.*flakes"'`
  - Result: config shows `experimental-features = fetch-tree flakes nix-command`
- Flakes verification command:
  - `bash -lc 'cd /home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool && nix flake metadata github:NixOS/nixpkgs'`
  - Result: succeeded and resolved `github:NixOS/nixpkgs` to revision `5b3c8ba23e3ff3576393ada226a470928b141676`
- Required repo validation after installation:
  - `make check` -> passed
  - `make lint` -> passed
  - `make test` -> passed
  - `make test` evidence included the Rust workspace suites plus `go test ./cmd/verifyservice -count=1`
- Improve-code-boundaries review:
  - No repo workflow files were changed to hide an incomplete host install.
  - The only repo-tracked changes for this task are the Ralph task and plan artifacts; Nix bootstrap remained at the VM/user config boundary where it belongs.
</execution_notes>
