package mysqlurl

import (
	"crypto/tls"
	"net/url"
	"testing"
	"time"

	mysqldriver "github.com/go-sql-driver/mysql"
	"github.com/stretchr/testify/require"
)

var testDSN = []struct {
	in  string
	out *mysqldriver.Config
}{
	{
		in: "mysql://username:password@protocol(address)/dbname?param=value",
		out: &mysqldriver.Config{
			User:                 "username",
			Passwd:               "password",
			Net:                  "protocol",
			Addr:                 "address",
			DBName:               "dbname",
			Params:               map[string]string{"param": "value"},
			Loc:                  time.UTC,
			MaxAllowedPacket:     defaultMaxAllowedPacket,
			AllowNativePasswords: true,
			CheckConnLiveness:    true,
		},
	},
	{
		in: "username:password@protocol(address)/dbname?param=value&columnsWithAlias=true",
		out: &mysqldriver.Config{
			User:                 "username",
			Passwd:               "password",
			Net:                  "protocol",
			Addr:                 "address",
			DBName:               "dbname",
			Params:               map[string]string{"param": "value"},
			Loc:                  time.UTC,
			MaxAllowedPacket:     defaultMaxAllowedPacket,
			AllowNativePasswords: true,
			CheckConnLiveness:    true,
			ColumnsWithAlias:     true,
		},
	},
	{
		in: "username:password@protocol(address)/dbname?param=value&columnsWithAlias=true&multiStatements=true",
		out: &mysqldriver.Config{
			User:                 "username",
			Passwd:               "password",
			Net:                  "protocol",
			Addr:                 "address",
			DBName:               "dbname",
			Params:               map[string]string{"param": "value"},
			Loc:                  time.UTC,
			MaxAllowedPacket:     defaultMaxAllowedPacket,
			AllowNativePasswords: true,
			CheckConnLiveness:    true,
			ColumnsWithAlias:     true,
			MultiStatements:      true,
		},
	},
	{
		in: "user:password@tcp(localhost:5555)/dbname?charset=utf8&tls=true",
		out: &mysqldriver.Config{
			User:                 "user",
			Passwd:               "password",
			Net:                  "tcp",
			Addr:                 "localhost:5555",
			DBName:               "dbname",
			Params:               map[string]string{"charset": "utf8"},
			Loc:                  time.UTC,
			MaxAllowedPacket:     defaultMaxAllowedPacket,
			AllowNativePasswords: true,
			CheckConnLiveness:    true,
			TLSConfig:            "true",
			TLS:                  &tls.Config{ServerName: "localhost"},
		},
	},
	{
		in: "user:password@tcp(localhost:5555)/dbname?charset=utf8mb4,utf8&tls=skip-verify",
		out: &mysqldriver.Config{
			User:                 "user",
			Passwd:               "password",
			Net:                  "tcp",
			Addr:                 "localhost:5555",
			DBName:               "dbname",
			Params:               map[string]string{"charset": "utf8mb4,utf8"},
			Loc:                  time.UTC,
			MaxAllowedPacket:     defaultMaxAllowedPacket,
			AllowNativePasswords: true,
			CheckConnLiveness:    true,
			TLSConfig:            "skip-verify",
			TLS:                  &tls.Config{InsecureSkipVerify: true},
		},
	},
	{
		in: "user:password@/dbname?loc=UTC&timeout=30s&readTimeout=1s&writeTimeout=1s&allowAllFiles=1&clientFoundRows=true&allowOldPasswords=TRUE&collation=utf8mb4_unicode_ci&maxAllowedPacket=16777216&tls=false&allowCleartextPasswords=true&parseTime=true&rejectReadOnly=true",
		out: &mysqldriver.Config{
			User:                    "user",
			Passwd:                  "password",
			Net:                     "tcp",
			Addr:                    "127.0.0.1:3306",
			DBName:                  "dbname",
			Collation:               "utf8mb4_unicode_ci",
			Loc:                     time.UTC,
			TLSConfig:               "false",
			AllowCleartextPasswords: true,
			AllowNativePasswords:    true,
			Timeout:                 30 * time.Second,
			ReadTimeout:             time.Second,
			WriteTimeout:            time.Second,
			AllowAllFiles:           true,
			AllowOldPasswords:       true,
			CheckConnLiveness:       true,
			ClientFoundRows:         true,
			MaxAllowedPacket:        16777216,
			ParseTime:               true,
			RejectReadOnly:          true,
		},
	},
	{
		in: "user:password@/dbname?allowNativePasswords=false&checkConnLiveness=false&maxAllowedPacket=0&allowFallbackToPlaintext=true",
		out: &mysqldriver.Config{
			User:                     "user",
			Passwd:                   "password",
			Net:                      "tcp",
			Addr:                     "127.0.0.1:3306",
			DBName:                   "dbname",
			Loc:                      time.UTC,
			MaxAllowedPacket:         0,
			AllowFallbackToPlaintext: true,
			AllowNativePasswords:     false,
			CheckConnLiveness:        false,
		},
	},
}

