package verifyservice

import (
	"context"

	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/verify"
	"github.com/cockroachdb/molt/verify/inconsistency"
	"github.com/rs/zerolog"
)

type VerifyRunner struct {
	config Config
	logger zerolog.Logger
}

func NewVerifyRunner(config Config, logger zerolog.Logger) VerifyRunner {
	return VerifyRunner{
		config: config,
		logger: logger,
	}
}

func (r VerifyRunner) Run(
	ctx context.Context,
	request JobRequest,
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

	sourceConn, err := dbconn.Connect(ctx, "source", sourceConnStr)
	if err != nil {
		return err
	}
	defer func() {
		runErr = errors.CombineErrors(runErr, sourceConn.Close(ctx))
	}()

	destinationConn, err := dbconn.Connect(ctx, "target", destinationConnStr)
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

	return verify.Verify(
		ctx,
		dbconn.OrderedConns{sourceConn, destinationConn},
		r.logger,
		combinedReporter,
		verify.WithDBFilter(request.FilterConfig()),
	)
}
