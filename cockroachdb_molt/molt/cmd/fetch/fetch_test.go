package fetch

import (
	"bytes"
	"testing"

	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/fetch"
	"github.com/rs/zerolog"
	"github.com/stretchr/testify/require"
)

func TestHandleMySQLConcurrencyFlags(t *testing.T) {
	type args struct {
		sourceConn dbconn.Conn
		cfg        *fetch.Config
	}
	tests := []struct {
		name                 string
		args                 args
		expectedLoggerOutput string
		expectedConfig       *fetch.Config
	}{
		{
			name: "not a MySQL connection at all",
			args: args{
				sourceConn: &dbconn.PGConn{},
				cfg:        &fetch.Config{},
			},
			expectedLoggerOutput: "",
		},
		{
			name: "MySQL connection and table and export concurrency not set",
			args: args{
				sourceConn: &dbconn.MySQLConn{},
				cfg:        &fetch.Config{},
			},
			expectedLoggerOutput: `{"level":"warn","message":"defaulting export concurrency and table concurrency to 1 in order to guarantee data consistency for MySQL"}
`,
			expectedConfig: &fetch.Config{TableConcurrency: 1, Shards: 1},
		},
		{
			name: "MySQL connection and only table concurrency set to 1 and export concurrency set to default of 4",
			args: args{
				sourceConn: &dbconn.MySQLConn{},
				cfg: &fetch.Config{
					TableConcurrency:      1,
					Shards:                4,
					IsTableConcurrencySet: true,
				},
			},
			expectedLoggerOutput: `{"level":"warn","message":"table concurrency or export concurrency is greater than 1. This can lead to inconsistency when migrating MySQL data. For details on maintaining consistency when using --table-concurrency and --export-concurrency: https://www.cockroachlabs.com/docs/stable/molt-fetch"}
`,
			expectedConfig: &fetch.Config{TableConcurrency: 1, Shards: 4},
		},
		{
			name: "MySQL connection and export concurrency is set to greater than 1",
			args: args{
				sourceConn: &dbconn.MySQLConn{},
				cfg: &fetch.Config{
					Shards:                 4,
					IsExportConcurrencySet: true,
				},
			},
			expectedLoggerOutput: `{"level":"warn","message":"table concurrency or export concurrency is greater than 1. This can lead to inconsistency when migrating MySQL data. For details on maintaining consistency when using --table-concurrency and --export-concurrency: https://www.cockroachlabs.com/docs/stable/molt-fetch"}
`,
			expectedConfig: &fetch.Config{TableConcurrency: 0, Shards: 4},
		},
		{
			name: "MySQL connection and table concurrency is set to greater than 1",
			args: args{
				sourceConn: &dbconn.MySQLConn{},
				cfg: &fetch.Config{
					Shards:                1,
					TableConcurrency:      4,
					IsTableConcurrencySet: true,
				},
			},
			expectedLoggerOutput: `{"level":"warn","message":"table concurrency or export concurrency is greater than 1. This can lead to inconsistency when migrating MySQL data. For details on maintaining consistency when using --table-concurrency and --export-concurrency: https://www.cockroachlabs.com/docs/stable/molt-fetch"}
`,
			expectedConfig: &fetch.Config{TableConcurrency: 4, Shards: 1},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var b bytes.Buffer
			logger := zerolog.New(&b)
			handleMySQLConcurrencyFlags(logger, tt.args.sourceConn, tt.args.cfg)
			require.Equal(t, tt.expectedLoggerOutput, b.String())

			if tt.expectedConfig != nil && tt.args.cfg != nil {
				require.Equal(t, tt.expectedConfig.TableConcurrency, tt.args.cfg.TableConcurrency)
				require.Equal(t, tt.expectedConfig.Shards, tt.args.cfg.Shards)
			}
		})
	}
}
