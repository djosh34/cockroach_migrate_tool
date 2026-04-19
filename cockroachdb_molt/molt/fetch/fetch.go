package fetch

import (
	"context"
	"fmt"
	"math"
	"os"
	"os/exec"
	"os/signal"
	"runtime"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/cockroachdb/cockroachdb-parser/pkg/util/uuid"
	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/compression"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/fetch/datablobstorage"
	"github.com/cockroachdb/molt/fetch/dataexport"
	"github.com/cockroachdb/molt/fetch/fetchcontext"
	"github.com/cockroachdb/molt/fetch/fetchmetrics"
	"github.com/cockroachdb/molt/fetch/replicator"
	"github.com/cockroachdb/molt/fetch/status"
	"github.com/cockroachdb/molt/moltlogger"
	"github.com/cockroachdb/molt/molttelemetry"
	"github.com/cockroachdb/molt/testutils"
	"github.com/cockroachdb/molt/utils"
	"github.com/cockroachdb/molt/verify"
	"github.com/cockroachdb/molt/verify/dbverify"
	"github.com/cockroachdb/molt/verify/rowverify"
	"github.com/cockroachdb/molt/verify/tableverify"
	"github.com/jackc/pgx/v5"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/rs/zerolog"
	"golang.org/x/exp/slices"
	"golang.org/x/sync/errgroup"
)

const testForcedError = "test forced an error"

type Config struct {
	FlushSize            int
	FlushRows            int
	Cleanup              bool
	UseCopy              bool
	TableConcurrency     int
	Shards               int
	FetchID              string
	ContinuationToken    string
	ContinuationFileName string
	// TestOnly means this fetch attempt is just for test, and hence all time/duration
	// stats are deterministic.
	TestOnly bool

	// The target table handling configs.
	Truncate bool

	DropAndRecreateNewSchema bool
	CustomizedTypeMapPath    string

	// NonInteractive relates to if user input should be prompted. If false,
	// user prompting is initiating before certain actions like wiping data.
	// If true, user prompting will be skipped and actions will be confirmed automatically.
	NonInteractive bool

	Compression        compression.Flag
	ExportSettings     dataexport.Settings
	OngoingReplication bool

	// AllowCockroachReplication gates Cockroach source replication using CDC sink.
	// TODO (rluu): remove this gate when feature is mature.
	AllowCockroachReplication bool

	// UsingDockerBackend lets fetch know if a docker backend is used
	// for the database spin up. This will affect URL pathing for
	// localstore and changefeed setup.
	UsingDockerBackend bool

	// IsTableConcurrencySet marks whether a user set the table concurrency flag.
	IsTableConcurrencySet bool
	// IsExportConcurrencySet marks whether a user set the export concurrency flag.
	IsExportConcurrencySet bool

	// ReplicatorFlags are the flags that can be passed into the replicator command.
	ReplicatorFlags string
}

// handleUnsupportedFeatures will check flags and properties of the configuration
// to ensure that fetch exits early and lets the customer know that a feature
// is not currently available.
func handleUnsupportedFeatures(conns dbconn.OrderedConns, cfg Config) error {
	// In the case that cfg.AllowCockroachReplication (a hidden flag) is set,
	// Cockroach replication will be allowed. Currently this is only mature
	// enough for internal testing.
	if pgConn, ok := conns[0].(*dbconn.PGConn); ok && pgConn.IsCockroach() &&
		cfg.OngoingReplication && !cfg.AllowCockroachReplication {
		return errors.New("Cockroach source replication is not currently supported")
	}

	return nil
}

