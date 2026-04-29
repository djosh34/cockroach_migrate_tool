package verifyservice

import (
	"fmt"
	"io"
	"sort"
	"strings"
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
			resolvedDatabases, err := cfg.Verify.ResolveAllDatabases()
			if err != nil {
				return writeJSONCommandError(logger, logFormat, err)
			}
			sourceSummary := summarizeSSLModeInventory(resolvedDatabases, func(pair serviceconfig.ResolvedDatabasePair) string {
				return pair.Source.SSLMode
			})
			destinationSummary := summarizeSSLModeInventory(resolvedDatabases, func(pair serviceconfig.ResolvedDatabasePair) string {
				return pair.Destination.SSLMode
			})

			if logFormat == logFormatJSON {
				logger.Info().
					Str("event", "config.validated").
					Str("listener_mode", cfg.Listener.Mode()).
					Int("database_count", len(resolvedDatabases)).
					Str("source_sslmode", sourceSummary).
					Str("destination_sslmode", destinationSummary).
					Msg("verify-service config validated")
				return nil
			}

			_, err = fmt.Fprintf(
				cmd.OutOrStdout(),
				"verify-service config is valid\nlistener mode: %s\ndatabase count: %d\nsource sslmode: %s\ndestination sslmode: %s\n",
				cfg.Listener.Mode(),
				len(resolvedDatabases),
				sourceSummary,
				destinationSummary,
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

func summarizeSSLModeInventory(
	databases []serviceconfig.ResolvedDatabasePair,
	selectMode func(serviceconfig.ResolvedDatabasePair) string,
) string {
	if len(databases) == 0 {
		return summarizeSSLMode("")
	}

	modes := make(map[string]struct{}, len(databases))
	for _, database := range databases {
		modes[summarizeSSLMode(selectMode(database))] = struct{}{}
	}

	orderedModes := make([]string, 0, len(modes))
	for mode := range modes {
		orderedModes = append(orderedModes, mode)
	}
	sort.Strings(orderedModes)
	if len(orderedModes) == 1 {
		return orderedModes[0]
	}
	return "mixed(" + strings.Join(orderedModes, ",") + ")"
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
		event := logger.Error().Str("event", "command.failed")
		if opErr, ok := serviceconfig.ExtractOperatorError(err); ok {
			event = event.
				Str("category", opErr.Category).
				Str("code", opErr.Code)
			if len(opErr.Details) > 0 {
				event = event.Any("details", opErr.Details)
			}
			event.Msg(opErr.Message)
		} else {
			event.Msg(err.Error())
		}
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
