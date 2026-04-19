package testutils

import (
	"os"
	"time"

	"github.com/cockroachdb/molt/dbconn"
)

type FetchTestingKnobs struct {
	// Used to simulate testing when the CSV input file is wrong.
	TriggerCorruptCSVFile bool

	FailedWriteToBucket FailedWriteToBucketKnob

	FailedEstablishSrcConnForExport *FailedEstablishSrcConnForExportKnob

	HistoryRetention *HistoryRetentionKnob

	CDCSink *CDCSinkKnob
}

type FailedEstablishSrcConnForExportKnob struct {
	SleepDuration time.Duration
}

type FailedWriteToBucketKnob struct {
	FailedBeforeReadFromPipe bool
	FailedAfterReadFromPipe  bool
}

type HistoryRetentionKnob struct {
	ExtensionFrequency time.Duration
	ExtensionCnt       *int64
	Cancelled          bool
	JobID              *string
}

type CDCSinkKnob struct {
	PollingFunction   func(conns dbconn.OrderedConns, sentinelFileName string, sigintChan chan os.Signal) (err error)
	SentinelFileName  string
	ForceCDCSinkError bool
	LocalhostName     string
}