func Fetch(
	ctx context.Context,
	cfg Config,
	logger zerolog.Logger,
	conns dbconn.OrderedConns,
	blobStore datablobstorage.Store,
	tableFilter utils.FilterConfig,
	testingKnobs testutils.FetchTestingKnobs,
) (retErr error) {
	if err := handleUnsupportedFeatures(conns, cfg); err != nil {
		return err
	}

	// Setup fetch status tracking.
	targetPgConn, valid := conns[1].(*dbconn.PGConn)
	if !valid {
		return errors.New("failed to assert conn as a pgconn")
	}

	targetPgxConn := targetPgConn.Conn
	fetchStatus, err := initStatusEntry(ctx, cfg, targetPgxConn, conns[0].Dialect())
	if err != nil {
		return err
	}
	ctx = fetchcontext.ContextWithFetchData(ctx, fetchcontext.FetchContextData{
		RunID:     fetchStatus.ID,
		StartedAt: fetchStatus.StartedAt,
	})

	timer := prometheus.NewTimer(prometheus.ObserverFunc(fetchmetrics.OverallDuration.Set))

	if cfg.FlushSize == 0 {
		cfg.FlushSize = blobStore.DefaultFlushBatchSize()
	}
	if cfg.TableConcurrency == 0 {
		cfg.TableConcurrency = 4
	}

	if cfg.Cleanup {
		defer func() {
			if err := blobStore.Cleanup(ctx); err != nil {
				logger.Err(err).Msgf("error marking object for cleanup")
			}
		}()
	}

	if err := dbconn.RegisterTelemetry(conns); err != nil {
		return err
	}
	reportTelemetry(logger, cfg, conns, blobStore)

	dataLogger := moltlogger.GetDataLogger(logger)
	dataLogger.Debug().
		Int("flush_size", cfg.FlushSize).
		Int("flush_num_rows", cfg.FlushRows).
		Str("store", fmt.Sprintf("%T", blobStore)).
		Msg("initial config")

	logger.Info().Msgf("checking database details")
	dbTables, err := dbverify.Verify(ctx, conns)
	if err != nil {
		return err
	}
	if dbTables, err = utils.FilterResult(tableFilter, dbTables); err != nil {
		return err
	}
	for _, tbl := range dbTables.ExtraneousTables {
		logger.Warn().
			Str("table", tbl.SafeString()).
			Msgf("ignoring table as it is missing a definition on the source")
	}

	if !cfg.DropAndRecreateNewSchema {
		for _, tbl := range dbTables.MissingTables {
			logger.Warn().
				Str("table", tbl.SafeString()).
				Msgf("ignoring table as it is missing a definition on the target")
		}
	}
	for _, tbl := range dbTables.Verified {
		logger.Info().
			Str("source_table", tbl[0].SafeString()).
			Str("target_table", tbl[1].SafeString()).
			Msgf("found matching table")
	}

	if cfg.DropAndRecreateNewSchema {
		tablesToProcess := dbTables.AllTablesFromSource()
		if len(tablesToProcess) == 0 {
			logger.Info().Msgf("no tables to drop and recreate on the target")
		} else {
			logger.Info().Msgf("creating schema for tables: %s", tablesToProcess)
			targetConn, ok := conns[1].(*dbconn.PGConn)
			if !ok {
				return errors.AssertionFailedf("the target connection is not a pg connection for CockroachDB")
			}

			typeMap, err := GetTypeConvMap(cfg.CustomizedTypeMapPath, logger)
			if err != nil {
				return err
			}

			var checkedMultiRegionConsistency bool

			for _, t := range tablesToProcess {
				dropTableStmt, err := GetDropTableStmt(t)
				if err != nil {
					return err
				}
				logger.Debug().Msgf("dropping table with %s", dropTableStmt)
				if _, err := targetConn.Exec(ctx, dropTableStmt); err != nil {
					return errors.Wrapf(err, "failed to drop table [%s] on the target connection", t)
				}
				logger.Debug().Msgf("finished dropping table with %s", dropTableStmt)

				if err := maybeCheckTableLocality(ctx, logger, conns[0], &t); err != nil {
					return err
				}

				if !checkedMultiRegionConsistency {
					checkedMultiRegionConsistency, err = MaybeCheckMultiRegionLocality(ctx, logger, conns, &t)
					if err != nil {
						return errors.Wrapf(err, "failed checking multi-region consistency between source and target databases")
					}
				}

				createTableStmt, err := GetCreateTableStmt(ctx, logger, conns, t, typeMap)
				if err != nil {
					return err
				}
				logger.Info().Msgf("creating new table with [%s]", createTableStmt)
				if _, err := targetConn.Exec(ctx, createTableStmt); err != nil {
					return errors.Wrapf(err, "failed to create new schema [%s] on the target connection with [%s]", t, createTableStmt)
				}
				logger.Debug().Msgf("finished creating new table with [%s]", createTableStmt)

				// TODO(janexing): maybe persist it in table?
				droppedConstraints, err := GetConstraints(ctx, logger, conns[0], t)
				if err != nil {
					return err
				}
				if len(droppedConstraints) != 0 {
					consWithTable := constraintsWithTable{table: t, cons: droppedConstraints}
					logger.Warn().Msgf("newly created schema doesn't contain the following constraints:%s", consWithTable.String())
				}
			}
			// Redo the verify.
			dbTables, err = dbverify.Verify(ctx, conns)
			if err != nil {
				return errors.Wrap(err, "failed to re-verify tables after schema creation")
			}
			if dbTables, err = utils.FilterResult(tableFilter, dbTables); err != nil {
				return err
			}
			logger.Info().Msgf("after recreating table, dbTables: %s", dbTables)
		}
	}

	logger.Info().Msgf("verifying common tables")
	tables, err := tableverify.VerifyCommonTables(ctx, conns, logger, dbTables.Verified)
	if err != nil {
		return err
	}
	if tables == nil {
		logger.Warn().Msgf("no table(s) to be imported, exiting molt fetch")
		return nil
	}
	logger.Info().Msgf("establishing snapshot")
	sqlSrc, err := dataexport.InferExportSource(ctx, cfg.ExportSettings, conns[0], logger, cfg.TestOnly)
	if err != nil {
		return err
	}
	defer func() {
		if err := sqlSrc.Close(ctx); err != nil {
			logger.Err(err).Msgf("error closing export source")
		}
	}()

	tableToExportStatus := sync.Map{}
	finishedDataLoad := false

	// Wait until all the verification portions are completed first before deferring this.
	// If verify fails, we don't need to report fetch_id.
	// We only want to log out if the fetch fails.
	defer func() {
		// Check if export finished for each table.
		finishedExport := true
		tableToExportStatus.Range(func(key, value any) bool {
			if value == false {
				logger.Info().Msgf("table %s failed export, omitting continuation token", key)
				finishedExport = false
				return false
			}

			return true
		})

		// Continuation tokens are only relevant if the export is completed
		// and the full initial data load did not complete yet. They are
		// not meaningful if export did not complete or if we are already
		// at the replication portion of the process.
		if retErr != nil && finishedExport && !finishedDataLoad {
			fetchID := fetchStatus.ID.String()
			logger.Info().
				Str("fetch_id", utils.MaybeFormatFetchID(cfg.TestOnly, fetchID)).Msg("continue from this fetch ID")
		}
	}()

	numTables := len(tables)
	summaryLogger := moltlogger.GetSummaryLogger(logger)
	summaryLogger.Info().
		Int("num_tables", numTables).
		Str("cdc_cursor", utils.MaybeFormatCDCCursor(cfg.TestOnly, sqlSrc.CDCCursor())).
		Msgf("starting fetch")
	fetchmetrics.NumTablesProcessed.Add(float64(numTables))

	type statsMu struct {
		sync.Mutex
		numImportedTables int
		importedTables    []string
	}
	var stats statsMu

	// If continuation file is passed in, it must conform to the file format.
	if cfg.ContinuationFileName != "" && !utils.FileConventionRegex.Match([]byte(cfg.ContinuationFileName)) {
		return errors.Newf("continuation file name %s doesn't match the format %s", cfg.ContinuationFileName, utils.FileConventionRegex.String())
	}

	exceptionLogMapping, err := getExceptionLogMapping(ctx, cfg, targetPgxConn)
	contTokenNotFoundErr := fmt.Sprintf("no exception logs that correspond to continuation-token of %s", cfg.ContinuationToken)

	// In the case that we have no results for the passed in continuation token or
	// fetch ID, we should error to let the user know it's invalid, instead of
	// doing a fetch in an unknown state.
	if err != nil && err == pgx.ErrNoRows {
		return errors.New(contTokenNotFoundErr)
	} else if err != nil {
		return err
	}

	if IsImportCopyOnlyMode(cfg) && len(exceptionLogMapping) == 0 {
		errMsg := fmt.Sprintf("no exception logs that correspond to fetch-id of %s", cfg.FetchID)
		if cfg.ContinuationToken != "" {
			errMsg = contTokenNotFoundErr
		}

		return errors.New(errMsg)
	}

	// We want to do the exceptions log deleting after the exception log retrieval/checks for two reasons.
	// 1. We want to get the most recent exception logs for the mode of continuation where fetch-id is specified
	// 2. We don't want the check for isImportCopyOnly mode and len(mapping) = 0 to cause an error
	// This case will certainly happen if we clear the table first before checking the mapping size.
	isClearContinuationTokenMode := IsClearContinuationTokenMode(cfg)
	if isClearContinuationTokenMode {
		if cfg.NonInteractive {
			logger.Warn().Msg("clearing all continuation tokens because running in clear continuation mode")
		} else {
			fmt.Println("Clearing all continuation tokens. Confirm (y/n)?")
			var confirmation string
			fmt.Scanln(&confirmation)

			if !strings.EqualFold(confirmation, "y") {
				return errors.New("clearing continuation tokens was not confirmed, exiting early")
			}
		}

		if err := status.DeleteAllExceptionLogs(ctx, targetPgxConn); err != nil {
			return err
		}
	}

	workCh := make(chan tableverify.Result)
	g, _ := errgroup.WithContext(ctx)
	for i := 0; i < cfg.TableConcurrency; i++ {
		g.Go(func() error {
			for {
				table, ok := <-workCh
				if !ok {
					return nil
				}

				// Get and first and last of each PK.
				shardClone, err := conns[0].Clone(ctx)
				if err != nil {
					return err
				}
				defer shardClone.Close(ctx)
				tableShards, err := verify.ShardTable(ctx, shardClone, table, nil, cfg.Shards)
				if err != nil {
					return errors.Wrapf(err, "error splitting tables")
				}

				var relevantExceptionLog *status.ExceptionLog
				if v, ok := exceptionLogMapping[table.SafeString()]; ok {
					relevantExceptionLog = v
				}

				// We want to run the fetch in only two cases:
				// 1. Export + import mode combined (when fetch ID is not passed in; means new fetch)
				// 2. When the fetch ID is passed in and exception log is not nil, which means it is a table we want to continue from.
				// This means we want to skip if we are trying to continue but there is no entry that specifies where to continue from.
				if (cfg.FetchID != "" && relevantExceptionLog != nil) || (cfg.FetchID == "") {
					madePastExport, err := fetchTable(ctx, cfg, logger, conns, blobStore, sqlSrc, table, tableShards, relevantExceptionLog, isClearContinuationTokenMode, testingKnobs)
					tableToExportStatus.Store(table.SafeString(), madePastExport)
					if err != nil {
						return err
					}
				} else {
					logger.Warn().Msgf("skipping fetch for %s", table.SafeString())
				}

				stats.Lock()
				stats.numImportedTables++
				stats.importedTables = append(stats.importedTables, table.SafeString())
				stats.Unlock()
			}
		})
	}

	go func() {
		defer close(workCh)
		for _, table := range tables {
			workCh <- table
		}
	}()

	if err := g.Wait(); err != nil {
		return err
	}

	ovrDuration := utils.MaybeFormatDurationForTest(cfg.TestOnly, timer.ObserveDuration())

	// Ensure that the tables slice is sorted so
	// e2e output is consistent across runs.
	slices.Sort(stats.importedTables)

	summaryLogger.Info().
		Str("fetch_id", utils.MaybeFormatFetchID(cfg.TestOnly, fetchStatus.ID.String())).
		Int("num_tables", stats.numImportedTables).
		Strs("tables", stats.importedTables).
		Str("cdc_cursor", utils.MaybeFormatCDCCursor(cfg.TestOnly, sqlSrc.CDCCursor())).
		Dur("net_duration_ms", ovrDuration).
		Str("net_duration", utils.FormatDurationToTimeString(ovrDuration)).
		Msgf("fetch complete")
	finishedDataLoad = true

	if cfg.OngoingReplication && len(dbTables.Verified) > 0 {
		logger.Info().Msgf("starting replicator")

		wd, err := os.Getwd()
		if err != nil {
			return err
		}

		binName, err := replicator.FindReplicatorBinary(wd)
		if err != nil {
			return err
		}

		if err := replicator.SetupReplicator(ctx, conns, tables, dbTables.Verified[0][1].Schema, sqlSrc.CDCCursor(), logger); err != nil {
			return err
		}

		cmd, err := sqlSrc.ReplicatorCommand(binName, conns[1], conns[1].Database(), dbTables.Verified[0][1].Schema, cfg.ReplicatorFlags)
		if err != nil {
			return err
		}

		sigintChan := make(chan os.Signal, 1)
		cmdErrChan := make(chan error)
		go func() {
			logger.Debug().Str("command", cmd.String()).Msgf("executing CDC sink command")

			cmd.Stdout = os.Stdout
			cmd.Stderr = os.Stderr
			if err = cmd.Start(); err != nil {
				cmdErrChan <- err
				return
			}

			// Cockroach specific setup
			// Create the changefeed on the source side
			if conns[0].IsCockroach() {
				tblQ := replicator.CreateTableList(tables)
				logger.Info().Msg("creating changefeed on the source CRDB database")
				localhostName := "localhost"
				if testingKnobs.CDCSink != nil && testingKnobs.CDCSink.LocalhostName != "" {
					localhostName = testingKnobs.CDCSink.LocalhostName
				} else if cfg.UsingDockerBackend {
					localhostName = getDockerLocalhostName()
				}

				q := fmt.Sprintf(`CREATE CHANGEFEED FOR TABLE %s
				INTO 'webhook-https://%s:30004/%s/%s?insecure_tls_skip_verify=true'
				WITH updated, resolved='1s', min_checkpoint_frequency='1s', initial_scan='no', cursor='%s',
					 webhook_sink_config='{"Flush":{"Bytes":1048576,"Frequency":"1s"}}'`, tblQ, localhostName, conns[1].Database(), dbTables.Verified[0][1].Schema, sqlSrc.CDCCursor())
				if _, err := conns[0].(*dbconn.PGConn).Exec(ctx, q); err != nil {
					cmdErrChan <- err
					return
				}
			}

			if testingKnobs.CDCSink != nil {
				if testingKnobs.CDCSink.ForceCDCSinkError {
					cmdErrChan <- errors.New(testForcedError)
					return
				}

				go func() {
					if err := testingKnobs.CDCSink.PollingFunction(conns, testingKnobs.CDCSink.SentinelFileName, sigintChan); err != nil {
						logger.Err(err).Send()
					}
				}()
			}

			procState, err := cmd.Process.Wait()
			if err != nil {
				cmdErrChan <- err
				return
			} else if procState.ExitCode() != 0 {
				cmdErrChan <- errors.New("CDC sink command failed to run")
				return
			}
		}()

		// NB: when run with "go run ." the interrupt from ctrl + C will always cause the run
		// to exit with a non-zero exit code. However, if you build the binary
		// first, then run it, you'll see that it properly exits with exit code 0.
		return watchForShutdown(logger, sigintChan, cmdErrChan, cmd)
	}
	return nil
}

