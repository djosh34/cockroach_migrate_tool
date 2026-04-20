package verifyservice

import (
	"fmt"
	"io"
	"time"

	serviceconfig "github.com/cockroachdb/molt/verifyservice"
	"github.com/rs/zerolog"
	"github.com/spf13/cobra"
)

const (
	logFormatText = "text"
	logFormatJSON = "json"
)

func Command() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "verify-service",
		Short: "Commands for the dedicated verify service.",
		Long:  "Commands for validating and running the dedicated verify service configuration.",
	}
	cmd.AddCommand(validateConfigCommand())
	cmd.AddCommand(runCommand())
	return cmd
}

func validateConfigCommand() *cobra.Command {
	var configPath string
	var logFormat string

	cmd := &cobra.Command{
		Use:           "validate-config --config <path>",
		Short:         "Validate the dedicated verify-service config file.",
		Args:          cobra.NoArgs,
		SilenceErrors: true,
		SilenceUsage:  true,
		RunE: func(cmd *cobra.Command, args []string) error {
			logger, err := newCommandLogger(cmd.ErrOrStderr(), logFormat)
			if err != nil {
				return err
			}
			cfg, err := serviceconfig.LoadConfig(configPath)
			if err != nil {
				return writeJSONCommandError(logger, logFormat, err)
			}

			if logFormat == logFormatJSON {
				logger.Info().
					Str("event", "config.validated").
					Str("listener_mode", cfg.Listener.Mode()).
					Str("source_sslmode", summarizeSSLMode(cfg.Verify.Source.SSLMode())).
					Str("destination_sslmode", summarizeSSLMode(cfg.Verify.Destination.SSLMode())).
					Msg("verify-service config validated")
				return nil
			}

			_, err = fmt.Fprintf(
				cmd.OutOrStdout(),
				"verify-service config is valid\nlistener mode: %s\nsource sslmode: %s\ndestination sslmode: %s\n",
				cfg.Listener.Mode(),
				summarizeSSLMode(cfg.Verify.Source.SSLMode()),
				summarizeSSLMode(cfg.Verify.Destination.SSLMode()),
			)
			return err
		},
	}
	cmd.Flags().StringVar(&configPath, "config", "", "Path to the verify-service config file.")
	registerLogFormatFlag(cmd, &logFormat)
	if err := cmd.MarkFlagRequired("config"); err != nil {
		panic(err)
	}
	return cmd
}

func runCommand() *cobra.Command {
	var configPath string
	var logFormat string

	cmd := &cobra.Command{
		Use:           "run --config <path>",
		Short:         "Run the dedicated verify-service HTTP API.",
		Args:          cobra.NoArgs,
		SilenceErrors: true,
		SilenceUsage:  true,
		RunE: func(cmd *cobra.Command, args []string) error {
			logger, err := newCommandLogger(cmd.ErrOrStderr(), logFormat)
			if err != nil {
				return err
			}
			cfg, err := serviceconfig.LoadConfig(configPath)
			if err != nil {
				return writeJSONCommandError(logger, logFormat, err)
			}
			if logFormat == logFormatJSON {
				logger.Info().Str("event", "runtime.starting").Msg("verify-service runtime starting")
			}
			err = serviceconfig.Run(cmd.Context(), cfg, serviceconfig.RuntimeDependencies{
				Logger: logger,
			})
			if err != nil {
				return writeJSONCommandError(logger, logFormat, err)
			}
			return nil
		},
	}
	cmd.Flags().StringVar(&configPath, "config", "", "Path to the verify-service config file.")
	registerLogFormatFlag(cmd, &logFormat)
	if err := cmd.MarkFlagRequired("config"); err != nil {
		panic(err)
	}
	return cmd
}

func registerLogFormatFlag(cmd *cobra.Command, target *string) {
	cmd.Flags().StringVar(target, "log-format", logFormatText, "Operator log format: text or json.")
}

func summarizeSSLMode(mode string) string {
	if mode == "" {
		return "default"
	}
	return mode
}

func newCommandLogger(w io.Writer, logFormat string) (zerolog.Logger, error) {
	if logFormat != logFormatText && logFormat != logFormatJSON {
		return zerolog.Logger{}, fmt.Errorf("invalid --log-format %q: must be one of %s, %s", logFormat, logFormatText, logFormatJSON)
	}

	zerolog.TimestampFieldName = "timestamp"
	zerolog.TimeFieldFormat = time.RFC3339Nano

	var writer io.Writer = w
	if logFormat == logFormatText {
		writer = zerolog.NewConsoleWriter(func(console *zerolog.ConsoleWriter) {
			console.Out = w
			console.TimeFormat = time.RFC3339
		})
	}
	return zerolog.New(writer).With().Timestamp().Str("service", "verify").Logger(), nil
}

func writeJSONCommandError(logger zerolog.Logger, logFormat string, err error) error {
	if logFormat == logFormatJSON {
		logger.Error().Str("event", "command.failed").Msg(err.Error())
		return loggedCommandError{cause: err}
	}
	return err
}

type loggedCommandError struct {
	cause error
}

func (e loggedCommandError) Error() string {
	return e.cause.Error()
}

func IsLoggedCommandError(err error) bool {
	_, ok := err.(loggedCommandError)
	return ok
}
