package fetch

import (
	"context"
	"fmt"
	"net/url"
	"strings"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/cmd/fetch/tokens"
	"github.com/cockroachdb/molt/cmd/internal/cmdutil"
	"github.com/cockroachdb/molt/compression"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/fetch"
	"github.com/cockroachdb/molt/fetch/datablobstorage"
	"github.com/cockroachdb/molt/fetch/fetchmetrics"
	"github.com/cockroachdb/molt/moltlogger"
	"github.com/cockroachdb/molt/testutils"
	"github.com/cockroachdb/molt/utils"
	"github.com/rs/zerolog"
	"github.com/spf13/cobra"
	"github.com/thediveo/enumflag/v2"
)

// TODO: update with mysql docs links for these cases.
const mysqlDocsLink = "https://www.cockroachlabs.com/docs/stable/molt-fetch"

type TableHandlingOption enumflag.Flag

func Command() *cobra.Command {
	const (
		fetchID              = "fetch-id"
		continuationToken    = "continuation-token"
		continuationFileName = "continuation-file-name"
	)

	const (
		// None means we will start ingesting into the target db without
		// affecting the existing data.
		None TableHandlingOption = iota
		// DropOnTargetAndRecreate means we will drop the tables with matching
		// names if they exist and automatically recreate it on the target side.
		// This is also the entrypoint for the schema creation functionality of
		// molt fetch.
		DropOnTargetAndRecreate
		// TruncateIfExists means we truncate the table with the matching name
		// if it exists on the target side. If it doesn't exist, we exit with error.
		TruncateIfExists
	)

	const (
		noneTableHandlingKey                    = "none"
		dropOnTargetAndRecreateTableHandlingKey = "drop-on-target-and-recreate"
		truncateIfExistsTableHandlingKey        = "truncate-if-exists"
		usingDockerFlagStr                      = "using-docker-backend"
		tableConcurrencyFlagStr                 = "table-concurrency"
		exportConcurrencyFlagStr                = "export-concurrency"
	)

	var TableHandlingOptionStringRepresentations = map[TableHandlingOption][]string{
		None:                    {noneTableHandlingKey},
		DropOnTargetAndRecreate: {dropOnTargetAndRecreateTableHandlingKey},
		TruncateIfExists:        {truncateIfExistsTableHandlingKey},
	}

	var (
		bucketPath              string
		localPath               string
		localPathListenAddr     string
		localPathCRDBAccessAddr string
		logFile                 string
		directCRDBCopy          bool
		tableHandlingMode       TableHandlingOption
		cfg                     fetch.Config
	)
	cmd := &cobra.Command{
		Use:   "fetch",
		Short: "Moves data from source to target.",
		Long:  `Imports data from source directly into target tables.`,
		PersistentPreRunE: func(cmd *cobra.Command, args []string) error {
			commandsToremoveDBConnsFlag := map[string]any{"molt fetch tokens": nil}
			if _, ok := commandsToremoveDBConnsFlag[cmd.CommandPath()]; ok {
				// This marks these flags as not required.
				// In the case that we want to list molt fetch tokens,
				// we no longer need to mark the source and target as required flags.
				if err := cmd.InheritedFlags().SetAnnotation("source", cobra.BashCompOneRequiredFlag, []string{"false"}); err != nil {
					return err
				}

				if err := cmd.InheritedFlags().SetAnnotation("target", cobra.BashCompOneRequiredFlag, []string{"false"}); err != nil {
					return err
				}
			}

			cfg.IsTableConcurrencySet = cmd.Flag(tableConcurrencyFlagStr).Changed
			cfg.IsExportConcurrencySet = cmd.Flag(exportConcurrencyFlagStr).Changed

			return nil
		},
		PreRunE: func(cmd *cobra.Command, args []string) error {
			// Ensure that if continuation-token is set that fetch-id is set
			if err := cmdutil.CheckFlagDependency(cmd, fetchID, []string{continuationToken}); err != nil {
				return err
			}
			// Ensure if continuation-file-name is set that continuation-token is set.
			if err := cmdutil.CheckFlagDependency(cmd, continuationToken, []string{continuationFileName}); err != nil {
				return err
			}

			// Ensure the continuation-file-name matches the file pattern.
			if strings.TrimSpace(cfg.ContinuationFileName) != "" && !utils.MatchesFileConvention(cfg.ContinuationFileName) {
				return errors.Newf(`continuation file name "%s" doesn't match the file convention "%s"`, cfg.ContinuationFileName, utils.FileConventionRegex.String())
			}

			return nil
		},
		RunE: func(cmd *cobra.Command, args []string) error {
			ctx := context.Background()
			logger, err := moltlogger.Logger(logFile)
			if err != nil {
				return err
			}
			cmdutil.RunMetricsServer(logger)
			cmdutil.RunPprofServer(logger)

			switch tableHandlingMode {
			case TruncateIfExists:
				cfg.Truncate = true
			case DropOnTargetAndRecreate:
				cfg.DropAndRecreateNewSchema = true
			}

			isCopyMode := cfg.UseCopy || directCRDBCopy
			if isCopyMode {
				if cfg.Compression == compression.GZIP {
					return errors.New("cannot run copy mode with compression")
				} else if cfg.Compression <= compression.Default {
					logger.Info().Msgf("default compression to none")
					cfg.Compression = compression.None
				}
			} else if !isCopyMode && cfg.Compression <= compression.Default {
				logger.Info().Msgf("default compression to GZIP")
				cfg.Compression = compression.GZIP
			} else {
				logger.Info().Msgf("user set compression to %s", cfg.Compression.String())
			}

			conns, err := cmdutil.LoadDBConns(ctx)
			if err != nil {
				return err
			}
			if !conns[1].IsCockroach() {
				return errors.AssertionFailedf("target must be cockroach")
			}

			handleMySQLConcurrencyFlags(logger, conns[0], &cfg)

			if !isCopyMode {
				pgxConnForCRDB, ok := conns[1].(*dbconn.PGConn)
				if !ok {
					return errors.AssertionFailedf("the target conn to cockroachdb must be a pgx connection")
				}
				isAfter241, err := pgxConnForCRDB.CheckIfAfterVersion(ctx, "24.1")
				if err != nil {
					return errors.Wrapf(err, "failed checking if the target cockroachdb is newer than 24.1")
				}
				if isAfter241 {
					_, err = pgxConnForCRDB.Exec(ctx, `SET CLUSTER SETTING bulkio.import.retry_duration = '1s';`)
					if err != nil {
						return errors.Wrapf(err, "failed setting cluster setting bulkio.import.retry_duration")
					}
					defer func() {
						_, err = pgxConnForCRDB.Exec(ctx, `RESET CLUSTER SETTING bulkio.import.retry_duration`)
						if err != nil {
							logger.Warn().Err(err).Msgf("failed resetting cluster setting bulkio.import.retry_duration")
						}
					}()

					const minProtectedTimestampExtensionDur = 10 * time.Second
					if !cfg.TestOnly && cfg.ExportSettings.CRDBPTSExtensionFreq < minProtectedTimestampExtensionDur {
						return errors.AssertionFailedf("protected timestamp extension duration is not allowed to be set shorter than 10 seconds")
					}
				}
			}

			var datastorePayload any

			switch {
			case directCRDBCopy:
				datastorePayload = &datablobstorage.DirectCopyPayload{
					TargetConnForCopy: conns[1].(*dbconn.PGConn).Conn,
				}
				// We need to set UseCopy to true here so that telemetry
				// properly reports that copy was used.
				cfg.UseCopy = true
			case bucketPath != "":
				u, err := url.Parse(bucketPath)
				if err != nil {
					return err
				}
				// Trim the leading "/" that url.Parse returns
				// in u.Path as that will cause issues.
				path := strings.TrimPrefix(u.Path, "/")
				switch u.Scheme {
				case "s3", "S3":
					datastorePayload = &datablobstorage.S3Payload{
						S3Bucket:   u.Host,
						BucketPath: path,
					}
				case "gs", "GS":
					datastorePayload = &datablobstorage.GCPPayload{
						GCPBucket:  u.Host,
						BucketPath: path,
					}
				default:
					return errors.Newf("unsupported datasource scheme: %s", u.Scheme)
				}
			case localPath != "":
				datastorePayload = &datablobstorage.LocalPathPayload{
					LocalPath:               localPath,
					LocalPathListenAddr:     localPathListenAddr,
					LocalPathCRDBAccessAddr: localPathCRDBAccessAddr,
				}
			default:
				return errors.AssertionFailedf("data source must be configured (--bucket-path, --direct-copy, --local-path)")
			}

			src, err := datablobstorage.GenerateDatastore(ctx, datastorePayload, logger, false /* testFailedWriteToBucket */, cfg.TestOnly)
			if err != nil {
				return err
			}

			err = fetch.Fetch(
				ctx,
				cfg,
				logger,
				conns,
				src,
				cmdutil.TableFilter(),
				testutils.FetchTestingKnobs{},
			)

			if err != nil {
				fetchmetrics.NumTaskErrors.Inc()
			}

			return err
		},
	}

	cmd.AddCommand(tokens.Command())

	cmd.PersistentFlags().StringVar(
		&logFile,
		"log-file",
		"",
		"If set, writes to the log file specified. Otherwise, only writes to stdout.",
	)
	cmd.PersistentFlags().BoolVar(
		&cfg.Cleanup,
		"cleanup",
		false,
		"Whether any created resources should be deleted. Ignored if in direct-copy mode.",
	)
	cmd.PersistentFlags().BoolVar(
		&directCRDBCopy,
		"direct-copy",
		false,
		"Enables direct copy mode, which copies data directly from source to target without using an intermediate store.",
	)
	cmd.PersistentFlags().BoolVar(
		&cfg.UseCopy,
		"use-copy",
		false,
		"Use `COPY FROM` instead of `IMPORT INTO` during the data movement phase. This keeps your table online during the process.",
	)
	cmd.PersistentFlags().IntVar(
		&cfg.FlushSize,
		"flush-size",
		0,
		"If set, size (in bytes) before the source data is flushed to intermediate files.",
	)
	cmd.PersistentFlags().IntVar(
		&cfg.FlushRows,
		"flush-rows",
		0,
		"If set, number of rows before the source data is flushed to intermediate files.",
	)

	cmd.PersistentFlags().IntVar(
		&cfg.TableConcurrency,
		tableConcurrencyFlagStr,
		4,
		"Number of tables to move at a time.",
	)
	cmd.PersistentFlags().IntVar(
		&cfg.Shards,
		exportConcurrencyFlagStr,
		4,
		"Number of threads to use for data export.",
	)
	cmd.PersistentFlags().StringVar(
		&bucketPath,
		"bucket-path",
		"",
		"Path of the s3/gcp bucket where intermediate files are written (e.g., s3://bucket/path, or gs://bucket/path).",
	)
	cmd.PersistentFlags().StringVar(
		&localPath,
		"local-path",
		"",
		"Path to upload files to locally.",
	)
	cmd.PersistentFlags().StringVar(
		&localPathListenAddr,
		"local-path-listen-addr",
		"",
		"Address of a local store server to listen to for traffic.",
	)
	cmd.PersistentFlags().StringVar(
		&localPathCRDBAccessAddr,
		"local-path-crdb-access-addr",
		"",
		"Address of data that CockroachDB can access to import from a local store (defaults to local-path-listen-addr).",
	)
	cmd.MarkFlagsMutuallyExclusive("bucket-path", "local-path")

	// The test-only is for internal use only and is hidden from the usage or help prompt.
	const testOnlyFlagStr = "test-only"
	cmd.PersistentFlags().BoolVar(
		&cfg.TestOnly,
		testOnlyFlagStr,
		false,
		"Whether this fetch attempt is only for test, and hence all time/duration related stats are deterministic",
	)

	cmd.PersistentFlags().IntVar(
		&cfg.ExportSettings.RowBatchSize,
		"row-batch-size",
		100_000,
		"Number of rows to select at a time for export from the source database.",
	)

	cmd.PersistentFlags().DurationVar(
		&cfg.ExportSettings.CRDBPTSExtensionFreq,
		"crdb-pts-refresh-interval",
		10*time.Minute,
		"Duration for the ticker to wait for the next protected timestamp extension. The new expiration timestamp of the protected timestamp will be {time when extension happens} + {crdb-pts-duration}",
	)

	cmd.PersistentFlags().DurationVar(
		&cfg.ExportSettings.CRDBPTSExtensionLifetime,
		"crdb-pts-duration",
		24*time.Hour,
		"The lifetime of a protected timestamp extension",
	)

	cmd.PersistentFlags().StringVar(
		&cfg.ExportSettings.PG.SlotName,
		"pglogical-replication-slot-name",
		"",
		"If set, the name of a replication slot that will be created before taking a snapshot of data.",
	)
	cmd.PersistentFlags().StringVar(
		&cfg.ExportSettings.PG.Plugin,
		"pglogical-replication-slot-plugin",
		"pgoutput",
		"If set, the output plugin used for logical replication under pglogical-replication-slot-name.",
	)
	cmd.PersistentFlags().BoolVar(
		&cfg.ExportSettings.PG.DropIfExists,
		"pglogical-replication-slot-drop-if-exists",
		false,
		"If set, drops the replication slot if it exists.",
	)
	cmd.PersistentFlags().Var(
		enumflag.New(
			&cfg.Compression,
			"compression",
			compression.CompressionStringRepresentations,
			enumflag.EnumCaseInsensitive,
		),
		"compression",
		"Compression type for IMPORT INTO mode (gzip/none). (default gzip)",
	)
	cmd.PersistentFlags().StringVar(
		&cfg.FetchID,
		fetchID,
		"",
		"If set, restarts the fetch process for all failed tables of the given ID",
	)
	cmd.PersistentFlags().StringVar(
		&cfg.ContinuationToken,
		continuationToken,
		"",
		"If set, restarts the fetch process for the given continuation token for a specific table",
	)
	cmd.PersistentFlags().StringVar(
		&cfg.ContinuationFileName,
		continuationFileName,
		"",
		"If set, restarts the fetch process for at the given file name instead of the recorded file in the exceptions table",
	)

	const nonInteractiveStr = "non-interactive"
	cmd.PersistentFlags().BoolVar(
		&cfg.NonInteractive,
		nonInteractiveStr,
		false,
		`If set, automatically skips all user prompting and initiates actions such as clearing exception log data (preferable if running in CI)
or as an automated job. If not set, prompts user for confirmation before performing actions.`,
	)

	cmd.PersistentFlags().BoolVar(
		&cfg.OngoingReplication,
		"ongoing-replication",
		false,
		"Whether or not to start ongoing replication after the initial data load",
	)

	cmd.PersistentFlags().StringVar(
		&cfg.ReplicatorFlags,
		"replicator-flags",
		"",
		"Flags to pass into replicator",
	)

	cmd.PersistentFlags().BoolVar(
		&cfg.AllowCockroachReplication,
		"allow-cockroach-replication",
		false,
		"Whether or not to allow ongoing replication for a Cockroach source",
	)

	cmd.PersistentFlags().BoolVar(
		&cfg.UsingDockerBackend,
		"using-docker-backend",
		false,
		"Whether docker is used for the database backend, which will change the behavior of certain URL pathing (i.e. changefeed URL)",
	)

	moltlogger.RegisterLoggerFlags(cmd)
	cmdutil.RegisterDBConnFlags(cmd)
	cmdutil.RegisterNameFilterFlags(cmd)
	cmdutil.RegisterMetricsFlags(cmd)
	cmdutil.RegisterPprofFlags(cmd)

	cmd.PersistentFlags().Var(
		enumflag.NewWithoutDefault(&tableHandlingMode, "string", TableHandlingOptionStringRepresentations, enumflag.EnumCaseInsensitive),
		"table-handling",
		fmt.Sprintf("the way to handle the table initialization on the target database: %q(default), %q or %q",
			noneTableHandlingKey,
			dropOnTargetAndRecreateTableHandlingKey,
			truncateIfExistsTableHandlingKey,
		),
	)

	cmd.PersistentFlags().StringVar(&cfg.CustomizedTypeMapPath, "type-map-file", "",
		`The path to a json file that add to or override specified type mapping from the source dialect to CockroachDB for given column. 
This is also used for automatic schema recreation enabled via --table-handling=drop-on-target-and-recreate. 
If a rule has specified column "*", all columns with matching source type will apply this mapping rule, unless specifically specified.
`)

	for _, hidden := range []string{testOnlyFlagStr, nonInteractiveStr, "allow-cockroach-replication", usingDockerFlagStr} {
		if err := cmd.PersistentFlags().MarkHidden(hidden); err != nil {
			panic(err)
		}
	}

	return cmd
}

func handleMySQLConcurrencyFlags(logger zerolog.Logger, sourceConn dbconn.Conn, cfg *fetch.Config) {
	if _, ok := sourceConn.(*dbconn.MySQLConn); ok {
		if !cfg.IsExportConcurrencySet && !cfg.IsTableConcurrencySet {
			logger.Warn().Msg("defaulting export concurrency and table concurrency to 1 in order to guarantee data consistency for MySQL")
			cfg.Shards = 1
			cfg.TableConcurrency = 1
		} else if (cfg.IsTableConcurrencySet || cfg.IsExportConcurrencySet) && (cfg.TableConcurrency > 1 || cfg.Shards > 1) {
			logger.Warn().Msgf("table concurrency or export concurrency is greater than 1. This can lead to inconsistency when migrating MySQL data. For details on maintaining consistency when using --table-concurrency and --export-concurrency: %s", mysqlDocsLink)
		}
	}
}