func getDockerLocalhostName() string {
	// Resources:
	// https://stackoverflow.com/questions/48546124/what-is-linux-equivalent-of-host-docker-internal
	// https://docs.docker.com/desktop/networking/#i-want-to-connect-from-a-container-to-a-service-on-the-host
	// In the CI, the databases are all spin up in docker-compose,
	// which not necessarily share the network with the host.
	// When importing the data to the target database,
	// it requires the database reaches the local storage server
	// (spun up on host network) from the container (i.e. from
	// the container's network). According to the 2 links
	// above, the `localhost` on the host network is accessible
	// via different endpoint based on the operating system:
	// - Linux, Windows: 172.17.0.1
	// - MacOS: host.docker.internal
	switch runtime.GOOS {
	case "darwin":
		return "host.docker.internal"
	default:
		return "172.17.0.1"
	}
}

func watchForShutdown(
	logger zerolog.Logger, sigint chan os.Signal, cmdErrChan chan error, cmd *exec.Cmd,
) error {
	signal.Notify(sigint, os.Interrupt)
	signal.Notify(sigint, syscall.SIGTERM)

	select {
	case err := <-cmdErrChan:
		logger.Error().Msg("error while running CDC sink")

		// We need to stop the command here in the case that we errored out.
		// Otherwise we may leave an orphaned process.
		sigTermErr := cmd.Process.Signal(syscall.SIGTERM)
		logger.Err(sigTermErr).Send()
		return err
	case <-sigint:
		logger.Info().Msg("received interrupt signal")
		return cmd.Process.Signal(syscall.SIGTERM)
	}
}

