use crate::predicates::Predicate;

use crate::runner_process_support::HostProcessRunner;

impl HostProcessRunner {
    pub(crate) fn assert_exits_failure(mut self, stderr_predicate: impl Predicate<str>) {
        let (stdout, stderr) = self.wait_for_failed_exit_logs();
        assert!(
            stderr_predicate.eval(&stderr),
            "runner stderr did not match expectation\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }

    pub(crate) fn wait_for_failed_exit_logs(&mut self) -> (String, String) {
        for _ in 0..50 {
            if let Some(status) = self
                .child
                .try_wait()
                .expect("runner child status should be readable")
            {
                assert!(
                    !status.success(),
                    "runner unexpectedly exited successfully with status {status}"
                );
                return self.read_logs();
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        panic!("runner stayed up instead of failing during startup");
    }
}
