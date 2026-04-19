package dbconn

import (
	"testing"

	"github.com/go-sql-driver/mysql"
	"github.com/stretchr/testify/require"
)

func TestHandleTLSParams(t *testing.T) {
	type args struct {
		cfg *mysql.Config
	}
	type output struct {
		tlsMap    map[string]string
		cfgParams map[string]string
	}

	tests := []struct {
		name string
		args args
		want output
	}{
		{
			name: "at least one parameter is an SSL parameter",
			args: args{
				cfg: &mysql.Config{
					Params: map[string]string{"sslmode": "require", "timeout": "10"},
				},
			},
			want: output{
				tlsMap:    map[string]string{"sslmode": "require"},
				cfgParams: map[string]string{"timeout": "10"},
			},
		},
		{
			name: "all of the parameters are SSL parameters",
			args: args{
				cfg: &mysql.Config{
					Params: map[string]string{"sslmode": "require", "sslrootcert": "rootsecret", "sslcert": "secret", "sslkey": "keysecret"},
				},
			},
			want: output{
				tlsMap:    map[string]string{"sslmode": "require", "sslrootcert": "rootsecret", "sslcert": "secret", "sslkey": "keysecret"},
				cfgParams: map[string]string{},
			},
		},
		{
			name: "none of the parameters are SSL parameters",
			args: args{
				cfg: &mysql.Config{
					Params: map[string]string{"timeout": "10", "conversion": "clean"},
				},
			},
			want: output{
				tlsMap:    map[string]string{},
				cfgParams: map[string]string{"timeout": "10", "conversion": "clean"},
			},
		},
		{
			name: "no parameters specified",
			args: args{
				cfg: &mysql.Config{
					Params: map[string]string{},
				},
			},
			want: output{
				tlsMap:    map[string]string{},
				cfgParams: map[string]string{},
			},
		},
		{
			name: "parameters not initialized",
			args: args{
				cfg: &mysql.Config{},
			},
			want: output{
				tlsMap:    map[string]string{},
				cfgParams: map[string]string(nil),
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := handleTLSParams(tt.args.cfg)
			require.Equal(t, tt.want.cfgParams, tt.args.cfg.Params)
			require.Equal(t, tt.want.tlsMap, got)
		})
	}
}
