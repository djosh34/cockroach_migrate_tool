---
name: info-about-repo
description: Use when the user asks any question about some repository, repo, Git repository, GitHub repository, or external codebase.
---

1. Check `.ralph/local_repos/<repo>/`.
   - If it exists, investigate there.

2. If `.ralph/local_repos/<repo>/` does not exist:
   - Find/Search repo_url based on user request
   - use git clone <repo url> inside .ralph/local_repos
   - Example: `git clone <repo-url> .ralph/local_repos/<repo>`
   - Find git tags and pick latest and checkout into that (not master, but latest git tagged version)
   - continue investigate