var testConnStr = []struct {
	in         string
	outCfg     *mysqldriver.Config
	outConnStr string
	tlsCfg     struct {
		params     url.Values
		skipVerify bool
		address    string
	}
}{
	{
		in: "mysql://username:password@address:3306/dbname?param=value&sslmode=disable",
		outCfg: &mysqldriver.Config{
			User:                 "username",
			Passwd:               "password",
			Net:                  "tcp",
			Addr:                 "address:3306",
			DBName:               "dbname",
			Params:               map[string]string{"param": "value", "sslmode": "disable"},
			Loc:                  time.UTC,
			MaxAllowedPacket:     defaultMaxAllowedPacket,
			AllowNativePasswords: true,
			CheckConnLiveness:    true,
		},

		outConnStr: "mysql://username:password@address:3306/dbname?param=value&sslmode=disable",
	},
	{
		in: "mysql://username:password@address:3306/dbname?param=value&columnsWithAlias=true&sslmode=disable",
		outCfg: &mysqldriver.Config{
			User:                 "username",
			Passwd:               "password",
			Net:                  "tcp",
			Addr:                 "address:3306",
			DBName:               "dbname",
			Params:               map[string]string{"param": "value", "sslmode": "disable"},
			Loc:                  time.UTC,
			MaxAllowedPacket:     defaultMaxAllowedPacket,
			AllowNativePasswords: true,
			CheckConnLiveness:    true,
			ColumnsWithAlias:     true,
		},

		outConnStr: "mysql://username:password@address:3306/dbname?columnsWithAlias=true&param=value&sslmode=disable",
	},
	{
		in: "mysql://username:password@address:3306/dbname?param=value&columnsWithAlias=true&multiStatements=true&sslmode=disable",
		outCfg: &mysqldriver.Config{
			User:                 "username",
			Passwd:               "password",
			Net:                  "tcp",
			Addr:                 "address:3306",
			DBName:               "dbname",
			Params:               map[string]string{"param": "value", "sslmode": "disable"},
			Loc:                  time.UTC,
			MaxAllowedPacket:     defaultMaxAllowedPacket,
			AllowNativePasswords: true,
			CheckConnLiveness:    true,
			ColumnsWithAlias:     true,
			MultiStatements:      true,
		},

		outConnStr: "mysql://username:password@address:3306/dbname?columnsWithAlias=true&multiStatements=true&param=value&sslmode=disable",
	},
	{
		in: "mysql://username:password@address:3306/dbname?param=value&columnsWithAlias=true&multiStatements=true&sslmode=require&sslrootcert=./testdata/rootcert.pem",
		outCfg: &mysqldriver.Config{
			User:                 "username",
			Passwd:               "password",
			Net:                  "tcp",
			Addr:                 "address:3306",
			DBName:               "dbname",
			Params:               map[string]string{"param": "value", "sslmode": "require", "sslrootcert": "./testdata/rootcert.pem"},
			Loc:                  time.UTC,
			MaxAllowedPacket:     defaultMaxAllowedPacket,
			AllowNativePasswords: true,
			CheckConnLiveness:    true,
			ColumnsWithAlias:     true,
			MultiStatements:      true,
			TLSConfig:            "parsed_30d4087c93e881810918678641ba5ac159bc5e40",
		},
		tlsCfg: struct {
			params     url.Values
			skipVerify bool
			address    string
		}{
			params:     url.Values{"sslmode": []string{"require"}, "sslrootcert": []string{"./testdata/rootcert.pem"}},
			skipVerify: true,
			address:    "address:3306",
		},
		outConnStr: "mysql://username:password@address:3306/dbname?columnsWithAlias=true&multiStatements=true&param=value&sslmode=require&sslrootcert=.%2Ftestdata%2Frootcert.pem",
	},
	{
		in: "mysql://username:password@address:3306/dbname?param=value&columnsWithAlias=true&multiStatements=true&sslmode=verify-full&sslrootcert=./testdata/rootcert.pem",
		outCfg: &mysqldriver.Config{
			User:                 "username",
			Passwd:               "password",
			Net:                  "tcp",
			Addr:                 "address:3306",
			DBName:               "dbname",
			Params:               map[string]string{"param": "value", "sslmode": "verify-full", "sslrootcert": "./testdata/rootcert.pem"},
			Loc:                  time.UTC,
			MaxAllowedPacket:     defaultMaxAllowedPacket,
			AllowNativePasswords: true,
			CheckConnLiveness:    true,
			ColumnsWithAlias:     true,
			MultiStatements:      true,
			TLSConfig:            "parsed_d703e4191f4328a95d960e2df1231f7080c80be2",
		},
		tlsCfg: struct {
			params     url.Values
			skipVerify bool
			address    string
		}{
			params:     url.Values{"sslmode": []string{"verify-full"}, "sslrootcert": []string{"./testdata/rootcert.pem"}},
			skipVerify: false,
			address:    "address:3306",
		},
		outConnStr: "mysql://username:password@address:3306/dbname?columnsWithAlias=true&multiStatements=true&param=value&sslmode=verify-full&sslrootcert=.%2Ftestdata%2Frootcert.pem",
	},
}

