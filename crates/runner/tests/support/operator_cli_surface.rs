#![allow(dead_code)]

pub struct OperatorCliSurface {
    id: &'static str,
    allowed_actions: &'static [&'static str],
    max_action_depth: usize,
    root_required_markers: &'static [&'static str],
    root_forbidden_markers: &'static [&'static str],
    command_help_contracts: &'static [OperatorCommandHelp],
}

pub struct OperatorCommandHelp {
    action: &'static str,
    path: &'static [&'static str],
    required_markers: &'static [&'static str],
    forbidden_markers: &'static [&'static str],
}

const SETUP_SQL_COMMAND_HELP_CONTRACTS: &[OperatorCommandHelp] = &[
    OperatorCommandHelp {
        action: "emit-cockroach-sql",
        path: &["emit-cockroach-sql"],
        required_markers: &[
            "Usage: setup-sql emit-cockroach-sql [OPTIONS] --config <CONFIG>",
            "--config <CONFIG>",
            "--log-format <LOG_FORMAT>",
            "--format <FORMAT>",
        ],
        forbidden_markers: &[
            "SUBCOMMAND",
            "render-bootstrap-sql",
            "render-postgres-setup",
        ],
    },
    OperatorCommandHelp {
        action: "emit-postgres-grants",
        path: &["emit-postgres-grants"],
        required_markers: &[
            "Usage: setup-sql emit-postgres-grants [OPTIONS] --config <CONFIG>",
            "--config <CONFIG>",
            "--log-format <LOG_FORMAT>",
            "--format <FORMAT>",
        ],
        forbidden_markers: &[
            "SUBCOMMAND",
            "render-bootstrap-sql",
            "render-postgres-setup",
        ],
    },
];

const RUNNER_COMMAND_HELP_CONTRACTS: &[OperatorCommandHelp] = &[
    OperatorCommandHelp {
        action: "validate-config",
        path: &["validate-config"],
        required_markers: &[
            "Usage: runner validate-config [OPTIONS] --config <CONFIG>",
            "--config <CONFIG>",
            "--deep",
            "--log-format <LOG_FORMAT>",
        ],
        forbidden_markers: &[
            "SUBCOMMAND",
            "--source-url",
            "--target-url",
            "--cockroach-schema",
            "verify",
        ],
    },
    OperatorCommandHelp {
        action: "run",
        path: &["run"],
        required_markers: &[
            "Usage: runner run [OPTIONS] --config <CONFIG>",
            "--config <CONFIG>",
            "--log-format <LOG_FORMAT>",
        ],
        forbidden_markers: &[
            "SUBCOMMAND",
            "--source-url",
            "--target-url",
            "--cockroach-schema",
            "verify",
        ],
    },
];

const DOCUMENTED_SURFACES: &[OperatorCliSurface] = &[
    OperatorCliSurface {
        id: "setup-sql",
        allowed_actions: &["emit-cockroach-sql", "emit-postgres-grants"],
        max_action_depth: 1,
        root_required_markers: &[
            "Usage: setup-sql [OPTIONS] <COMMAND>",
            "emit-cockroach-sql",
            "emit-postgres-grants",
            "--log-format <LOG_FORMAT>",
        ],
        root_forbidden_markers: &["render-bootstrap-sql", "render-postgres-setup", "\n  run"],
        command_help_contracts: SETUP_SQL_COMMAND_HELP_CONTRACTS,
    },
    OperatorCliSurface {
        id: "runner",
        allowed_actions: &["validate-config", "run"],
        max_action_depth: 1,
        root_required_markers: &[
            "Usage: runner [OPTIONS] <COMMAND>",
            "validate-config",
            "run",
            "--log-format <LOG_FORMAT>",
        ],
        root_forbidden_markers: &[
            "compare-schema",
            "render-helper-plan",
            "render-postgres-setup",
            "cutover-readiness",
            "--source-url",
            "--cockroach-schema",
            "--allow-tls-mode-disable",
        ],
        command_help_contracts: RUNNER_COMMAND_HELP_CONTRACTS,
    },
    OperatorCliSurface {
        id: "verify-service-image",
        allowed_actions: &["run"],
        max_action_depth: 0,
        root_required_markers: &[
            "Run the dedicated verify-service HTTP API.",
            "--config string",
            "--log-format string",
        ],
        root_forbidden_markers: &[
            "validate-config",
            "--rows",
            "--table-splits",
            "--source",
            "--target",
            "render-postgres-setup",
        ],
        command_help_contracts: &[],
    },
];

impl OperatorCliSurface {
    pub fn documented() -> &'static [Self] {
        DOCUMENTED_SURFACES
    }

    pub fn setup_sql() -> &'static Self {
        Self::find("setup-sql")
    }

    pub fn runner() -> &'static Self {
        Self::find("runner")
    }

    pub fn verify_service_image() -> &'static Self {
        Self::find("verify-service-image")
    }

    pub fn id(&self) -> &'static str {
        self.id
    }

    pub fn allowed_actions(&self) -> &'static [&'static str] {
        self.allowed_actions
    }

    pub fn max_action_depth(&self) -> usize {
        self.max_action_depth
    }

    pub fn assert_root_help_output(&self, help_output: &str) {
        assert_contains_all(
            help_output,
            self.root_required_markers,
            &format!("{} root help", self.id),
        );
        assert_excludes_all(
            help_output,
            self.root_forbidden_markers,
            &format!("{} root help", self.id),
        );
    }

    pub fn command_help(&self, action: &str) -> &'static OperatorCommandHelp {
        self.command_help_contracts
            .iter()
            .find(|contract| contract.action == action)
            .unwrap_or_else(|| {
                panic!(
                    "operator CLI surface `{}` must define a help contract for action `{action}`",
                    self.id
                )
            })
    }

    fn find(id: &str) -> &'static Self {
        Self::documented()
            .iter()
            .find(|surface| surface.id == id)
            .unwrap_or_else(|| panic!("operator CLI surface contract must define `{id}`"))
    }
}

impl OperatorCommandHelp {
    pub fn path_with_help_flag(&self) -> Vec<&'static str> {
        self.path
            .iter()
            .copied()
            .chain(std::iter::once("--help"))
            .collect()
    }

    pub fn assert_help_output(&self, help_output: &str) {
        assert_contains_all(
            help_output,
            self.required_markers,
            &format!("operator help for `{}`", self.action),
        );
        assert_excludes_all(
            help_output,
            self.forbidden_markers,
            &format!("operator help for `{}`", self.action),
        );
    }
}

fn assert_contains_all(text: &str, required_markers: &[&str], context: &str) {
    for marker in required_markers {
        assert!(
            text.contains(marker),
            "{context} must include `{marker}`, got: {text}",
        );
    }
}

fn assert_excludes_all(text: &str, forbidden_markers: &[&str], context: &str) {
    for marker in forbidden_markers {
        assert!(
            !text.contains(marker),
            "{context} must not include `{marker}`, got: {text}",
        );
    }
}
