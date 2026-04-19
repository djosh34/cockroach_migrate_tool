package verifyservice

import (
	"fmt"
	"io"

	serviceconfig "github.com/cockroachdb/molt/verifyservice"
	"github.com/rs/zerolog"
	"github.com/spf13/cobra"
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

	cmd := &cobra.Command{
		Use:   "validate-config --config <path>",
		Short: "Validate the dedicated verify-service config file.",
		Args:  cobra.NoArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			cfg, err := serviceconfig.LoadConfig(configPath)
			if err != nil {
				return err
			}

			_, err = fmt.Fprintf(
				cmd.OutOrStdout(),
				"verify-service config is valid\nlistener transport: %s\nlistener client auth: %s\nsource tls mode: %s\ndestination tls mode: %s\n",
				cfg.Listener.Transport.Mode,
				cfg.Listener.TLS.ClientAuth.Mode,
				cfg.Verify.Source.TLS.Mode,
				cfg.Verify.Destination.TLS.Mode,
			)
			if err != nil {
				return err
			}
			if warning := cfg.DirectServiceAuthWarning(); warning != "" {
				_, err = fmt.Fprintf(cmd.OutOrStdout(), "%s\n", warning)
			}
			return err
		},
	}
	cmd.Flags().StringVar(&configPath, "config", "", "Path to the verify-service config file.")
	if err := cmd.MarkFlagRequired("config"); err != nil {
		panic(err)
	}
	return cmd
}

func runCommand() *cobra.Command {
	var configPath string

	cmd := &cobra.Command{
		Use:   "run --config <path>",
		Short: "Run the dedicated verify-service HTTP API.",
		Args:  cobra.NoArgs,
		RunE: func(cmd *cobra.Command, args []string) error {
			cfg, err := serviceconfig.LoadConfig(configPath)
			if err != nil {
				return err
			}
			if warning := cfg.DirectServiceAuthWarning(); warning != "" {
				if _, err := fmt.Fprintf(cmd.ErrOrStderr(), "%s\n", warning); err != nil {
					return err
				}
			}
			return serviceconfig.Run(cmd.Context(), cfg, serviceconfig.RuntimeDependencies{
				Logger: newCommandLogger(cmd.ErrOrStderr()),
			})
		},
	}
	cmd.Flags().StringVar(&configPath, "config", "", "Path to the verify-service config file.")
	if err := cmd.MarkFlagRequired("config"); err != nil {
		panic(err)
	}
	return cmd
}

func newCommandLogger(w io.Writer) zerolog.Logger {
	return zerolog.New(w).With().Timestamp().Logger()
}
