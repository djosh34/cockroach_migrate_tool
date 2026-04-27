## Task: Install Nix On This VM <status>not_started</status> <passes>false</passes>

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
- [ ] Nix is installed on this VM and is available from a fresh shell used in this repository.
- [ ] Flakes and the required experimental features for this project are enabled and verified with a real Nix command.
- [ ] The installer output and any warnings are reviewed; unresolved warnings or failures are captured in task notes or a follow-up bug/task.
- [ ] Manual verification: `nix --version` succeeds in the project workspace.
- [ ] Manual verification: a flakes-enabled Nix command succeeds in the project workspace.
</acceptance_criteria>
