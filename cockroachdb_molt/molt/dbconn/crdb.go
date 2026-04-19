package dbconn

import (
	"context"
	"fmt"

	"github.com/cockroachdb/errors"
)

func (c *PGConn) CheckIfAfterVersion(ctx context.Context, version string) (bool, error) {
	var isAfterVersion bool
	if err := c.QueryRow(ctx, fmt.Sprintf(`SELECT crdb_internal.is_at_least_version('%s');`, version)).Scan(&isAfterVersion); err != nil {
		return isAfterVersion, errors.Wrapf(err, "failed checking if the version of crdb is at least %s", version)
	}
	return isAfterVersion, nil
}
