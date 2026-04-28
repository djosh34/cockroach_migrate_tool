You are `opencode-go/deepseek-v4-flash`.

You are acting as an outside operator. You may read only the documentation bundle attached to this prompt. Do not read repository code, tests, workflows, scripts, or any project files other than that docs bundle.

Your task:

1. Attempt to set up and operate this project directly from the published Docker images using only the attached documentation.
2. Report the exact points where the docs are sufficient.
3. Report every point where you become blocked, uncertain, or would likely make a wrong move.
4. Treat inline `<comment>...</comment>` blocks as unresolved verifier concerns that weaken trust in nearby instructions.
5. Be strict. Assume the docs are insufficient unless they genuinely let you proceed.

Output format:

- `Verdict:` one sentence
- `What worked:` flat bullet list
- `Blockers:` flat bullet list
- `Ambiguities:` flat bullet list
- `Would you proceed in production?` yes/no with one sentence

Do not propose code changes. Only judge whether the docs stand alone for an outside operator using the published Docker images.