func truncateTable(
	ctx context.Context,
	logger zerolog.Logger,
	table tableverify.Result,
	truncateTargetTableConn dbconn.Conn,
) error {
	logger.Info().Msgf("truncating table")
	_, err := truncateTargetTableConn.(*dbconn.PGConn).Conn.Exec(ctx, "TRUNCATE TABLE "+table.SafeString())
	if err != nil {
		return errors.Wrap(err, "failed executing the TRUNCATE TABLE statement")
	}
	logger.Info().Msgf("finished truncating table")
	return nil
}

// Note that if `ExceptionLog` is not nil, then that means
// there is an exception log and import/copy only mode
// was specified.
func fetchTable(
	ctx context.Context,
	cfg Config,
	logger zerolog.Logger,
	conns dbconn.OrderedConns,
	blobStore datablobstorage.Store,
	sqlSrc dataexport.Source,
	table tableverify.Result,
	shards []rowverify.TableShard,
	exceptionLog *status.ExceptionLog,
	isClearContinuationTokenMode bool,
	testingKnobs testutils.FetchTestingKnobs,
) (finishedExport bool, retErr error) {
	finishedExport = false
	tableStartTime := time.Now()
	// Initialize metrics for this table so we can calculate a rate later.
	fetchmetrics.ExportedRows.WithLabelValues(table.SafeString())
	fetchmetrics.ImportedRows.WithLabelValues(table.SafeString())

	for _, col := range table.MismatchingTableDefinitions {
		logger.Warn().
			Str("reason", col.Info).
			Msgf("not migrating column %s as it mismatches", col.Name)
	}
	if !table.RowVerifiable {
		logger.Error().Msgf("table %s do not have matching primary keys, cannot migrate", table.SafeString())
		return finishedExport, nil
	}

	targetTableConnCopy, err := conns[1].Clone(ctx)
	if err != nil {
		return finishedExport, errors.Wrap(err, "unable to clone the target table connection")
	}
	defer func() {
		err := targetTableConnCopy.Close(ctx)
		if err != nil {
			logger.Err(err).Msg("failed to close connection to copy")
		}
	}()

	// Truncate table on the target side, if applicable.
	if cfg.Truncate && !IsImportCopyOnlyMode(cfg) {
		if err := truncateTable(ctx, logger, table, targetTableConnCopy); err != nil {
			return finishedExport, err
		}
	} else if cfg.Truncate && IsImportCopyOnlyMode(cfg) {
		logger.Warn().Msg("truncate is skipped because you are using a continuation mode and it could result in missing data")
	}

	logger.Info().Msgf("data extraction phase starting")

	var e exportResult
	// In the case that exception log is nil or fetch id is empty,
	// this means that we want to export the table because it means
	// we want export + copy mode.
	if exceptionLog == nil || cfg.FetchID == "" {
		// Set up the upper and lower bounds for start/end min max comparisons
		e.StartTime = time.Unix(math.MaxInt, 0)
		e.EndTime = time.Unix(math.MinInt, 0)
		orderedResults := make([]exportResult, len(shards))

		var jobManagementWg *errgroup.Group

		exportTableFunc := func() (ret error) {
			exportFinishedChan := make(chan struct{})
			defer func() {
				close(exportFinishedChan)
				if jobManagementWg != nil {
					err := jobManagementWg.Wait()
					if err != nil {
						// If we failed the PTS job, log the error but don't let it block the whole process.
						logger.Err(err).Msgf("failed to accomplish history retention job")
					} else {
						if testingKnobs.HistoryRetention != nil {
							testingKnobs.HistoryRetention.Cancelled = true
						}
						logger.Info().Msgf("history retention job has finished")
					}
				}
			}()

			extensionFreq := cfg.ExportSettings.CRDBPTSExtensionFreq
			if testingKnobs.HistoryRetention != nil && testingKnobs.HistoryRetention.ExtensionFrequency != 0 {
				extensionFreq = testingKnobs.HistoryRetention.ExtensionFrequency
			}

			var extensionCnt *int64
			if testingKnobs.HistoryRetention != nil {
				extensionCnt = testingKnobs.HistoryRetention.ExtensionCnt
			}

			jobManagementWg = sqlSrc.HistoryRetentionJobManagement(ctx, logger, exportFinishedChan, extensionFreq, extensionCnt, cfg.TestOnly, &testingKnobs)

			wg, _ := errgroup.WithContext(ctx)
			for i, s := range shards {
				it, sh := i, s
				wg.Go(func() error {
					er, err := exportTable(ctx, cfg, logger, sqlSrc, blobStore, table.VerifiedTable, sh, testingKnobs)
					if err != nil {
						return err
					}
					orderedResults[it] = er
					return nil
				})
			}
			if err = wg.Wait(); err != nil {
				return err
			}
			return nil
		}

		if err := exportTableFunc(); err != nil {
			return finishedExport, err
		}

		for _, er := range orderedResults {
			e.StartTime = time.Unix(min(e.StartTime.Unix(), er.StartTime.Unix()), 0)
			e.EndTime = time.Unix(max(e.EndTime.Unix(), er.EndTime.Unix()), 0)
			e.NumRows += er.NumRows
			e.Resources = append(e.Resources, er.Resources...)
		}
	} else {
		if exceptionLog.FileName == "" {
			logger.Warn().Msgf("skipping table %s because no file name is present in the exception log", table.SafeString())
			return finishedExport, errors.Newf("table %s not imported because no file name is present in the exception log", table.SafeString())
		}
		logger.Warn().Msgf("skipping export for table %s due to running in import-copy only mode", table.SafeString())

		// TODO: future PR needs to add number of rows estimation. and populate exportResult.NumRows
		// TODO: need to figure out start and end time too.
		rsc, err := blobStore.ListFromContinuationPoint(ctx, table.VerifiedTable, exceptionLog.FileName)
		if err != nil {
			return finishedExport, err
		}
		e.Resources = rsc

		if len(e.Resources) == 0 {
			return finishedExport, errors.Newf("exported resources for table %s is empty, please make sure you did not accidentally delete from the intermediate store", table.SafeString())
		}
	}
	// We actually need to skip the cleanup for something that has an error
	// On a continuation run we can cleanup so long as it's successful.
	if cfg.Cleanup {
		defer func() {
			if retErr != nil {
				logger.Info().Msg("skipping cleanup because an error occurred and files may need to be kept for continuation")
				return
			}

			logger.Info().Msg("cleaning up resources created during fetch run")
			for _, r := range e.Resources {
				if r == nil {
					continue
				}

				if err := r.MarkForCleanup(ctx); err != nil {
					logger.Err(err).Msgf("error cleaning up resource")
				}
			}
		}()
	}

	finishedExport = true

	// TODO: consider if we want to skip this portion since we don't export anything....
	exportDuration := utils.MaybeFormatDurationForTest(cfg.TestOnly, e.EndTime.Sub(e.StartTime))
	summaryLogger := moltlogger.GetSummaryLogger(logger)
	summaryLogger.Info().
		Int("num_rows", e.NumRows).
		Dur("export_duration_ms", exportDuration).
		Str("export_duration", utils.FormatDurationToTimeString(exportDuration)).
		Msgf("data extraction from source complete")
	fetchmetrics.TableExportDuration.WithLabelValues(table.SafeString()).Set(float64(exportDuration.Milliseconds()))

	if blobStore.CanBeTarget() {
		var importDuration time.Duration

		// Make sure this is outside the closure below so that retErr is assigned to the error properly.
		// In the case that retErr is nil, it means that this table
		// fetch suceeded and we want to delete the entry for
		// the continuation token because it's no longer relevant.
		defer func() {
			if retErr == nil && !isClearContinuationTokenMode && exceptionLog != nil {
				targetPgConn, valid := targetTableConnCopy.(*dbconn.PGConn)
				if !valid {
					retErr = errors.New("failed to assert conn as a pgconn")
				}
				targetPgxConn := targetPgConn.Conn

				if err := exceptionLog.DeleteEntry(ctx, targetPgxConn); err != nil {
					retErr = err
				} else {
					logger.Info().Msgf("removing exception log for continuation-token %s because fetch was successful on table %s", exceptionLog.ID, table.SafeString())
				}
			}
		}()

		if err := func() error {
			logger.Info().
				Msgf("starting data import on target")

			isLocal := false
			if len(e.Resources) > 0 {
				isLocal = e.Resources[0].IsLocal()
			}

			if !cfg.UseCopy {
				go func() {
					err := reportImportTableProgress(ctx,
						targetTableConnCopy,
						logger,
						table.VerifiedTable,
						time.Now(),
						false /*testing*/)
					if err != nil {
						logger.Err(err).Msg("failed to report import table progress")
					}
				}()

				r, err := importTable(ctx, cfg, targetTableConnCopy, logger, table.VerifiedTable, e.Resources, isLocal, isClearContinuationTokenMode, exceptionLog)
				if err != nil {
					return err
				}
				importDuration = utils.MaybeFormatDurationForTest(cfg.TestOnly, r.EndTime.Sub(r.StartTime))
			} else {
				r, err := Copy(ctx, targetTableConnCopy, logger, table.VerifiedTable, e.Resources, isLocal, isClearContinuationTokenMode, exceptionLog)
				if err != nil {
					return err
				}
				importDuration = utils.MaybeFormatDurationForTest(cfg.TestOnly, r.EndTime.Sub(r.StartTime))
			}
			return nil
		}(); err != nil {
			return finishedExport, errors.CombineErrors(err, targetTableConnCopy.Close(ctx))
		}

		netDuration := utils.MaybeFormatDurationForTest(cfg.TestOnly, time.Since(tableStartTime))
		cdcCursor := utils.MaybeFormatCDCCursor(cfg.TestOnly, sqlSrc.CDCCursor())
		summaryLogger.Info().
			Dur("net_duration_ms", netDuration).
			Str("net_duration", utils.FormatDurationToTimeString(netDuration)).
			Dur("import_duration_ms", importDuration).
			Str("import_duration", utils.FormatDurationToTimeString(importDuration)).
			Dur("export_duration_ms", exportDuration).
			Str("export_duration", utils.FormatDurationToTimeString(exportDuration)).
			Int("num_rows", e.NumRows).
			Str("cdc_cursor", cdcCursor).
			Msgf("data import on target for table complete")
		fetchmetrics.TableImportDuration.WithLabelValues(table.SafeString()).Set(float64(importDuration.Milliseconds()))
		fetchmetrics.TableOverallDuration.WithLabelValues(table.SafeString()).Set(float64(netDuration.Milliseconds()))

		return finishedExport, nil
	}
	return finishedExport, nil
}