func TestParseDSN(t *testing.T) {
	for _, conn := range testDSN {
		t.Run(conn.in, func(t *testing.T) {
			cfg, err := ParseDSN(conn.in)
			require.NoError(t, err)
			// nil the logger since its unused and will
			// cause the test to fail.
			cfg.Logger = nil
			require.Equal(t, conn.out, cfg)
		})
	}
}

func TestParseConnStr(t *testing.T) {
	for _, conn := range testConnStr {
		t.Run(conn.in, func(t *testing.T) {
			cfg, err := ParseConnStr(conn.in)
			require.NoError(t, err)

			// nil the logger since its unused and will
			// cause the test to fail.
			cfg.Logger = nil

			// Check expected TLS configs exist.
			require.Equal(t, len(conn.tlsCfg.params) > 0, cfg.TLS != nil)

			if cfg.TLS != nil {
				testTLSCfg, err := newClientTLSConfig(conn.tlsCfg.params, conn.tlsCfg.skipVerify, conn.tlsCfg.address)
				require.NoError(t, err)
				checkTLSConfigEqual(t, testTLSCfg, cfg.TLS)

				// If we format the DSN and re-parse it, check the TLS matches as well.
				reparse, err := mysqldriver.ParseDSN(cfg.FormatDSN())
				require.NoError(t, err)
				checkTLSConfigEqual(t, testTLSCfg, reparse.TLS)

				// nil the TLS config now that we have checked the fields.
				// otherwise require.Equal will fail due to pointer
				// comparisons.
				cfg.TLS = nil
			}
			require.Equal(t, conn.outCfg, cfg)
			// The reverse operation should be equal to the input conn string.
			// We don't want to escape the params since it will be double escaped due
			// to the format call already doing a escape.
			require.Equal(t, conn.outConnStr, CfgToConnStr(cfg, false /* do not escape query params */))
		})
	}
}

func TestParse(t *testing.T) {
	for _, conn := range testDSN {
		t.Run(conn.in, func(t *testing.T) {
			cfg, err := Parse(conn.in)
			require.NoError(t, err)
			// nil the logger since its unused and will
			// cause the test to fail.
			cfg.Logger = nil
			require.Equal(t, conn.out, cfg)
		})
	}
}

func TestParseFallback(t *testing.T) {
	for _, conn := range testConnStr {
		t.Run(conn.in, func(t *testing.T) {
			cfg, err := Parse(conn.in)
			require.NoError(t, err)
			// nil the logger since its unused and will
			// cause the test to fail.
			cfg.Logger = nil
			if cfg.TLS != nil {
				testTLSCfg, err := newClientTLSConfig(conn.tlsCfg.params, conn.tlsCfg.skipVerify, conn.tlsCfg.address)
				require.NoError(t, err)
				checkTLSConfigEqual(t, testTLSCfg, cfg.TLS)
				// nil the TLS config now that we have checked the fields.
				// otherwise require.Equal will fail due to pointer
				// comparisons.
				cfg.TLS = nil
			}
			require.Equal(t, conn.outCfg, cfg)
			// The reverse operation should be equal to the input conn string.
			// We don't want to escape the params since it will be double escaped due
			// to the format call already doing a escape.
			require.Equal(t, conn.outConnStr, CfgToConnStr(cfg, false /* do not escape query params */))
		})
	}
}

func checkTLSConfigEqual(t *testing.T, expected, actual *tls.Config) {
	require.True(t, expected.RootCAs.Equal(actual.RootCAs))
	require.Equal(t, expected.ServerName, actual.ServerName)
	require.Equal(t, expected.Certificates, actual.Certificates)
	require.Equal(t, expected.InsecureSkipVerify, actual.InsecureSkipVerify)
}
