package verifyservice

import (
	"fmt"

	serviceconfig "github.com/cockroachdb/molt/verifyservice"
	"github.com/spf13/cobra"
)

func Command() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "verify-service",
		Short: "Commands for the dedicated verify service.",
		Long:  "Commands for validating and running the dedicated verify service configuration.",
	}
	cmd.AddCommand(validateConfigCommand())
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