func reportTelemetry(
	logger zerolog.Logger, cfg Config, conns dbconn.OrderedConns, store datablobstorage.Store,
) {
	dialect := "CockroachDB"
	for _, conn := range conns {
		if !conn.IsCockroach() {
			dialect = conn.Dialect()
			break
		}
	}
	ingestMethod := "import"
	if cfg.UseCopy {
		ingestMethod = "copy"
	}
	molttelemetry.ReportTelemetryAsync(
		logger,
		"molt_fetch_dialect_"+dialect,
		"molt_fetch_ingest_method_"+ingestMethod,
		"molt_fetch_blobstore_"+store.TelemetryName(),
	)
}

func initStatusEntry(
	ctx context.Context, cfg Config, conn *pgx.Conn, dialect string,
) (*status.FetchStatus, error) {
	// Setup the status and exception tables.
	if err := status.CreateStatusAndExceptionTables(ctx, conn); err != nil {
		return nil, err
	}

	createdAt := time.Now().UTC()
	fetchStatus := &status.FetchStatus{
		Name:          fmt.Sprintf("run at %d", createdAt.Unix()),
		StartedAt:     createdAt,
		SourceDialect: dialect,
	}

	// This is the case where we have continuation tokens.
	// and we want to "reuse the last fetch run" as the current one.
	if !IsClearContinuationTokenMode(cfg) {
		id, err := uuid.FromString(cfg.FetchID)
		if err != nil {
			return nil, err
		}

		fetchStatus.ID = id
	} else {
		if err := fetchStatus.CreateEntry(ctx, conn); err != nil {
			return nil, err
		}
	}

	return fetchStatus, nil
}

