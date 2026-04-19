package verifyservice

import (
	"context"

	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/utils"
	"github.com/cockroachdb/molt/verify"
	"github.com/cockroachdb/molt/verify/inconsistency"
	"github.com/rs/zerolog"
)

type connectFunc func(ctx context.Context, preferredID dbconn.ID, connStr string) (dbconn.Conn, error)

type runVerifyFunc func(
	ctx context.Context,
	conns dbconn.OrderedConns,
	logger zerolog.Logger,
	reporter inconsistency.Reporter,
	filter utils.FilterConfig,
) error

type VerifyRunner struct {
	config    Config
	logger    zerolog.Logger
	connect   connectFunc
	runVerify runVerifyFunc
}

func NewVerifyRunner(config Config, logger zerolog.Logger) VerifyRunner {
	return VerifyRunner{
		config:    config,
		logger:    logger,
		connect:   dbconn.Connect,
		runVerify: defaultRunVerify,
	}
}

func (r VerifyRunner) Run(
	ctx context.Context,
	request RunRequest,
	reporter inconsistency.Reporter,
) (runErr error) {
	sourceConnStr, err := r.config.Verify.Source.ConnectionString()
	if err != nil {
		return err
	}
	destinationConnStr, err := r.config.Verify.Destination.ConnectionString()
	if err != nil {
		return err
	}

	connect := r.connect
	if connect == nil {
		connect = dbconn.Connect
	}
	runVerify := r.runVerify
	if runVerify == nil {
		runVerify = defaultRunVerify
	}

	sourceConn, err := connect(ctx, "source", sourceConnStr)
	if err != nil {
		return err
	}
	defer func() {
		runErr = errors.CombineErrors(runErr, sourceConn.Close(ctx))
	}()

	destinationConn, err := connect(ctx, "target", destinationConnStr)
	if err != nil {
		return err
	}
	defer func() {
		runErr = errors.CombineErrors(runErr, destinationConn.Close(ctx))
	}()

	combinedReporter := inconsistency.CombinedReporter{
		Reporters: []inconsistency.Reporter{
			reporter,
			inconsistency.LogReporter{Logger: r.logger},
		},
	}
	defer combinedReporter.Close()

	return runVerify(
		ctx,
		dbconn.OrderedConns{sourceConn, destinationConn},
		r.logger,
		combinedReporter,
		request.FilterConfig(),
	)
}

func defaultRunVerify(
	ctx context.Context,
	conns dbconn.OrderedConns,
	logger zerolog.Logger,
	reporter inconsistency.Reporter,
	filter utils.FilterConfig,
) error {
	return verify.Verify(
		ctx,
		conns,
		logger,
		reporter,
		verify.WithDBFilter(filter),
	)
}
