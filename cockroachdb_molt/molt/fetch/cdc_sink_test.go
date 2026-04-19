package fetch

import (
	"context"
	"fmt"
	"os"
	"strings"
	"syscall"
	"testing"
	"time"

	"github.com/cockroachdb/datadriven"
	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/fetch/datablobstorage"
	"github.com/cockroachdb/molt/fetch/dataexport"
	"github.com/cockroachdb/molt/retry"
	"github.com/cockroachdb/molt/testutils"
	"github.com/cockroachdb/molt/utils"
	"github.com/rs/zerolog"
	"github.com/stretchr/testify/require"
)

const (
	createSourceTableStmt        = `CREATE TABLE replicatortbl1 (id INT PRIMARY KEY, t TEXT);`
	insertInitialSourceTableStmt = `INSERT INTO replicatortbl1 VALUES (1, 'aaa'), (2, 'bb b'), (3, 'ééé'), (4, '🫡🫡🫡'), (5, '娜娜'), (6, 'Лукас'), (7, 'ルカス');`
	insertCDCSourceTableStmt     = `INSERT INTO replicatortbl1 VALUES (8, 'replicator added this');`
	numRetries                   = 8
)

func TestCDCSinkFetchIntegration(t *testing.T) {
	ctx := context.Background()
	enterpriseOrg := os.Getenv("ENTERPRISE_ORG")
	enterpriseLicense := os.Getenv("ENTERPRISE_LICENSE")

	// Helper methods.
	var defaultSetupFunc = func(conns dbconn.OrderedConns) error {
		if _, err := testutils.ExecConnQuery(ctx, createSourceTableStmt, conns[0]); err != nil {
			return err
		}

		if _, err := testutils.ExecConnQuery(ctx, insertInitialSourceTableStmt, conns[0]); err != nil {
			return err
		}

		// Need a slight wait between creating the table and starting fetch
		// in order to address a timing issue for AOST for CRDB, when
		// the AOST can get trimmed and it misses the CREATE/INSERT table here.
		if conns[0].IsCockroach() {
			time.Sleep(time.Second)
		}

		return nil
	}

	var defaultPollingFunc = func(conns dbconn.OrderedConns, sentinelFileName string, sigint chan os.Signal) (err error) {
		defer func() {
			t.Logf("sending termination signal to fetch")
			sigint <- syscall.SIGTERM
		}()

		// Need to use a cloned connection otherwise the following query will
		// error out and say that you cannot modify data during a read-only
		// transaction.
		// We want a fresh connection to the database that can write the
		// row we need to verify that CDC sink works.
		sourceConnClone, err := conns[0].Clone(context.Background())
		require.NoError(t, err)

		t.Logf("writing row to the source side of the database")
		if _, err := testutils.ExecConnQuery(ctx, insertCDCSourceTableStmt, sourceConnClone); err != nil {
			require.NoError(t, err)
		}

		t.Logf("polling for the row to be written to the target")
		r, err := retry.NewRetry(retry.Settings{
			InitialBackoff: 1 * time.Second,
			Multiplier:     1,
			MaxBackoff:     5 * time.Second,
			MaxRetries:     numRetries,
		})
		if err != nil {
			return err
		}

		result := ""
		if err := r.Do(func() error {
			result = testutils.QueryConnCommand(t, &datadriven.TestData{
				CmdArgs: []datadriven.CmdArg{
					{Key: "target"},
				},
				Input: "SELECT * FROM replicatortbl1 WHERE t='replicator added this';",
			}, conns)
			if strings.Contains(result, "replicator added this") {
				t.Logf("found replicated entry, breaking from loop")
				return nil
			}

			t.Logf("could not find replicated entry, retrying...")

			return errors.New("could not find replicated entry")
		}, func(err error) {}); err != nil {
			require.NoError(t, err)
		}

		t.Logf("checking to verify that the replicator added row is present")
		require.Contains(t, result, "replicator added this")
		return nil
	}

	for i, tc := range []struct {
		desc        string
		setup       func(conns dbconn.OrderedConns) error
		pollingFunc func(conns dbconn.OrderedConns, sentinelFileName string, sigint chan os.Signal) (err error)
		src         string
		dest        string
		forceError  bool
		expectError bool
	}{
		{
			desc: "crdb source and no error",
			setup: func(conns dbconn.OrderedConns) error {
				if err := defaultSetupFunc(conns); err != nil {
					return err
				}

				clusterOrgQuery := fmt.Sprintf("SET CLUSTER SETTING cluster.organization = '%s';", enterpriseOrg)
				if _, err := testutils.ExecConnQuery(ctx, clusterOrgQuery, conns[0]); err != nil {
					return err
				}

				licenseQuery := fmt.Sprintf("SET CLUSTER SETTING enterprise.license = '%s';", enterpriseLicense)
				if _, err := testutils.ExecConnQuery(ctx, licenseQuery, conns[0]); err != nil {
					return err
				}

				return nil
			},
			pollingFunc: defaultPollingFunc,
			src:         testutils.CRDBConnStr(),
			dest:        testutils.CRDBTargetConnStr(),
			expectError: false,
		},
		{
			desc:        "pg source and no error",
			setup:       defaultSetupFunc,
			pollingFunc: defaultPollingFunc,
			src:         testutils.PGConnStr(),
			dest:        testutils.CRDBConnStr(),
			expectError: false,
		},
		{
			desc:        "mysql source and no error",
			setup:       defaultSetupFunc,
			pollingFunc: defaultPollingFunc,
			src:         testutils.MySQLConnStr(),
			dest:        testutils.CRDBConnStr(),
			expectError: false,
		},
		// We are choosing to test only one error case in order to
		// save on time. Also, this path for cleanup from an error
		// is dialect agnostic.
		{
			desc:        "mysql source and error",
			setup:       defaultSetupFunc,
			pollingFunc: defaultPollingFunc,
			src:         testutils.MySQLConnStr(),
			dest:        testutils.CRDBConnStr(),
			forceError:  true,
			expectError: true,
		},
	} {
		t.Run(tc.desc, func(t *testing.T) {
			ctx := context.Background()
			var conns dbconn.OrderedConns
			var err error
			dbName := fmt.Sprintf("fetch_cdc_sink_%d", i)
			logger := zerolog.New(os.Stderr)
			logger.Info().Msg("starting logger")

			conns[0], err = dbconn.TestOnlyCleanDatabase(ctx, "source", tc.src, dbName)
			require.NoError(t, err)
			conns[1], err = dbconn.TestOnlyCleanDatabase(ctx, "target", tc.dest, dbName)
			require.NoError(t, err)

			t.Logf("verifying connection to source and target")
			for _, c := range conns {
				_, err := testutils.ExecConnQuery(ctx, "SELECT 1", c)
				require.NoError(t, err)
			}
			t.Logf("successfully connected to both source and target")

			t.Logf("setting up the source and target table")
			require.NoError(t, tc.setup(conns))

			t.Logf("setting up intermediate store")
			var store datablobstorage.Store
			defer func() {
				if store != nil {
					require.NoError(t, store.Cleanup(ctx))
				}
			}()
			dir, err := os.MkdirTemp("", "")
			require.NoError(t, err)
			t.Logf("stored in local dir %q", dir)

			localStoreListenAddr, localStoreCrdbAccessAddr := testutils.GetLocalStoreAddrs("crdb", "4041")

			store, err = datablobstorage.NewLocalStore(logger, dir, localStoreListenAddr, localStoreCrdbAccessAddr)
			require.NoError(t, err)

			// Testing configs
			localhostName := getDockerLocalhostName()
			if os.Getenv("NO_DOCKER") == "1" {
				localhostName = "localhost"
			}

			filter := utils.DefaultFilterConfig()
			knobs := testutils.FetchTestingKnobs{
				CDCSink: &testutils.CDCSinkKnob{
					PollingFunction:   tc.pollingFunc,
					ForceCDCSinkError: tc.forceError,
					LocalhostName:     localhostName,
				},
				HistoryRetention: &testutils.HistoryRetentionKnob{},
			}
			defaultCRDBPTSExtFreq := 10 * time.Minute
			numShards := 1

			err = Fetch(
				ctx,
				Config{
					UseCopy:                  false,
					DropAndRecreateNewSchema: true,
					Cleanup:                  true,
					NonInteractive:           true,
					Shards:                   numShards,
					ExportSettings: dataexport.Settings{
						RowBatchSize:             2,
						CRDBPTSExtensionFreq:     defaultCRDBPTSExtFreq,
						CRDBPTSExtensionLifetime: 24 * time.Hour,
						PG: dataexport.PGReplicationSlotSettings{
							Plugin:       "pgoutput",
							SlotName:     "cdc_sink_slot",
							DropIfExists: true,
						},
					},
					OngoingReplication:        true,
					AllowCockroachReplication: true,
				},
				logger,
				conns,
				store,
				filter,
				knobs,
			)

			if tc.expectError {
				require.EqualError(t, err, testForcedError)
			} else {
				require.NoError(t, err)
			}
		})
	}
}