func getExceptionLogMapping(
	ctx context.Context, cfg Config, targetPgxConn *pgx.Conn,
) (excLogMap map[string]*status.ExceptionLog, retErr error) {
	exceptionLogMapping := map[string]*status.ExceptionLog{}
	if IsImportCopyOnlyMode(cfg) {
		exceptionLogs := []*status.ExceptionLog{}
		if strings.TrimSpace(cfg.ContinuationToken) == "" {
			exceptionLogsFID, err := status.GetAllExceptionLogsByFetchID(ctx, targetPgxConn, cfg.FetchID)
			if err != nil {
				return nil, err
			}
			exceptionLogs = append(exceptionLogs, exceptionLogsFID...)
		} else {
			exceptionLog, err := status.GetExceptionLogByToken(ctx, targetPgxConn, cfg.ContinuationToken)
			if err != nil {
				return nil, err
			}

			if cfg.ContinuationFileName != "" {
				exceptionLog.FileName = cfg.ContinuationFileName
			}

			exceptionLogs = append(exceptionLogs, exceptionLog)
		}

		exceptionLogMapping = status.GetTableSchemaToExceptionLog(exceptionLogs)
	}

	return exceptionLogMapping, nil
}

func IsImportCopyOnlyMode(cfg Config) bool {
	return strings.TrimSpace(cfg.FetchID) != ""
}

// IsClearContinuationTokenMode determines if we must clear continuation tokens
// from the _molt_fetch_exceptions table. This is to ensure that there is only
// ever one set of active tokens at a time.
func IsClearContinuationTokenMode(cfg Config) bool {
	// Condition: fresh fetch run without continuation.
	return strings.TrimSpace(cfg.FetchID) == ""
}
